//! basedpython grammar extension: `class Foo` (no colon, no body) is parsed
//! as a class with an empty body. this transform expands it back to the
//! standard Python form `class Foo: ...`
//!
//! the parser change lives in `crates/ruff_python_parser/src/parser/statement.rs`

use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{Stmt, StmtClassDef};
use ruff_text_size::{Ranged, TextRange};

pub struct EmptyDeclarations {
    pub edits: Vec<(TextRange, String)>,
}

impl EmptyDeclarations {
    pub fn new() -> Self {
        Self { edits: Vec::new() }
    }

    fn process_class(&mut self, class: &StmtClassDef) {
        if !class.body.is_empty() {
            return;
        }
        // append `: ...` after the class header. the class node range ends
        // right after the name (or `]` of type params, or `)` of arguments)
        // when there's no body, so a zero-width insert at `class.range().end()`
        // lands in the right spot
        let pos = class.range().end();
        self.edits
            .push((TextRange::new(pos, pos), ": ...".to_owned()));
    }
}

impl<'ast> Visitor<'ast> for EmptyDeclarations {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        if let Stmt::ClassDef(c) = stmt {
            self.process_class(c);
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
    fn bare_empty_class() {
        check("class Foo\n", "class Foo: ...\n");
    }

    #[test]
    fn empty_class_with_args() {
        check("class Foo(Base)\n", "class Foo(Base): ...\n");
    }

    #[test]
    fn empty_class_with_empty_args() {
        check("class Foo()\n", "class Foo(): ...\n");
    }

    #[test]
    fn empty_class_coexists_with_generics_polyfill() {
        // on 3.10 the generics polyfill rewrites `class Foo[T]` into
        // `class Foo(Generic[_T])` — and the empty-decl transform appends
        // `: ...` at the end of the original class header
        check(
            "class Foo[T]\n",
            indoc! {"
                from typing import TypeVar, Generic
                _T = TypeVar(\"_T\")
                class Foo(Generic[_T]): ...
            "},
        );
    }

    #[test]
    fn nonempty_class_unchanged() {
        check("class Foo:\n    x: int\n", "class Foo:\n    x: int\n");
    }

    #[test]
    fn nested_empty_class() {
        // empty class inside a class body
        check(
            indoc! {"
                class Outer:
                    class Inner
            "},
            indoc! {"
                class Outer:
                    class Inner: ...
            "},
        );
    }

    #[test]
    fn decorated_empty_class() {
        check(
            "@dataclass\nclass Foo\n",
            "@dataclass\nclass Foo: ...\n",
        );
    }

    #[test]
    fn multiple_empty_classes() {
        check(
            indoc! {"
                class A
                class B(A)
            "},
            indoc! {"
                class A: ...
                class B(A): ...
            "},
        );
    }
}
