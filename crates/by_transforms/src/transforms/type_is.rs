//! AST pass that rewrites `a is T` narrowing-predicate syntax in type
//! positions to `typing.TypeIs[T]`.
//!
//! `def f(a) -> a is int: ...` → `def f(a) -> TypeIs[int]: ...`
//!
//! basedpython surface syntax for narrowing predicates names the parameter
//! being narrowed alongside its target type. The runtime semantics are
//! identical to PEP 742 `TypeIs[T]`; the parameter name is lost in
//! lowering since `TypeIs` doesn't carry it.
//!
//! traversal is delegated to [`type_expr_walker`] (with `types = None` —
//! value-position `a is T` is *not* a type expression here; it's the
//! basedpython surface form for `isinstance(a, T)`, owned by
//! `identity_swap`). running before `identity_swap` in the `AstPass` list so
//! type-position rewrites win the first-wins overlap dedup

use ruff_python_ast::{
    AtomicNodeIndex, CmpOp, Expr, ExprContext, ExprName, ExprSubscript, ModModule, Stmt, name::Name,
};
use ruff_text_size::{Ranged, TextRange};

use super::ast_driver::{AstPass, PassContext, render_expr};
use super::type_expr_walker::{Recurse, TypeExprVisitor, TypePos, walk_type_positions};

pub(crate) struct TypeIs;

impl TypeIs {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl AstPass for TypeIs {
    fn run(&self, module: &mut ModModule, ctx: &mut PassContext) {
        let mut state = State {
            edits: Vec::new(),
            needs_import: false,
        };
        let body: &[Stmt] = &module.body;
        walk_type_positions(body, None, &mut state);
        ctx.text_edits.extend(state.edits);
        if state.needs_import {
            // typing.TypeIs landed in 3.13 (PEP 742). on older runtimes the
            // typing_redirect pass switches the import to typing_extensions
            ctx.required_imports
                .push("from typing import TypeIs".to_owned());
        }
    }
}

struct State {
    edits: Vec<(TextRange, String)>,
    needs_import: bool,
}

impl TypeExprVisitor for State {
    fn visit(&mut self, expr: &Expr, _pos: TypePos) -> Recurse {
        if let Expr::Compare(c) = expr
            && c.ops.len() == 1
            && matches!(c.ops[0], CmpOp::Is)
            && matches!(c.left.as_ref(), Expr::Name(_))
            && let Some(target) = c.comparators.first()
        {
            let new_node = Expr::Subscript(ExprSubscript {
                node_index: AtomicNodeIndex::NONE,
                range: TextRange::default(),
                value: Box::new(Expr::Name(ExprName {
                    node_index: AtomicNodeIndex::NONE,
                    range: TextRange::default(),
                    id: Name::from("TypeIs"),
                    ctx: ExprContext::Load,
                })),
                slice: Box::new(target.clone()),
                ctx: ExprContext::Load,
                is_typeof: false,
            });
            self.needs_import = true;
            self.edits.push((expr.range(), render_expr(&new_node)));
        }
        Recurse::Descend
    }
}

#[cfg(test)]
mod tests {
    use crate::python_passthrough::unchanged;
    use crate::{Config, PythonVersion, transpile};
    use indoc::indoc;

    fn check(input: &str, expected: &str) {
        assert_eq!(
            transpile(input, &Config::test_default()).unwrap(),
            crate::python_passthrough::lazify_expected(expected)
        );
    }

    fn check_py312(input: &str, expected: &str) {
        let config = Config {
            min_version: PythonVersion::PY312,
            ..Config::test_default()
        };
        assert_eq!(
            transpile(input, &config).unwrap(),
            crate::python_passthrough::lazify_expected(expected)
        );
    }

    #[test]
    fn simple() {
        check(
            "def f(a) -> a is int: ...\n",
            indoc! {"
                from typing_extensions import TypeIs
                def f(a) -> TypeIs[int]: ...
            "},
        );
    }

    #[test]
    fn other_param_name() {
        check(
            "def is_str(x) -> x is str: ...\n",
            indoc! {"
                from typing_extensions import TypeIs
                def is_str(x) -> TypeIs[str]: ...
            "},
        );
    }

    #[test]
    fn body_is_value_unchanged() {
        unchanged("def f(a):\n    return a is None\n");
    }

    #[test]
    fn predicate_in_param_annotation() {
        // walker now exposes the predicate syntax in any type position. param
        // annotations are unusual but consistent — `x: a is int` lowers
        check(
            "def f(x: a is int): ...\n",
            indoc! {"
                from typing_extensions import TypeIs
                def f(x: TypeIs[int]): ...
            "},
        );
    }

    #[test]
    fn predicate_in_ann_assign() {
        check(
            "b: a is int\n",
            indoc! {"
                from typing_extensions import TypeIs
                b: TypeIs[int]
            "},
        );
    }

    #[test]
    fn predicate_in_type_alias_rhs() {
        check_py312(
            "type Pred = a is int\n",
            indoc! {"
                from typing_extensions import TypeIs
                type Pred = TypeIs[int]
            "},
        );
    }
}
