use std::collections::HashMap;

use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{Expr, Stmt, StmtClassDef, StmtFunctionDef, StmtTypeAlias, TypeParam};
use ruff_text_size::{Ranged, TextRange, TextSize};

use crate::config::{Config, PythonVersion};
use crate::symbol_table::SymbolTable;
use crate::transforms::literal_types;

/// Polyfills PEP 695 generic syntax (Python 3.12+) and `type` alias statements.
///
/// - `class Foo[T, S](Base):` → `class Foo(Base, Generic[_T, _S]):` + TypeVar defs
/// - `def f[T](x: T) -> T:` → `def f(x: T) -> T:` + TypeVar defs
/// - `type Alias = T` → `Alias: TypeAlias = T`
pub struct GenericPolyfill<'src, 'sym> {
    source: &'src str,
    symbols: &'sym SymbolTable,
    config: Config,
    pub edits: Vec<(TextRange, String)>,
    // Imports to inject at the top of the file.
    pub needed_imports: ImportNeeds,
}

#[derive(Default)]
pub struct ImportNeeds {
    pub typevar: bool,
    pub generic: bool,
    pub typevar_tuple: bool,
    pub unpack: bool,
    pub paramspec: bool,
    pub typealias_type: bool,
    pub typevar_needs_ext: bool, // TypeVar(default=) on < 3.13
}

impl ImportNeeds {
    pub fn is_empty(&self) -> bool {
        !self.typevar
            && !self.generic
            && !self.typevar_tuple
            && !self.unpack
            && !self.paramspec
            && !self.typealias_type
    }

    /// Build the import lines to prepend to the file.
    pub fn into_lines(self) -> Vec<String> {
        let mut lines = Vec::new();

        let mut typing_names: Vec<&str> = Vec::new();
        let mut ext_names: Vec<&str> = Vec::new();

        if self.typevar {
            if self.typevar_needs_ext {
                ext_names.push("TypeVar");
            } else {
                typing_names.push("TypeVar");
            }
        }
        if self.typevar_tuple {
            typing_names.push("TypeVarTuple");
        }
        if self.unpack {
            typing_names.push("Unpack");
        }
        if self.paramspec {
            typing_names.push("ParamSpec");
        }
        if self.generic {
            typing_names.push("Generic");
        }
        if self.typealias_type {
            ext_names.push("TypeAliasType");
        }

        if !typing_names.is_empty() {
            lines.push(format!("from typing import {}", typing_names.join(", ")));
        }
        if !ext_names.is_empty() {
            lines.push(format!(
                "from typing_extensions import {}",
                ext_names.join(", ")
            ));
        }

        lines
    }
}

impl<'src, 'sym> GenericPolyfill<'src, 'sym> {
    pub fn new(source: &'src str, symbols: &'sym SymbolTable, config: Config) -> Self {
        Self {
            source,
            symbols,
            config,
            edits: Vec::new(),
            needed_imports: ImportNeeds::default(),
        }
    }

    fn src(&self, range: TextRange) -> &str {
        &self.source[usize::from(range.start())..usize::from(range.end())]
    }

    fn line_start_of(&self, pos: TextSize) -> (TextSize, &str) {
        let offset = usize::from(pos);
        let line_start = self.source[..offset]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let rest = &self.source[line_start..offset];
        let ws_len = rest.len() - rest.trim_start().len();
        (
            TextSize::from(line_start as u32),
            &self.source[line_start..line_start + ws_len],
        )
    }

