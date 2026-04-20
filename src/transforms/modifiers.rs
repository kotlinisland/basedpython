//! Rewrites basedpython modifier keywords to Python decorator equivalents.
//!
//! The parser synthesises a `Decorator` node for each modifier keyword — the
//! decorator range covers the keyword text plus trailing whitespace up to but
//! not including the following `class`/`def` token. Because that text starts
//! with a letter rather than `@`, this transform can distinguish synthetic
//! decorators from real ones.
//!
//! Modifier → output mapping
//! ─────────────────────────
//! `final class/def`       → `@final` (from `typing`)
//! `abstract class`        → (modifier deleted, no decorator)
//! `abstract def`          → `@abstractmethod` (from `abc`)
//! `override def`          → `@override` (from `typing`)
//! `open class`            → (modifier deleted, no decorator)
//! `static def`            → `@staticmethod`
//! `class def`             → `@classmethod`
//! `data class`            → `@dataclass(slots=True)` (from `dataclasses`)
//! `frozen data class`     → `@dataclass(frozen=True, slots=True)`
//! `enum class`            → base class `Enum` added (from `enum`)
//! `let x = 5`             → `x: Final = 5` (from `typing`)
//! `class a = 1`           → `a: ClassVar = 1` (from `typing`)
//! `newtype Foo = int`     → `Foo = NewType("Foo", int)` (from `typing`)
//! `export`/`public`       → modifier deleted; symbol name added to auto-generated `__all__`
//! `private`               → modifier deleted; symbol renamed with `_` prefix and excluded from `__all__`

use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{Expr, Stmt, StmtAnnAssign, StmtClassDef, StmtFunctionDef};
use ruff_text_size::{Ranged, TextRange, TextSize};

pub struct Modifiers<'src> {
    source: &'src str,
    pub edits: Vec<(TextRange, String)>,
    /// Needs `from typing import final` (decorator for classes/methods)
    pub needs_final: bool,
    /// Needs `from typing import Final` (type annotation for constants)
    pub needs_final_annotation: bool,
    pub needs_abstractmethod: bool,
    pub needs_override: bool,
    pub needs_dataclass: bool,
    pub needs_enum: bool,
    pub needs_protocol: bool,
    pub needs_classvar: bool,
    pub needs_newtype: bool,
    /// Names marked `export`/`public` at module level. Used to generate `__all__`.
    pub exports: Vec<String>,
    /// Tracks the current class-nesting depth so visibility modifiers can
    /// distinguish module-level declarations from class members.
    class_depth: u32,
}

