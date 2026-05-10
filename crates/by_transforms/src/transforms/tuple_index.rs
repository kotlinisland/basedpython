//! AST rewrite for basedpython tuple-member dot access: `expr.N` → `expr[N]`.
//!
//! The parser accepts `expr.N` where `N` is one or more decimal digits and
//! constructs an `ExprAttribute` whose `attr` is the digit string. That
//! shape is unreachable in stock python (identifiers cannot start with a
//! digit), so the attr-is-digits check is unambiguous.

use std::cell::Cell;

use ruff_python_ast::visitor::transformer::{Transformer, walk_expr};
use ruff_python_ast::{
    AtomicNodeIndex, Expr, ExprContext, ExprNumberLiteral, ExprSubscript, Int, Number, Stmt,
};
use ruff_text_size::TextRange;

pub(crate) struct TupleIndex {
    changed: Cell<bool>,
}

impl TupleIndex {
    pub(crate) fn new() -> Self {
        Self {
            changed: Cell::new(false),
        }
    }

    pub(crate) fn changed_cell(&self) -> &Cell<bool> {
        &self.changed
    }
}

impl Transformer for TupleIndex {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        ruff_python_ast::visitor::transformer::walk_stmt(self, stmt);
    }

    fn visit_expr(&self, expr: &mut Expr) {
        walk_expr(self, expr);

        let Expr::Attribute(attr) = expr else { return };
        let attr_id = attr.attr.id.as_str();
        if attr_id.is_empty() || !attr_id.bytes().all(|b| b.is_ascii_digit()) {
            return;
        }
        // strip leading zeros: `t.01` lowers to `t[1]`
        let trimmed = attr_id.trim_start_matches('0');
        let n: u64 = if trimmed.is_empty() {
            0
        } else {
            trimmed.parse().unwrap_or(0)
        };
        let value = std::mem::replace(
            attr.value.as_mut(),
            Expr::NoneLiteral(ruff_python_ast::ExprNoneLiteral {
                node_index: AtomicNodeIndex::NONE,
                range: TextRange::default(),
            }),
        );
        let slice = Expr::NumberLiteral(ExprNumberLiteral {
            node_index: AtomicNodeIndex::NONE,
            range: TextRange::default(),
            value: Number::Int(Int::from(n)),
        });
        *expr = Expr::Subscript(ExprSubscript {
            node_index: AtomicNodeIndex::NONE,
            range: TextRange::default(),
            value: Box::new(value),
            slice: Box::new(slice),
            ctx: ExprContext::Load,
            is_typeof: false,
        });
        self.changed.set(true);
    }
}

#[cfg(test)]
mod tests {
    use crate::python_passthrough::unchanged;
    use crate::{Config, transpile};

    fn check(input: &str, expected: &str) {
        assert_eq!(
            transpile(input, &Config::test_default()).unwrap(),
            crate::python_passthrough::lazify_expected(expected)
        );
    }

    #[test]
    fn literal_tuple() {
        check("x = (1, 2).0\n", "x = (1, 2)[0]\n");
    }

    #[test]
    fn name_tuple() {
        check("y = t.1\n", "y = t[1]\n");
    }

    #[test]
    fn multi_digit() {
        check("z = t.10\n", "z = t[10]\n");
    }

    #[test]
    fn nested() {
        check("v = (1, (2, 3)).1.0\n", "v = (1, (2, 3))[1][0]\n");
    }

    #[test]
    fn followed_by_call() {
        check("r = t.0()\n", "r = t[0]()\n");
    }

    #[test]
    fn followed_by_subscript() {
        check("r = t.0[k]\n", "r = t[0][k]\n");
    }

    #[test]
    fn followed_by_attribute() {
        check("r = t.0.name\n", "r = t[0].name\n");
    }

    #[test]
    fn leading_zero() {
        check("x = t.01\n", "x = t[1]\n");
    }

    #[test]
    fn all_zero() {
        check("x = t.00\n", "x = t[0]\n");
    }

    #[test]
    fn python_unchanged() {
        unchanged("x = (1, 2)[0]\n");
    }

    #[test]
    fn float_literal_unchanged() {
        unchanged("x = 1.0\n");
    }
}