    /// Processes a list of type parameters.
    /// Returns (mangled_names_for_Generic, TypeVar_definition_lines).
    fn process_type_params(
        &mut self,
        params: &[TypeParam],
    ) -> (Vec<String>, Vec<String>) {
        let mut generic_args: Vec<String> = Vec::new();
        let mut defs: Vec<String> = Vec::new();

        for param in params {
            match param {
                TypeParam::TypeVar(tv) => {
                    let name = tv.name.id.as_str();
                    let mangled = mangle(name);

                    let mut args: Vec<String> = vec![format!("\"{mangled}\"")];

                    if let Some(bound) = &tv.bound {
                        // Tuple literal → union constraints (positional);
                        // anything else → bound=. Apply literal_types rewrite
                        // to non-tuple bounds so e.g. `T: 1 | 2` becomes
                        // `bound=Literal[1, 2]`.
                        if matches!(bound.as_ref(), Expr::Tuple(_)) {
                            let bound_src = self.src(bound.range());
                            let inner = bound_src
                                .trim_start_matches('(')
                                .trim_end_matches(')')
                                .trim();
                            args.push(inner.to_owned());
                        } else {
                            let bound_src =
                                literal_types::rewrite_type_expr(self.source, self.symbols, bound)
                                    .unwrap_or_else(|| self.src(bound.range()).to_owned());
                            args.push(format!("bound={bound_src}"));
                        }
                    }

                    if let Some(default) = &tv.default {
                        let default_src =
                            literal_types::rewrite_type_expr(self.source, self.symbols, default)
                                .unwrap_or_else(|| self.src(default.range()).to_owned());
                        if self.config.min_version < PythonVersion::V313 {
                            self.needed_imports.typevar_needs_ext = true;
                        }
                        args.push(format!("default={default_src}"));
                    }

                    let def = format!("{mangled} = TypeVar({})", args.join(", "));

                    self.needed_imports.typevar = true;
                    generic_args.push(mangled.clone());
                    defs.push(def);
                }

                TypeParam::TypeVarTuple(tvt) => {
                    let name = tvt.name.id.as_str();
                    let mangled = mangle(name);
                    defs.push(format!("{mangled} = TypeVarTuple(\"{mangled}\")"));
                    self.needed_imports.typevar_tuple = true;
                    self.needed_imports.unpack = true;
                    generic_args.push(format!("*{mangled}"));
                }

                TypeParam::ParamSpec(ps) => {
                    let name = ps.name.id.as_str();
                    let mangled = mangle(name);
                    defs.push(format!("{mangled} = ParamSpec(\"{mangled}\")"));
                    self.needed_imports.paramspec = true;
                    generic_args.push(mangled);
                }
            }
        }

        (generic_args, defs)
    }

    fn process_class(&mut self, class: &StmtClassDef) {
        let Some(tp) = &class.type_params else {
            return;
        };
        // PEP 695 class type params are native syntax in 3.12+
        if self.config.min_version >= PythonVersion::V312 {
            return;
        }

        let rename_map = build_rename_map(&tp.type_params);
        let (generic_args, defs) = self.process_type_params(&tp.type_params);
        let generic_str = format!("Generic[{}]", generic_args.join(", "));
        self.needed_imports.generic = true;

        // Modify or add base classes.
        if let Some(args) = &class.arguments {
            // Emit rename edits for type params within base class expressions
            // as individual edits — this lets literal_types and auto_quote also
            // emit their own non-overlapping edits on the same expressions.
            for base_expr in &args.args {
                rename_in_expr(base_expr, &rename_map, &mut self.edits);
            }
            if args.args.is_empty() && args.keywords.is_empty() {
                // empty `()` → replace with `(Generic[_T])`; 2-char range, safe
                self.edits.push((args.range(), format!("({generic_str})")));
            } else {
                // insert `, Generic[_T]` before the closing `)` as a zero-width
                // edit so it doesn't subsume any edits on the base expressions
                let rparen = args.range().end() - TextSize::from(1);
                let insert_range = TextRange::new(rparen, rparen);
                self.edits.push((insert_range, format!(", {generic_str}")));
            }
            self.edits.push((tp.range(), String::new()));
        } else {
            self.edits.push((tp.range(), format!("({generic_str})")));
        }

        // Insert TypeVar definitions before the class.
        let (line_start, indent) = self.line_start_of(class.range().start());
        let indent = indent.to_owned();
        let prefix: String = defs.iter().map(|d| format!("{indent}{d}\n")).collect();
        self.edits.push((TextRange::new(line_start, line_start), prefix));

        // Rename type param references in class body.
        for stmt in &class.body {
            rename_in_stmt(stmt, &rename_map, &mut self.edits);
        }
    }

    fn process_function(&mut self, func: &StmtFunctionDef) {
        let Some(tp) = &func.type_params else {
            return;
        };
        // PEP 695 function type params are native syntax in 3.12+
        if self.config.min_version >= PythonVersion::V312 {
            return;
        }

        let rename_map = build_rename_map(&tp.type_params);
        let (_, defs) = self.process_type_params(&tp.type_params);

        // Remove `[T, ...]` from the function signature.
        self.edits.push((tp.range(), String::new()));

        // Insert TypeVar definitions before the function.
        let (line_start, indent) = self.line_start_of(func.range().start());
        let indent = indent.to_owned();
        let prefix: String = defs.iter().map(|d| format!("{indent}{d}\n")).collect();
        self.edits.push((TextRange::new(line_start, line_start), prefix));

        // Rename type param references in parameter annotations, return type, and body.
        let all_params = func
            .parameters
            .posonlyargs
            .iter()
            .chain(func.parameters.args.iter())
            .chain(func.parameters.kwonlyargs.iter());
        for param in all_params {
            if let Some(ann) = &param.parameter.annotation {
                rename_in_expr(ann, &rename_map, &mut self.edits);
            }
        }
        if let Some(ret) = &func.returns {
            rename_in_expr(ret, &rename_map, &mut self.edits);
        }
        for stmt in &func.body {
            rename_in_stmt(stmt, &rename_map, &mut self.edits);
        }
    }

