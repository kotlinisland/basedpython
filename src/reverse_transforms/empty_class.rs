//! Reverse of `crate::transforms::empty_declarations`:
//!   `class Foo: ...` → `class Foo`
//!
//! Only fires when the body is exactly a single ellipsis expression statement,
//! which is what the forward transform emits. `class Foo: pass` and other
//! "trivially empty" forms are intentionally left alone — they're not what
//! the forward produces and rewriting them would lose author intent.

use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{Expr, Stmt, StmtClassDef};
use ruff_text_size::{Ranged, TextRange, TextSize};

pub struct EmptyClass {
    pub edits: Vec<(TextRange, String)>,
}

impl EmptyClass {
    pub fn new() -> Self {
        Self { edits: Vec::new() }
    }

    fn process_class(&mut self, class: &StmtClassDef) {
        // Body must be exactly one statement: a bare ellipsis expression.
        let [Stmt::Expr(expr_stmt)] = class.body.as_slice() else {
            return;
        };
        if !matches!(expr_stmt.value.as_ref(), Expr::EllipsisLiteral(_)) {
            return;
        }

        // Header end = end of the last header element (name / type_params / arguments).
        let mut header_end: TextSize = class.name.range().end();
        if let Some(tp) = &class.type_params {
            header_end = header_end.max(tp.range().end());
        }
        if let Some(args) = &class.arguments {
            header_end = header_end.max(args.range().end());
        }

        // Drop everything from the end of the header to the end of the class
        // body — the `:`, any whitespace/newline/indent, and the `...` itself.
        let body_end = class.range().end();
        if body_end > header_end {
            self.edits
                .push((TextRange::new(header_end, body_end), String::new()));
        }
    }
}

impl<'ast> Visitor<'ast> for EmptyClass {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        if let Stmt::ClassDef(c) = stmt {
            self.process_class(c);
        }
        walk_stmt(self, stmt);
    }
}

#[cfg(test)]
mod tests {
    use crate::{Config, reverse_transpile};
    use indoc::indoc;

    fn check(input: &str, expected: &str) {
        assert_eq!(reverse_transpile(input, &Config::default()).unwrap(), expected);
    }

    #[test]
    fn single_line_ellipsis() {
        check("class Foo: ...\n", "class Foo\n");
    }

    #[test]
    fn with_base_class() {
        check("class Foo(Base): ...\n", "class Foo(Base)\n");
    }

    #[test]
    fn with_empty_parens() {
        check("class Foo(): ...\n", "class Foo()\n");
    }

    #[test]
    fn multiline_ellipsis_body() {
        check(
            indoc! {"
                class Foo:
                    ...
            "},
            "class Foo\n",
        );
    }

    #[test]
    fn pass_body_unchanged() {
        // Forward never emits `pass`; leave it alone to preserve author intent.
        check(
            indoc! {"
                class Foo:
                    pass
            "},
            indoc! {"
                class Foo:
                    pass
            "},
        );
    }

    #[test]
    fn nonempty_body_unchanged() {
        check(
            indoc! {"
                class Foo:
                    x: int
            "},
            indoc! {"
                class Foo:
                    x: int
            "},
        );
    }

    #[test]
    fn nested_empty_class() {
        check(
            indoc! {"
                class Outer:
                    class Inner: ...
            "},
            indoc! {"
                class Outer:
                    class Inner
            "},
        );
    }

    #[test]
    fn decorated_empty_class() {
        check("@dataclass\nclass Foo: ...\n", "@dataclass\nclass Foo\n");
    }

    #[test]
    fn multiple_empty_classes() {
        check(
            indoc! {"
                class A: ...
                class B(A): ...
            "},
            indoc! {"
                class A
                class B(A)
            "},
        );
    }
}
