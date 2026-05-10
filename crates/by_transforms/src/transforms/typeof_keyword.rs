//! AST rewrite for the `typeof X` keyword.
//!
//! The parser models `typeof X` as `ExprSubscript { is_typeof: true,
//! value: Name("typeof"), slice: X }`. The AST pass rewrites it to
//! `ExprSubscript { is_typeof: false, value: Name("TypeOf"), slice: X }`
//! so the [`Generator`](`ruff_python_codegen::Generator`) emits
//! `TypeOf[X]`. Nested `typeof` operands rewrite first (post-order),
//! so `typeof typeof X` lowers to `TypeOf[TypeOf[X]]`.

use std::cell::Cell;

use ruff_python_ast::name::Name;
use ruff_python_ast::visitor::transformer::{Transformer, walk_expr};
use ruff_python_ast::{Expr, ExprContext, ExprName};
use ruff_text_size::TextRange;

pub(crate) struct TypeofFold {
    changed: Cell<bool>,
    ever_changed: Cell<bool>,
}

impl TypeofFold {
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

impl Transformer for TypeofFold {
    fn visit_expr(&self, expr: &mut Expr) {
        // post-order: nested `typeof` rewrites first
        walk_expr(self, expr);

        if let Expr::Subscript(s) = expr
            && s.is_typeof
        {
            s.is_typeof = false;
            *s.value = Expr::Name(ExprName {
                node_index: ruff_python_ast::AtomicNodeIndex::NONE,
                range: TextRange::default(),
                id: Name::from("TypeOf"),
                ctx: ExprContext::Load,
            });
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
        assert_eq!(
            transpile(input, &Config::test_default()).unwrap(),
            crate::python_passthrough::lazify_expected(expected)
        );
    }

    #[test]
    fn simple() {
        check(
            indoc! {"
                b: int = 1
                a: typeof b = 1
            "},
            indoc! {"
                from ty_extensions import TypeOf
                b: int = 1
                a: TypeOf[b] = 1
            "},
        );
    }

    #[test]
    fn typeof_attribute() {
        check(
            indoc! {"
                a: typeof obj.field = 1
            "},
            indoc! {"
                from ty_extensions import TypeOf
                a: TypeOf[obj.field] = 1
            "},
        );
    }

    #[test]
    fn typeof_in_union() {
        check(
            indoc! {"
                a: typeof b | int = 1
            "},
            indoc! {"
                from ty_extensions import TypeOf
                a: TypeOf[b] | int = 1
            "},
        );
    }

    #[test]
    fn typeof_in_function_signature() {
        check(
            indoc! {"
                def f(x: typeof y) -> typeof z: ...
            "},
            indoc! {"
                from ty_extensions import TypeOf
                def f(x: TypeOf[y]) -> TypeOf[z]:
                    ...
            "},
        );
    }

    #[test]
    fn typeof_identifier_is_passthrough_in_python() {
        unchanged("typeof = 5\n");
    }
}