    fn process_type_alias(&mut self, alias: &StmtTypeAlias) {
        // `type Point = tuple[float, float]`
        //   → `Point = TypeAliasType("Point", tuple[float, float])`
        if self.config.min_version >= PythonVersion::V312 {
            return;
        }

        let name_src = self.src(alias.name.range()).to_owned();
        let raw_value_src = self.src(alias.value.range()).to_owned();
        // Pull in the literal-types rewrite for the RHS — our
        // `alias.range()` edit subsumes anything `literal_types` emitted on
        // the value alone, so we have to splice the rewrite into our output.
        let literal_rewrite =
            literal_types::rewrite_type_expr(self.source, self.symbols, &alias.value);

        let (type_params_arg, defs, value_src) = if let Some(tp) = &alias.type_params {
            let rename_map = build_rename_map(&tp.type_params);
            let (generic_args, type_defs) = self.process_type_params(&tp.type_params);

            // Apply renames inside the value expression inline (value is
            // subsumed by the alias.range() edit so can't be emitted globally).
            //
            // Combining renames with the literal rewrite needs care: the
            // literal rewrite emits a single replacement covering the whole
            // value, so any rename edits inside its range would overlap and
            // be lost. For now we use the literal rewrite when one exists
            // (typical case: the value has no type-param references), and
            // fall back to renames-only otherwise.
            let value_src = if let Some(rewrite) = &literal_rewrite {
                rewrite.clone()
            } else {
                let mut value_renames: Vec<(TextRange, String)> = Vec::new();
                rename_in_expr(&alias.value, &rename_map, &mut value_renames);
                apply_renames_in_slice(
                    &raw_value_src,
                    alias.value.range().start(),
                    &value_renames,
                )
            };

            // TypeVarTuple entries have a leading `*` in generic_args (for
            // Generic[*_Ts]) but `type_params=` wants the bare name.
            let param_names: Vec<&str> =
                generic_args.iter().map(|s| s.trim_start_matches('*')).collect();
            let trailing = if param_names.len() == 1 { "," } else { "" };
            let tps = format!(", type_params=({}{})", param_names.join(", "), trailing);

            (tps, type_defs, value_src)
        } else {
            (
                String::new(),
                Vec::new(),
                literal_rewrite.unwrap_or(raw_value_src),
            )
        };

        self.needed_imports.typealias_type = true;

        let (_line_start, indent) = self.line_start_of(alias.range().start());
        let indent = indent.to_owned();

        let mut replacement = String::new();
        for d in &defs {
            replacement.push_str(&format!("{indent}{d}\n"));
        }
        replacement.push_str(&format!(
            "{indent}{name_src} = TypeAliasType(\"{name_src}\", {value_src}{type_params_arg})"
        ));

        self.edits.push((alias.range(), replacement));
    }
}

impl<'src, 'sym, 'ast> Visitor<'ast> for GenericPolyfill<'src, 'sym> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        match stmt {
            Stmt::ClassDef(class) => self.process_class(class),
            Stmt::FunctionDef(func) => self.process_function(func),
            Stmt::TypeAlias(alias) => {
                self.process_type_alias(alias);
                return; // don't recurse into the alias value
            }
            _ => {}
        }
        walk_stmt(self, stmt);
    }
}

fn build_rename_map(params: &[TypeParam]) -> HashMap<String, String> {
    params
        .iter()
        .map(|p| {
            let name = match p {
                TypeParam::TypeVar(tv) => tv.name.id.as_str(),
                TypeParam::TypeVarTuple(tvt) => tvt.name.id.as_str(),
                TypeParam::ParamSpec(ps) => ps.name.id.as_str(),
            };
            (name.to_owned(), mangle(name))
        })
        .collect()
}