impl<'src> Modifiers<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            edits: Vec::new(),
            needs_final: false,
            needs_final_annotation: false,
            needs_abstractmethod: false,
            needs_override: false,
            needs_dataclass: false,
            needs_enum: false,
            needs_protocol: false,
            needs_classvar: false,
            needs_newtype: false,
            exports: Vec::new(),
            class_depth: 0,
        }
    }

    fn line_indent(&self, pos: TextSize) -> &str {
        let offset = usize::from(pos);
        let line_start = self.source[..offset]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let rest = &self.source[line_start..offset];
        let ws_len = rest.len() - rest.trim_start().len();
        &self.source[line_start..line_start + ws_len]
    }

    /// Check whether a decorator is synthetic (produced by the parser for a
    /// modifier keyword rather than written by the user with a `@` sigil).
    fn is_synthetic(&self, dec: &ruff_python_ast::Decorator) -> bool {
        let start = usize::from(dec.range().start());
        self.source.as_bytes().get(start).copied() != Some(b'@')
    }

    /// Return the `id` of the synthetic decorator's Name expression, or `None`
    /// if the decorator is a normal `@…` decorator.
    fn synthetic_name<'a>(&self, dec: &'a ruff_python_ast::Decorator) -> Option<&'a str> {
        if !self.is_synthetic(dec) {
            return None;
        }
        if let Expr::Name(n) = &dec.expression {
            Some(n.id.as_str())
        } else {
            None
        }
    }

    fn process_class(&mut self, class: &StmtClassDef) {
        for dec in &class.decorator_list {
            let Some(name) = self.synthetic_name(dec) else {
                continue;
            };
            let indent = self.line_indent(dec.range().start()).to_owned();
            match name {
                "abstract" | "open" => {
                    // Just remove the modifier prefix, no decorator needed.
                    self.edits.push((dec.range(), String::new()));
                }
                "final" => {
                    self.needs_final = true;
                    self.edits
                        .push((dec.range(), format!("@final\n{indent}")));
                }
                "data_class" => {
                    self.needs_dataclass = true;
                    self.edits
                        .push((dec.range(), format!("@dataclass(slots=True)\n{indent}")));
                }
                "frozen_data_class" => {
                    self.needs_dataclass = true;
                    self.edits.push((
                        dec.range(),
                        format!("@dataclass(frozen=True, slots=True)\n{indent}"),
                    ));
                }
                "enum_class" => {
                    self.needs_enum = true;
                    // Remove the modifier prefix; the enum base class is inserted separately.
                    self.edits.push((dec.range(), String::new()));
                    self.insert_enum_base(class);
                }
                "protocol_class" => {
                    self.needs_protocol = true;
                    // Replace "protocol " with "class "; Protocol base is inserted separately.
                    self.edits.push((dec.range(), "class ".to_owned()));
                    self.insert_protocol_base(class);
                }
                "export" => {
                    self.edits.push((dec.range(), String::new()));
                    if self.class_depth == 0 {
                        self.exports.push(class.name.as_str().to_owned());
                    }
                }
                "private" => {
                    self.edits.push((dec.range(), String::new()));
                    if self.class_depth == 0 {
                        self.rename_with_underscore(class.name.range());
                    }
                }
                _ => {}
            }
        }
    }

    fn process_function(&mut self, func: &StmtFunctionDef) {
        for dec in &func.decorator_list {
            let Some(name) = self.synthetic_name(dec) else {
                continue;
            };
            let indent = self.line_indent(dec.range().start()).to_owned();
            match name {
                "abstract" => {
                    self.needs_abstractmethod = true;
                    self.edits
                        .push((dec.range(), format!("@abstractmethod\n{indent}")));
                }
                "final" => {
                    self.needs_final = true;
                    self.edits
                        .push((dec.range(), format!("@final\n{indent}")));
                }
                "override" => {
                    self.needs_override = true;
                    self.edits
                        .push((dec.range(), format!("@override\n{indent}")));
                }
                "open" => {
                    // `open` does not apply to functions; treat as no-op.
                    self.edits.push((dec.range(), String::new()));
                }
                "static" => {
                    self.edits
                        .push((dec.range(), format!("@staticmethod\n{indent}")));
                }
                "classmethod" => {
                    self.edits
                        .push((dec.range(), format!("@classmethod\n{indent}")));
                }
                "export" => {
                    self.edits.push((dec.range(), String::new()));
                    if self.class_depth == 0 {
                        self.exports.push(func.name.as_str().to_owned());
                    }
                }
                "private" => {
                    self.edits.push((dec.range(), String::new()));
                    if self.class_depth == 0 {
                        self.rename_with_underscore(func.name.range());
                    }
                }
                _ => {}
            }
        }
    }

    /// Replace the identifier at `range` with an underscore-prefixed copy.
    fn rename_with_underscore(&mut self, range: TextRange) {
        let original = self.src(range).to_owned();
        self.edits.push((range, format!("_{original}")));
    }

    /// Returns the source text for a range.
    fn src(&self, range: TextRange) -> &str {
        &self.source[usize::from(range.start())..usize::from(range.end())]
    }

    fn process_ann_assign(&mut self, node: &StmtAnnAssign) {
        // Only handle our synthetic annotation markers from the parser.
        let Expr::Name(ann) = node.annotation.as_ref() else {
            return;
        };
        let Some(value) = &node.value else { return };
        let name = self.src(node.target.range()).to_owned();
        let value_src = self.src(value.range()).to_owned();

        match ann.id.as_str() {
            "__let__" => {
                if self.class_depth > 0 {
                    self.edits.push((node.range(), format!("{name} = {value_src}")));
                } else {
                    self.needs_final_annotation = true;
                    self.edits.push((
                        node.range(),
                        format!("{name}: Final = {value_src}"),
                    ));
                }
            }
            "__classvar__" => {
                self.needs_classvar = true;
                self.edits.push((
                    node.range(),
                    format!("{name}: ClassVar = {value_src}"),
                ));
            }
            "__newtype__" => {
                self.needs_newtype = true;
                self.edits.push((
                    node.range(),
                    format!("{name} = NewType(\"{name}\", {value_src})"),
                ));
            }
            _ => {}
        }
    }

    fn insert_base_class(&mut self, class: &StmtClassDef, base_name: &str) {
        if let Some(args) = &class.arguments {
            let rparen = args.range().end() - TextSize::from(1);
            let insert_range = TextRange::new(rparen, rparen);
            if args.args.is_empty() && args.keywords.is_empty() {
                self.edits.push((insert_range, base_name.to_owned()));
            } else {
                self.edits.push((insert_range, format!(", {base_name}")));
            }
        } else {
            let after_name = class.name.range().end();
            let insert_range = TextRange::new(after_name, after_name);
            self.edits.push((insert_range, format!("({base_name})")));
        }
    }

    fn insert_enum_base(&mut self, class: &StmtClassDef) {
        self.insert_base_class(class, "Enum");
    }

    fn insert_protocol_base(&mut self, class: &StmtClassDef) {
        self.insert_base_class(class, "Protocol");
    }
}

