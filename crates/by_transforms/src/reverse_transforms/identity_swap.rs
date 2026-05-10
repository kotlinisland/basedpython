//! reverse of `crate::transforms::identity_swap`:
//!   `x is y`            → `x === y`
//!   `x is not y`        → `x === not y`  (not symmetric — see below)
//!   `isinstance(x, y)`  → `x is y`
//!
//! basedpython's `is` is the instance check, so a Python `is` round-trips
//! to `===` and an `isinstance` call round-trips to `is`. Note `is not`
//! reverses incompletely — there's no concise basedpython spelling

use ruff_diagnostics::{Edit, Fix};
use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_python_ast::{CmpOp, Expr, Stmt};
use ruff_text_size::{Ranged, TextRange, TextSize};

pub(crate) struct IdentitySwapReverse<'src> {
    source: &'src str,
    pub(crate) edits: Vec<Fix>,
}

impl<'src> IdentitySwapReverse<'src> {
    pub(crate) fn new(source: &'src str) -> Self {
        Self {
            source,
            edits: Vec::new(),
        }
    }

    fn src(&self, range: TextRange) -> &str {
        &self.source[usize::from(range.start())..usize::from(range.end())]
    }

    fn process_compare(&mut self, c: &ruff_python_ast::ExprCompare) {
        let mut lhs_end = c.left.range().end();
        for (op, rhs) in c.ops.iter().zip(c.comparators.iter()) {
            let rhs_start = rhs.range().start();
            let between = &self.source[usize::from(lhs_end)..usize::from(rhs_start)];
            if matches!(op, CmpOp::Is) && between.trim() == "is" {
                if let Some(pos) = between.find("is") {
                    let op_start = lhs_end + TextSize::try_from(pos).unwrap();
                    let op_range = TextRange::new(op_start, op_start + TextSize::from(2u32));
                    self.edits.push(Fix::safe_edit(Edit::range_replacement(
                        "===".to_owned(),
                        op_range,
                    )));
                }
            }
            lhs_end = rhs.range().end();
        }
    }

    fn process_call(&mut self, call: &ruff_python_ast::ExprCall) {
        // detect `isinstance(x, y)` with exactly 2 positional args and no
        // keyword args. anything else stays as-is to avoid losing semantics
        if !matches!(call.func.as_ref(), Expr::Name(n) if n.id.as_str() == "isinstance") {
            return;
        }
        if !call.arguments.keywords.is_empty() {
            return;
        }
        if call.arguments.args.len() != 2 {
            return;
        }
        let x = &call.arguments.args[0];
        let y = &call.arguments.args[1];
        let x_src = self.src(x.range()).to_owned();
        let y_src = self.src(y.range()).to_owned();
        self.edits.push(Fix::safe_edit(Edit::range_replacement(
            format!("{x_src} is {y_src}"),
            call.range(),
        )));
    }
}

impl<'ast> Visitor<'ast> for IdentitySwapReverse<'_> {
    fn visit_expr(&mut self, expr: &'ast Expr) {
        match expr {
            Expr::Compare(c) => self.process_compare(c),
            Expr::Call(call) => self.process_call(call),
            _ => {}
        }
        walk_expr(self, expr);
    }

    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        walk_stmt(self, stmt);
    }
}

#[cfg(test)]
mod tests {
    use crate::{Config, reverse_transpile};
    use indoc::indoc;

    fn check(input: &str, expected: &str) {
        assert_eq!(
            reverse_transpile(input, &Config::test_default()).unwrap(),
            expected
        );
    }

    #[test]
    fn isinstance_to_is() {
        check(
            indoc! {"
                if isinstance(x, int):
                    pass
            "},
            indoc! {"
                if x is int:
                    pass
            "},
        );
    }

    #[test]
    fn not_isinstance_to_is_not() {
        check(
            indoc! {"
                if not isinstance(x, str):
                    pass
            "},
            indoc! {"
                if not x is str:
                    pass
            "},
        );
    }

    #[test]
    fn unrelated_call_left_alone() {
        check("y = some(x, int)\n", "y = some(x, int)\n");
    }
}