fn rename_in_expr(
    expr: &Expr,
    renames: &HashMap<String, String>,
    edits: &mut Vec<(TextRange, String)>,
) {
    match expr {
        Expr::Name(n) => {
            if let Some(new) = renames.get(n.id.as_str()) {
                edits.push((n.range(), new.clone()));
            }
        }
        Expr::Subscript(s) => {
            rename_in_expr(&s.value, renames, edits);
            rename_in_expr(&s.slice, renames, edits);
        }
        Expr::Attribute(a) => rename_in_expr(&a.value, renames, edits),
        Expr::Tuple(t) => t.elts.iter().for_each(|e| rename_in_expr(e, renames, edits)),
        Expr::List(l) => l.elts.iter().for_each(|e| rename_in_expr(e, renames, edits)),
        Expr::BinOp(b) => {
            rename_in_expr(&b.left, renames, edits);
            rename_in_expr(&b.right, renames, edits);
        }
        Expr::Call(c) => {
            rename_in_expr(&c.func, renames, edits);
            c.arguments.args.iter().for_each(|a| rename_in_expr(a, renames, edits));
        }
        Expr::UnaryOp(u) => rename_in_expr(&u.operand, renames, edits),
        _ => {}
    }
}

fn rename_in_stmt(
    stmt: &Stmt,
    renames: &HashMap<String, String>,
    edits: &mut Vec<(TextRange, String)>,
) {
    match stmt {
        Stmt::AnnAssign(a) => {
            rename_in_expr(&a.annotation, renames, edits);
            if let Some(v) = &a.value {
                rename_in_expr(v, renames, edits);
            }
        }
        Stmt::FunctionDef(f) => {
            for p in f
                .parameters
                .posonlyargs
                .iter()
                .chain(f.parameters.args.iter())
                .chain(f.parameters.kwonlyargs.iter())
            {
                if let Some(ann) = &p.parameter.annotation {
                    rename_in_expr(ann, renames, edits);
                }
            }
            if let Some(ret) = &f.returns {
                rename_in_expr(ret, renames, edits);
            }
            for s in &f.body {
                rename_in_stmt(s, renames, edits);
            }
        }
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                rename_in_expr(v, renames, edits);
            }
        }
        Stmt::Assign(a) => {
            for t in &a.targets {
                rename_in_expr(t, renames, edits);
            }
            rename_in_expr(&a.value, renames, edits);
        }
        Stmt::Expr(e) => rename_in_expr(&e.value, renames, edits),
        Stmt::If(i) => {
            rename_in_expr(&i.test, renames, edits);
            for s in &i.body {
                rename_in_stmt(s, renames, edits);
            }
            for clause in &i.elif_else_clauses {
                for s in &clause.body {
                    rename_in_stmt(s, renames, edits);
                }
            }
        }
        _ => {}
    }
}

fn apply_renames_in_slice(
    text: &str,
    text_start: TextSize,
    renames: &[(TextRange, String)],
) -> String {
    let base = usize::from(text_start);
    let mut local: Vec<(usize, usize, &str)> = renames
        .iter()
        .filter_map(|(r, s)| {
            let lo = usize::from(r.start()).checked_sub(base)?;
            let hi = usize::from(r.end()).checked_sub(base)?;
            (hi <= text.len()).then_some((lo, hi, s.as_str()))
        })
        .collect();
    local.sort_by_key(|&(lo, ..)| std::cmp::Reverse(lo));
    let mut result = text.to_owned();
    for (lo, hi, new) in local {
        result.replace_range(lo..hi, new);
    }
    result
}

fn mangle(name: &str) -> String {
    if name.starts_with('_') {
        name.to_owned()
    } else {
        format!("_{name}")
    }
}

#[cfg(test)]
mod tests {
    use crate::{transpile, Config, config::PythonVersion};
    use indoc::indoc;

    fn check(input: &str, expected: &str) {
        assert_eq!(transpile(input, &Config::default()).unwrap(), expected);
    }

    fn check_at(input: &str, expected: &str, version: PythonVersion) {
        let config = Config { min_version: version };
        assert_eq!(transpile(input, &config).unwrap(), expected);
    }

