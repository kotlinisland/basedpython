//! Rewrites literal expressions in type-expression context to `Literal[...]`.
//!
//! `a: "asdf" | 5`              → `a: Literal["asdf", 5]`
//! `a: 1 | 2 | int`             → `a: Literal[1, 2] | int`
//! `a: 5`                       → `a: Literal[5]`
//! `X[1 | 2]` where X is a type → `X[Literal[1, 2]]`
//!
//! Type-expression context is determined structurally (annotations, function
//! return types) or via `SymbolTable` lookup (subscript slices where the value
//! resolves to a class, type alias, or imported/unknown name).

use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_python_ast::{Expr, ExprSubscript, Operator, Stmt, TypeParam, UnaryOp};
use ruff_text_size::{Ranged, TextRange, TextSize};

use crate::symbol_table::{BindingKind, SymbolTable};

pub struct LiteralType<'src, 'sym> {
    source: &'src str,
    symbols: &'sym SymbolTable,
    pub edits: Vec<(TextRange, String)>,
    pub needs_literal_import: bool,
}

impl<'src, 'sym> LiteralType<'src, 'sym> {
    pub fn new(source: &'src str, symbols: &'sym SymbolTable) -> Self {
        Self {
            source,
            symbols,
            edits: Vec::new(),
            needs_literal_import: false,
        }
    }

    fn src(&self, range: TextRange) -> &str {
        &self.source[usize::from(range.start())..usize::from(range.end())]
    }

    /// Whether a `Subscript.value` resolves to something whose subscript slice
    /// is a type-argument position.
    fn is_type_subscript(&self, value: &Expr) -> bool {
        match value {
            Expr::Name(n) => match self.symbols.resolve(n.id.as_str(), n.range().start()) {
                Some(k) => k.subscript_is_type_context(),
                // Unresolved → assume type (handles builtins like `list`,
                // `dict` and imports we don't see the source for).
                None => true,
            },
            Expr::Attribute(a) => match a.value.as_ref() {
                Expr::Name(base) => matches!(
                    self.symbols.resolve(base.id.as_str(), base.range().start()),
                    Some(BindingKind::Import) | None
                ),
                _ => false,
            },
            _ => false,
        }
    }

    /// Is `value` a reference to the named typing special form?
    ///
    /// Matches on name only. A local shadow would be caught by the symbol
    /// table (the user would have to locally define e.g. an `Annotated`
    /// that's also used to subscript — vanishingly rare).
    fn is_typing_name(&self, value: &Expr, name: &str) -> bool {
        match value {
            Expr::Name(n) => {
                n.id.as_str() == name
                    && matches!(
                        self.symbols.resolve(n.id.as_str(), n.range().start()),
                        Some(BindingKind::Import) | None
                    )
            }
            Expr::Attribute(a) => {
                a.attr.id.as_str() == name
                    && matches!(
                        a.value.as_ref(),
                        Expr::Name(base) if matches!(
                            self.symbols.resolve(base.id.as_str(), base.range().start()),
                            Some(BindingKind::Import) | None
                        )
                    )
            }
            _ => false,
        }
    }

    fn is_annotated_name(&self, value: &Expr) -> bool {
        self.is_typing_name(value, "Annotated")
    }

    fn is_literal_name(&self, value: &Expr) -> bool {
        self.is_typing_name(value, "Literal")
    }

    /// Transform a type expression. Returns `Some(rewrite)` if any literal
    /// was promoted to `Literal[...]`, else `None`.
    ///
    /// `at_root` distinguishes a bare-annotation position (where `a: None`
    /// stays bare) from an interior position (where `None` inside `A | None`
    /// participates in a Literal group).
    fn transform_type_expr(&mut self, expr: &Expr, at_root: bool) -> Option<String> {
        if at_root && matches!(expr, Expr::NoneLiteral(_)) {
            return None;
        }

        if is_literal_expr(expr) {
            self.needs_literal_import = true;
            return Some(format!("Literal[{}]", self.src(expr.range())));
        }

        if let Expr::BinOp(b) = expr {
            if matches!(b.op, Operator::BitOr) {
                return self.transform_union(expr);
            }
        }

        if let Expr::Subscript(s) = expr {
            return self.transform_subscript(s);
        }

        None
    }

