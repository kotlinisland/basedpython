//! AST rewrite for the `<value> cast <type>` infix.
//!
//! The parser models `<value> cast <type>` as an `ExprCall` carrying
//! `is_cast: true` and arguments `[type_expr, value_expr]`. The AST pass
//! clears the flag and leaves the call shape unchanged, because the
//! call's arguments already encode the target form `cast(<type>, <value>)`.
//! Nested cases (`a cast int cast str`) compose naturally — post-order
//! traversal clears the inner flag first, then the outer.

use std::cell::Cell;

use ruff_python_ast::Expr;
use ruff_python_ast::visitor::transformer::{Transformer, walk_expr};

pub(crate) struct CastFold {
    changed: Cell<bool>,
    ever_changed: Cell<bool>,
}

impl CastFold {
    pub(crate) fn new() -> Self {
        Self {
            changed: Cell::new(false),
            ever_changed: Cell::new(false),
        }
    }

    pub(crate) fn changed_cell(&self) -> &Cell<bool> {
        &self.changed
    }

    pub(crate) fn ever_changed(&self) -> bool {
        self.ever_changed.get()
    }
}

impl Transformer for CastFold {
    fn visit_expr(&self, expr: &mut Expr) {
        // post-order: inner `cast` rewritten first
        walk_expr(self, expr);

        if let Expr::Call(c) = expr
            && c.is_cast
        {
            c.is_cast = false;
            self.changed.set(true);
            self.ever_changed.set(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::python_passthrough::unchanged;
    use crate::{Config, transpile};
    use indoc::indoc;

    fn check(input: &str, expected: &str) {
        assert_eq!(transpile(input, &Config::test_default()).unwrap(), expected);
    }

    #[test]
    fn simple() {
        check(
            indoc! {"
                a = 1
                b = a cast int
            "},
            indoc! {"
                from typing import cast
                a = 1
                b = cast(int, a)
            "},
        );
    }

    #[test]
    fn cast_to_union() {
        check(
            indoc! {"
                b = a cast int | str
            "},
            indoc! {"
                from typing import cast
                b = cast(int | str, a)
            "},
        );
    }

    #[test]
    fn cast_in_call_argument() {
        check(
            indoc! {"
                f(a cast int)
            "},
            indoc! {"
                from typing import cast
                f(cast(int, a))
            "},
        );
    }

    #[test]
    fn cast_identifier_is_passthrough_in_python() {
        unchanged("cast = 5\n");
    }

    #[test]
    fn regular_cast_call_is_passthrough_in_python() {
        unchanged("from typing import cast\nb = cast(int, a)\n");
    }
}