impl<'src, 'ast> Visitor<'ast> for Modifiers<'src> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        match stmt {
            Stmt::ClassDef(c) => {
                self.process_class(c);
                // Walk the class body with `class_depth` incremented so nested
                // declarations are not treated as module-level for visibility purposes.
                self.class_depth += 1;
                walk_stmt(self, stmt);
                self.class_depth -= 1;
                return;
            }
            Stmt::FunctionDef(f) => {
                self.process_function(f);
            }
            Stmt::AnnAssign(a) => {
                self.process_ann_assign(a);
            }
            _ => {}
        }
        walk_stmt(self, stmt);
    }
}

#[cfg(test)]
mod tests {
    use crate::{transpile, Config};
    use indoc::indoc;

    fn check(input: &str, expected: &str) {
        assert_eq!(transpile(input, &Config::default()).unwrap(), expected);
    }

    #[test]
    fn final_class() {
        check(
            "final class Foo: ...\n",
            indoc! {"
                from typing import final
                @final
                class Foo: ...
            "},
        );
    }

    #[test]
    fn final_def() {
        check(
            indoc! {"
                class Base:
                    final def method(self): ...
            "},
            indoc! {"
                from typing import final
                class Base:
                    @final
                    def method(self): ...
            "},
        );
    }

    #[test]
    fn abstract_class() {
        check("abstract class Foo: ...\n", "class Foo: ...\n");
    }

    #[test]
    fn abstract_def() {
        check(
            indoc! {"
                class Base:
                    abstract def method(self)
            "},
            indoc! {"
                from abc import abstractmethod
                class Base:
                    @abstractmethod
                    def method(self): raise NotImplementedError
            "},
        );
    }

    #[test]
    fn override_def() {
        check(
            indoc! {"
                class Child:
                    override def method(self): ...
            "},
            indoc! {"
                from typing import override
                class Child:
                    @override
                    def method(self): ...
            "},
        );
    }

    #[test]
    fn open_class() {
        check("open class Foo: ...\n", "class Foo: ...\n");
    }

    #[test]
    fn static_def() {
        check(
            indoc! {"
                class A:
                    static def helper(): ...
            "},
            indoc! {"
                class A:
                    @staticmethod
                    def helper(): ...
            "},
        );
    }

    #[test]
    fn class_def() {
        check(
            indoc! {"
                class A:
                    class def from_str(cls, s: str): ...
            "},
            indoc! {"
                class A:
                    @classmethod
                    def from_str(cls, s: str): ...
            "},
        );
    }

    #[test]
    fn data_class() {
        check(
            "data class Point: ...\n",
            indoc! {"
                from dataclasses import dataclass
                @dataclass(slots=True)
                class Point: ...
            "},
        );
    }

    #[test]
    fn frozen_data_class() {
        check(
            "frozen data class Point: ...\n",
            indoc! {"
                from dataclasses import dataclass
                @dataclass(frozen=True, slots=True)
                class Point: ...
            "},
        );
    }

    #[test]
    fn enum_class() {
        check(
            "enum class Color: ...\n",
            indoc! {"
                from enum import Enum
                class Color(Enum): ...
            "},
        );
    }

    #[test]
    fn enum_class_with_base() {
        check(
            "enum class Color(str): ...\n",
            indoc! {"
                from enum import Enum
                class Color(str, Enum): ...
            "},
        );
    }

    #[test]
    fn nested_modifiers_in_class() {
        check(
            indoc! {"
                class Base:
                    override def foo(self): ...
                    static def bar(): ...
                    class def baz(cls): ...
            "},
            indoc! {"
                from typing import override
                class Base:
                    @override
                    def foo(self): ...
                    @staticmethod
                    def bar(): ...
                    @classmethod
                    def baz(cls): ...
            "},
        );
    }

    #[test]
    fn protocol_class() {
        check(
            "protocol Foo: ...\n",
            indoc! {"
                from typing import Protocol
                class Foo(Protocol): ...
            "},
        );
    }

    #[test]
    fn protocol_with_base() {
        check(
            "protocol Foo(Bar): ...\n",
            indoc! {"
                from typing import Protocol
                class Foo(Bar, Protocol): ...
            "},
        );
    }

    #[test]
    fn let_decl() {
        check(
            "let MAX = 100\n",
            indoc! {"
                from typing import Final
                MAX: Final = 100
            "},
        );
    }

    #[test]
    fn let_decl_in_class() {
        check(
            indoc! {"
                class A:
                    let foo = 100
            "},
            indoc! {"
                class A:
                    foo = 100
            "},
        );
    }

    #[test]
    fn let_decl_string() {
        check(
            "let NAME = \"alice\"\n",
            indoc! {"
                from typing import Final
                NAME: Final = \"alice\"
            "},
        );
    }

    #[test]
    fn class_var_decl() {
        check(
            indoc! {"
                class Foo:
                    class count = 0
            "},
            indoc! {"
                from typing import ClassVar
                class Foo:
                    count: ClassVar = 0
            "},
        );
    }

    #[test]
    fn newtype_decl() {
        check(
            "newtype UserId = int\n",
            indoc! {"
                from typing import NewType
                UserId = NewType(\"UserId\", int)
            "},
        );
    }

    #[test]
    fn export_def() {
        check(
            "export def foo(): ...\n",
            indoc! {"
                def foo(): ...
                __all__ = [\"foo\"]
            "},
        );
    }

    #[test]
    fn export_class() {
        check(
            "export class Foo: ...\n",
            indoc! {"
                class Foo: ...
                __all__ = [\"Foo\"]
            "},
        );
    }

    #[test]
    fn public_alias_for_export() {
        check(
            "public def helper(): ...\n",
            indoc! {"
                def helper(): ...
                __all__ = [\"helper\"]
            "},
        );
    }

    #[test]
    fn export_multiple_collected_into_all() {
        check(
            indoc! {"
                export def first(): ...
                def internal(): ...
                export class Second: ...
            "},
            indoc! {"
                def first(): ...
                def internal(): ...
                class Second: ...
                __all__ = [\"first\", \"Second\"]
            "},
        );
    }

    #[test]
    fn no_all_emitted_when_no_exports() {
        // A file with no export markers should not produce an `__all__` block.
        check("def foo(): ...\n", "def foo(): ...\n");
    }

    #[test]
    fn private_def() {
        check(
            "private def helper(): ...\n",
            "def _helper(): ...\n",
        );
    }

    #[test]
    fn private_class() {
        check(
            "private class Helper: ...\n",
            "class _Helper: ...\n",
        );
    }

    #[test]
    fn private_skipped_inside_class() {
        // Inside a class body, `private def`/`private class` strips the modifier
        // but does NOT rename the symbol. Method privacy is left to the
        // user's existing `_`-prefix conventions on call sites.
        check(
            indoc! {"
                class Outer:
                    private def helper(self): ...
            "},
            indoc! {"
                class Outer:
                    def helper(self): ...
            "},
        );
    }

    #[test]
    fn export_skipped_inside_class() {
        // `export` on a class member must not pollute the module-level `__all__`.
        check(
            indoc! {"
                class Outer:
                    export def helper(self): ...
            "},
            indoc! {"
                class Outer:
                    def helper(self): ...
            "},
        );
    }

    #[test]
    fn export_and_private_together_in_module() {
        check(
            indoc! {"
                export def api(): ...
                private def helper(): ...
            "},
            indoc! {"
                def api(): ...
                def _helper(): ...
                __all__ = [\"api\"]
            "},
        );
    }
}