    fn transform_union(&mut self, expr: &Expr) -> Option<String> {
        let parts = flatten_union(expr);

        enum Group {
            Literals(Vec<String>),
            Other(String),
        }
        let mut groups: Vec<Group> = Vec::new();
        let mut changed = false;
        // `None` between two non-literal arms (e.g. `int | None`) stays bare.
        // We only know whether to attach a `None` to a Literal group once we
        // see what follows it, so hold it pending until we see a non-None
        // literal (attach forward) or anything else (flush as bare `None`).
        let mut pending_none = false;

        for p in parts {
            if matches!(p, Expr::NoneLiteral(_)) {
                if let Some(Group::Literals(lits)) = groups.last_mut() {
                    lits.push("None".to_owned());
                } else {
                    pending_none = true;
                }
            } else if is_literal_expr(p) {
                let s = self.src(p.range()).to_owned();
                if pending_none {
                    pending_none = false;
                    match groups.last_mut() {
                        Some(Group::Literals(lits)) => {
                            lits.push("None".to_owned());
                            lits.push(s);
                        }
                        _ => groups.push(Group::Literals(vec!["None".to_owned(), s])),
                    }
                } else if let Some(Group::Literals(lits)) = groups.last_mut() {
                    lits.push(s);
                } else {
                    groups.push(Group::Literals(vec![s]));
                }
                changed = true;
            } else {
                if pending_none {
                    pending_none = false;
                    groups.push(Group::Other("None".to_owned()));
                }
                let rewritten = self.transform_type_expr(p, false);
                if rewritten.is_some() {
                    changed = true;
                }
                let s = rewritten.unwrap_or_else(|| self.src(p.range()).to_owned());
                groups.push(Group::Other(s));
            }
        }
        if pending_none {
            groups.push(Group::Other("None".to_owned()));
        }

        if !changed {
            return None;
        }
        if groups.iter().any(|g| matches!(g, Group::Literals(_))) {
            self.needs_literal_import = true;
        }

        let out: Vec<String> = groups
            .into_iter()
            .map(|g| match g {
                Group::Literals(lits) => format!("Literal[{}]", lits.join(", ")),
                Group::Other(s) => s,
            })
            .collect();
        Some(out.join(" | "))
    }

    fn transform_subscript(&mut self, s: &ExprSubscript) -> Option<String> {
        // `Literal[...]` is already in literal context — its slice doesn't
        // need re-wrapping.
        if self.is_literal_name(&s.value) {
            return None;
        }
        if self.is_annotated_name(&s.value) {
            return self.transform_annotated_subscript(s);
        }
        if !self.is_type_subscript(&s.value) {
            return None;
        }

        let slice = s.slice.as_ref();
        // Unparenthesized tuple → multiple type args (e.g. `dict[str, int]`).
        if let Expr::Tuple(t) = slice {
            if !t.parenthesized {
                let rewrites: Vec<Option<String>> = t
                    .elts
                    .iter()
                    .map(|e| self.transform_type_expr(e, false))
                    .collect();
                if !rewrites.iter().any(|r| r.is_some()) {
                    return None;
                }
                let parts: Vec<String> = rewrites
                    .into_iter()
                    .zip(t.elts.iter())
                    .map(|(r, e)| r.unwrap_or_else(|| self.src(e.range()).to_owned()))
                    .collect();
                let value_src = self.src(s.value.range());
                return Some(format!("{value_src}[{}]", parts.join(", ")));
            }
        }

        let rewrite = self.transform_type_expr(slice, false)?;
        let value_src = self.src(s.value.range());
        Some(format!("{value_src}[{rewrite}]"))
    }

    /// `Annotated[T, meta...]` — only the first arg is a type position; the
    /// rest is arbitrary metadata and must not be rewritten.
    fn transform_annotated_subscript(&mut self, s: &ExprSubscript) -> Option<String> {
        let Expr::Tuple(t) = s.slice.as_ref() else {
            return None;
        };
        if t.parenthesized || t.elts.is_empty() {
            return None;
        }
        let first_rewrite = self.transform_type_expr(&t.elts[0], false)?;
        let mut parts = vec![first_rewrite];
        for e in &t.elts[1..] {
            parts.push(self.src(e.range()).to_owned());
        }
        let value_src = self.src(s.value.range());
        Some(format!("{value_src}[{}]", parts.join(", ")))
    }