    #[test]
    fn class_simple_typevar() {
        check(
            indoc! {"
                class Foo[T]: ...
            "},
            indoc! {"
                from typing import TypeVar, Generic
                _T = TypeVar(\"_T\")
                class Foo(Generic[_T]): ...
            "},
        );
    }

    #[test]
    fn class_with_base() {
        check(
            indoc! {"
                class Foo[T](Base): ...
            "},
            indoc! {"
                from typing import TypeVar, Generic
                _T = TypeVar(\"_T\")
                class Foo(Base, Generic[_T]): ...
            "},
        );
    }

    #[test]
    fn class_with_empty_parens() {
        check(
            indoc! {"
                class Foo[T](): ...
            "},
            indoc! {"
                from typing import TypeVar, Generic
                _T = TypeVar(\"_T\")
                class Foo(Generic[_T]): ...
            "},
        );
    }

    #[test]
    fn class_multiple_params() {
        check(
            indoc! {"
                class Map[K, V]: ...
            "},
            indoc! {"
                from typing import TypeVar, Generic
                _K = TypeVar(\"_K\")
                _V = TypeVar(\"_V\")
                class Map(Generic[_K, _V]): ...
            "},
        );
    }

    #[test]
    fn class_bound_typevar() {
        check(
            indoc! {"
                class Foo[T: int]: ...
            "},
            indoc! {"
                from typing import TypeVar, Generic
                _T = TypeVar(\"_T\", bound=int)
                class Foo(Generic[_T]): ...
            "},
        );
    }

    #[test]
    fn class_bound_literal_typevar() {
        // Bound `1 | 2` must be rewritten to `Literal[1, 2]`, and the default
        // must not be silently dropped when a bound is present.
        check(
            indoc! {"
                class A[T: 1 | 2 = 1 | 2]: ...
            "},
            indoc! {"
                from typing import Generic
                from typing_extensions import TypeVar
                from typing import Literal
                _T = TypeVar(\"_T\", bound=Literal[1, 2], default=Literal[1, 2])
                class A(Generic[_T]): ...
            "},
        );
    }

    #[test]
    fn class_default_typevar() {
        // Default-only TypeVar with literal default should also rewrite.
        check(
            indoc! {"
                class A[T = 1 | 2]: ...
            "},
            indoc! {"
                from typing import Generic
                from typing_extensions import TypeVar
                from typing import Literal
                _T = TypeVar(\"_T\", default=Literal[1, 2])
                class A(Generic[_T]): ...
            "},
        );
    }

    #[test]
    fn generic_function() {
        check(
            indoc! {"
                def identity[T](x: T) -> T:
                    return x
            "},
            indoc! {"
                from typing import TypeVar
                _T = TypeVar(\"_T\")
                def identity(x: _T) -> _T:
                    return x
            "},
        );
    }

    #[test]
    fn class_body_rename() {
        check(
            indoc! {"
                class A[T]:
                    t: T
                    def method(self, x: T) -> T:
                        return x
            "},
            indoc! {"
                from typing import TypeVar, Generic
                _T = TypeVar(\"_T\")
                class A(Generic[_T]):
                    t: _T
                    def method(self, x: _T) -> _T:
                        return x
            "},
        );
    }

    #[test]
    fn type_alias_simple() {
        check(
            indoc! {"
                type Point = tuple[float, float]
            "},
            indoc! {"
                from typing_extensions import TypeAliasType
                Point = TypeAliasType(\"Point\", tuple[float, float])
            "},
        );
    }

    #[test]
    fn type_alias_generic() {
        check(
            indoc! {"
                type Vector[T] = list[T]
            "},
            indoc! {"
                from typing import TypeVar
                from typing_extensions import TypeAliasType
                _T = TypeVar(\"_T\")
                Vector = TypeAliasType(\"Vector\", list[_T], type_params=(_T,))
            "},
        );
    }

    #[test]
    fn no_type_params_unchanged() {
        check(
            indoc! {"
                class Foo(Base): ...
            "},
            indoc! {"
                class Foo(Base): ...
            "},
        );
    }

    #[test]
    fn class_generic_unchanged_on_312() {
        // PEP 695 is native in 3.12+, so the polyfill must not fire
        let src = "class Foo[T]: ...\n";
        check_at(src, src, PythonVersion::V312);
        check_at(src, src, PythonVersion::V313);
        check_at(src, src, PythonVersion::V314);
    }

    #[test]
    fn function_generic_unchanged_on_312() {
        let src = indoc! {"
            def identity[T](x: T) -> T:
                return x
        "};
        check_at(src, src, PythonVersion::V312);
        check_at(src, src, PythonVersion::V314);
    }

    #[test]
    fn type_alias_unchanged_on_312() {
        let src = "type Point = tuple[float, float]\n";
        check_at(src, src, PythonVersion::V312);
        check_at(src, src, PythonVersion::V314);
    }
}
