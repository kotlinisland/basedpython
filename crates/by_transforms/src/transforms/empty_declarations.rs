//! AST pass: basedpython grammar extension — `class Foo` (no colon, no body)
//! parses as a class with an empty body. This pass appends `: ...` to the
//! source so the standard Python form `class Foo: ...` survives the
//! downstream pipeline

use std::cell::RefCell;

use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{ModModule, Stmt, StmtClassDef};
use ruff_text_size::{Ranged, TextRange};

use super::ast_driver::{AstPass, PassContext};

pub(crate) struct EmptyDeclarations;

impl EmptyDeclarations {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl AstPass for EmptyDeclarations {
    fn run(&self, module: &mut ModModule, ctx: &mut PassContext) {
        let mut state = State {
            edits: RefCell::new(Vec::new()),
        };
        for stmt in &module.body {
            state.visit_stmt(stmt);
        }
        ctx.text_edits.extend(state.edits.into_inner());
    }
}

struct State {
    edits: RefCell<Vec<(TextRange, String)>>,
}

impl State {
    fn process_class(&mut self, class: &StmtClassDef) {
        if !class.body.is_empty() {
            return;
        }
        let pos = class.range().end();
        self.edits
            .borrow_mut()
            .push((TextRange::new(pos, pos), ": ...".to_owned()));
    }
}

impl<'ast> Visitor<'ast> for State {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        if let Stmt::ClassDef(c) = stmt {
            self.process_class(c);
        }
        walk_stmt(self, stmt);
    }
}

#[cfg(test)]
mod tests {
    use crate::{Config, transpile};
    use indoc::indoc;

    fn check(input: &str, expected: &str) {
        assert_eq!(
            transpile(input, &Config::test_default()).unwrap(),
            crate::python_passthrough::lazify_expected(expected)
        );
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
        check("@dataclass\nclass Foo\n", "@dataclass\nclass Foo: ...\n");
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