    fn transform_annotation(&mut self, expr: &Expr) {
        if let Some(rewrite) = self.transform_type_expr(expr, true) {
            self.edits.push((expr.range(), rewrite));
        }
    }
}

impl<'src, 'sym, 'ast> Visitor<'ast> for LiteralType<'src, 'sym> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        match stmt {
            Stmt::AnnAssign(a) => {
                self.transform_annotation(&a.annotation);
                if let Some(v) = &a.value {
                    self.visit_expr(v);
                }
            }
            Stmt::TypeAlias(a) => {
                // RHS of `type X = ...` is a type-expression context. The edit
                // we emit here may be subsumed by `generics.rs` when targeting
                // < 3.12 (which rewrites the whole statement to TypeAliasType);
                // in that case `generics.rs` re-runs `rewrite_type_expr` to
                // pick up our rewrite. The flag we flip here still drives the
                // `from typing import Literal` preamble either way.
                self.transform_annotation(&a.value);
            }
            Stmt::FunctionDef(f) => {
                for p in f.parameters.iter_non_variadic_params() {
                    if let Some(ann) = &p.parameter.annotation {
                        self.transform_annotation(ann);
                    }
                    if let Some(default) = &p.default {
                        self.visit_expr(default);
                    }
                }
                if let Some(v) = &f.parameters.vararg {
                    if let Some(ann) = &v.annotation {
                        self.transform_annotation(ann);
                    }
                }
                if let Some(k) = &f.parameters.kwarg {
                    if let Some(ann) = &k.annotation {
                        self.transform_annotation(ann);
                    }
                }
                if let Some(ret) = &f.returns {
                    self.transform_annotation(ret);
                }
                for s in &f.body {
                    self.visit_stmt(s);
                }
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let Expr::Subscript(s) = expr {
            if self.is_type_subscript(&s.value) || self.is_annotated_name(&s.value) {
                if let Some(rewrite) = self.transform_type_expr(expr, false) {
                    self.edits.push((expr.range(), rewrite));
                }
            }
        }
        walk_expr(self, expr);
    }

    fn visit_type_param(&mut self, type_param: &'ast TypeParam) {
        // Bound and default are type-expression contexts. When the generics
        // polyfill is active (< 3.12) it emits a wider edit that subsumes
        // ours; we still need the visit so `needs_literal_import` flips.
        if let TypeParam::TypeVar(tv) = type_param {
            if let Some(b) = &tv.bound {
                self.transform_annotation(b);
            }
            if let Some(d) = &tv.default {
                self.transform_annotation(d);
            }
        }
    }
}

fn is_literal_expr(expr: &Expr) -> bool {
    match expr {
        Expr::NumberLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral(_)
        | Expr::BytesLiteral(_) => true,
        Expr::UnaryOp(u) => {
            matches!(u.op, UnaryOp::USub | UnaryOp::UAdd)
                && matches!(u.operand.as_ref(), Expr::NumberLiteral(_))
        }
        _ => false,
    }
}

fn flatten_union(expr: &Expr) -> Vec<&Expr> {
    let mut parts = Vec::new();
    flatten_into(expr, &mut parts);
    parts
}

fn flatten_into<'a>(expr: &'a Expr, out: &mut Vec<&'a Expr>) {
    if let Expr::BinOp(b) = expr {
        if matches!(b.op, Operator::BitOr) {
            flatten_into(&b.left, out);
            flatten_into(&b.right, out);
            return;
        }
    }
    out.push(expr);
}

/// Whether `source` already binds `Literal` (via any kind of import), so lib.rs
/// can avoid prepending a duplicate import.
pub fn literal_already_imported(symbols: &SymbolTable) -> bool {
    matches!(
        symbols.resolve("Literal", TextSize::from(0)),
        Some(BindingKind::Import)
    )
}

/// Stateless rewrite of a type expression, for use by other transforms that
/// need to splice rewritten type text into their own output (e.g.
/// `generics.rs` when wrapping a type alias body in `TypeAliasType(...)`).
///
/// Doesn't update any "needs Literal import" flag — call
/// `LiteralType::visit_stmt` separately for that. The rewrite returned here
/// uses the original source text for sub-expressions, so callers must not
/// then apply incompatible edits (like name renames) on top of overlapping
/// ranges.
pub fn rewrite_type_expr(
    source: &str,
    symbols: &SymbolTable,
    expr: &Expr,
) -> Option<String> {
    let mut t = LiteralType::new(source, symbols);
    t.transform_type_expr(expr, true)
}

#[cfg(test)]
mod tests {
    use crate::{transpile, Config};
    use indoc::indoc;

    fn check(input: &str, expected: &str) {
        assert_eq!(transpile(input, &Config::default()).unwrap(), expected);
    }

    // -------------------------------------------------------------------------
    // Basic literal unions — the core feature.
    // -------------------------------------------------------------------------

    #[test]
    fn simple_int_union() {
        check(
            "a: 1 | 2\n",
            indoc! {"
                from typing import Literal
                a: Literal[1, 2]
            "},
        );
    }

    #[test]
    fn string_union() {
        check(
            "a: \"foo\" | \"bar\"\n",
            indoc! {"
                from typing import Literal
                a: Literal[\"foo\", \"bar\"]
            "},
        );
    }

    #[test]
    fn mixed_literal_types_from_roadmap() {
        check(
            "a: \"asdf\" | 5 = \"asdf\"\n",
            indoc! {"
                from typing import Literal
                a: Literal[\"asdf\", 5] = \"asdf\"
            "},
        );
    }

    #[test]
    fn three_way_int_union() {
        check(
            "a: 1 | 2 | 3\n",
            indoc! {"
                from typing import Literal
                a: Literal[1, 2, 3]
            "},
        );
    }

    #[test]
    fn bool_union() {
        check(
            "a: True | False\n",
            indoc! {"
                from typing import Literal
                a: Literal[True, False]
            "},
        );
    }

    #[test]
    fn negative_int_literals() {
        check(
            "a: -1 | -2\n",
            indoc! {"
                from typing import Literal
                a: Literal[-1, -2]
            "},
        );
    }

    // -------------------------------------------------------------------------
    // Literals mixed with non-literal types.
    // -------------------------------------------------------------------------

    #[test]
    fn literal_on_right_of_type() {
        check(
            "a: int | 1\n",
            indoc! {"
                from typing import Literal
                a: int | Literal[1]
            "},
        );
    }

    #[test]
    fn literal_on_left_of_type() {
        check(
            "a: 1 | int\n",
            indoc! {"
                from typing import Literal
                a: Literal[1] | int
            "},
        );
    }

    #[test]
    fn literals_split_by_type_stay_split() {
        check(
            "a: 1 | int | 2\n",
            indoc! {"
                from typing import Literal
                a: Literal[1] | int | Literal[2]
            "},
        );
    }

    #[test]
    fn adjacent_literals_merge() {
        check(
            "a: 1 | 2 | int\n",
            indoc! {"
                from typing import Literal
                a: Literal[1, 2] | int
            "},
        );
    }

    // -------------------------------------------------------------------------
    // `None` handling.
    // -------------------------------------------------------------------------

    #[test]
    fn none_with_literal_combines() {
        check(
            "a: None | 1\n",
            indoc! {"
                from typing import Literal
                a: Literal[None, 1]
            "},
        );
    }

    #[test]
    fn bare_none_annotation_unchanged() {
        check("a: None\n", "a: None\n");
    }

    // -------------------------------------------------------------------------
    // Bare (non-union) literal annotations.
    // -------------------------------------------------------------------------

    #[test]
    fn bare_int_annotation() {
        check(
            "a: 5\n",
            indoc! {"
                from typing import Literal
                a: Literal[5]
            "},
        );
    }

    #[test]
    fn bare_bool_annotation() {
        check(
            "a: True\n",
            indoc! {"
                from typing import Literal
                a: Literal[True]
            "},
        );
    }

    #[test]
    fn bare_string_annotation() {
        check(
            "a: \"Foo\"\n",
            indoc! {"
                from typing import Literal
                a: Literal[\"Foo\"]
            "},
        );
    }

    // -------------------------------------------------------------------------
    // Function signatures.
    // -------------------------------------------------------------------------

    #[test]
    fn function_parameter() {
        check(
            indoc! {"
                def f(x: 1 | 2):
                    pass
            "},
            indoc! {"
                from typing import Literal
                def f(x: Literal[1, 2]):
                    pass
            "},
        );
    }

    #[test]
    fn function_return_type() {
        check(
            indoc! {"
                def f() -> 1 | 2:
                    pass
            "},
            indoc! {"
                from typing import Literal
                def f() -> Literal[1, 2]:
                    pass
            "},
        );
    }

    // -------------------------------------------------------------------------
    // Value-position preservation — `|` there is bitwise-or.
    // -------------------------------------------------------------------------

    #[test]
    fn value_context_unchanged() {
        check("x = 1 | 2\n", "x = 1 | 2\n");
    }

    #[test]
    fn value_in_annotated_assign_unchanged() {
        check(
            "a: 1 | 2 = 1 | 2\n",
            indoc! {"
                from typing import Literal
                a: Literal[1, 2] = 1 | 2
            "},
        );
    }

    // -------------------------------------------------------------------------
    // Existing `Literal[...]` — don't double-wrap or touch.
    // -------------------------------------------------------------------------

    #[test]
    fn already_literal_unchanged() {
        check(
            indoc! {"
                from typing import Literal
                a: Literal[1, 2]
            "},
            indoc! {"
                from typing import Literal
                a: Literal[1, 2]
            "},
        );
    }

    #[test]
    fn existing_literal_import_not_duplicated() {
        check(
            indoc! {"
                from typing import Literal
                a: 1 | 2
            "},
            indoc! {"
                from typing import Literal
                a: Literal[1, 2]
            "},
        );
    }

    // -------------------------------------------------------------------------
    // Propagation into subscript slices.
    // -------------------------------------------------------------------------

    #[test]
    fn inside_list_generic() {
        check(
            "a: list[1 | 2]\n",
            indoc! {"
                from typing import Literal
                a: list[Literal[1, 2]]
            "},
        );
    }

    #[test]
    fn inside_dict_generic() {
        check(
            "a: dict[str, 1 | 2]\n",
            indoc! {"
                from typing import Literal
                a: dict[str, Literal[1, 2]]
            "},
        );
    }

    #[test]
    fn subscript_propagation_requires_semantic_resolution() {
        // `X` is a type alias → slice is a type context, propagate.
        // `x` is a runtime value → slice is subscription, leave alone.
        //
        // Using min_version 3.12 so the `type X[T] = ...` stays as-is and
        // doesn't interact with the PEP-695 polyfill in this fixture.
        let config = crate::Config {
            min_version: crate::config::PythonVersion::V312,
        };
        let input = indoc! {"
            type X[T] = list[T]
            x = X[1 | 2]()
            b = x[1 | 2]
        "};
        let expected = indoc! {"
            from typing import Literal
            type X[T] = list[T]
            x = X[Literal[1, 2]]()
            b = x[1 | 2]
        "};
        assert_eq!(transpile(input, &config).unwrap(), expected);
    }

    #[test]
    fn annotated_metadata_not_propagated() {
        check("a: Annotated[int, 1 | 2]\n", "a: Annotated[int, 1 | 2]\n");
    }

    // -------------------------------------------------------------------------
    // Type alias values (RHS of `type X = ...`).
    //
    // The output depends on the minimum configured Python version: at 3.12+
    // the `type` statement is native, so we just rewrite the value in place.
    // Below 3.12 the generics polyfill turns the whole statement into a
    // `TypeAliasType(...)` call; the literal rewrite has to land inside.
    // -------------------------------------------------------------------------

    #[test]
    fn type_alias_value_rewritten_312() {
        let config = crate::Config {
            min_version: crate::config::PythonVersion::V312,
        };
        assert_eq!(
            transpile("type X = 1 | 2\n", &config).unwrap(),
            indoc! {"
                from typing import Literal
                type X = Literal[1, 2]
            "},
        );
    }

    #[test]
    fn type_alias_value_rewritten_310() {
        check(
            "type X = 1 | 2\n",
            indoc! {"
                from typing_extensions import TypeAliasType
                from typing import Literal
                X = TypeAliasType(\"X\", Literal[1, 2])
            "},
        );
    }

    // -------------------------------------------------------------------------
    // Class-body annotations.
    // -------------------------------------------------------------------------

    #[test]
    fn class_attribute_annotation() {
        check(
            indoc! {"
                class Foo:
                    x: 1 | 2
            "},
            indoc! {"
                from typing import Literal
                class Foo:
                    x: Literal[1, 2]
            "},
        );
    }
}
