//! reverse of `crate::transforms::compat`:
//!   `datetime.timezone.utc` → `datetime.UTC`
//!   `sys.exc_info()[1]`     → `sys.exception()`
//!
//! `2 ** (x)` → `math.exp2(x)` is not reversed: `2 ** x` is too common
//! a user expression to safely treat as generated compat code

use ruff_diagnostics::{Edit, Fix};
use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

pub(crate) struct CompatReverse {
    pub(crate) edits: Vec<Fix>,
}

impl CompatReverse {
    pub(crate) fn new() -> Self {
        Self { edits: Vec::new() }
    }
}

impl<'ast> Visitor<'ast> for CompatReverse {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        match expr {
            // `datetime.timezone.utc` → `datetime.UTC`
            Expr::Attribute(outer)
                if outer.attr.id.as_str() == "utc"
                    && matches!(
                        outer.value.as_ref(),
                        Expr::Attribute(inner)
                            if inner.attr.id.as_str() == "timezone"
                                && matches!(inner.value.as_ref(), Expr::Name(n) if n.id.as_str() == "datetime")
                    ) =>
            {
                self.edits.push(Fix::safe_edit(Edit::range_replacement(
                    "datetime.UTC".to_owned(),
                    expr.range(),
                )));
            }

            // `sys.exc_info()[1]` → `sys.exception()`
            Expr::Subscript(sub)
                if matches!(&sub.slice.as_ref(), Expr::NumberLiteral(n) if n.value.as_int().is_some_and(|v| v.as_u64() == Some(1)))
                    && matches!(
                        sub.value.as_ref(),
                        Expr::Call(call)
                            if call.arguments.args.is_empty()
                                && call.arguments.keywords.is_empty()
                                && matches!(
                                    call.func.as_ref(),
                                    Expr::Attribute(a)
                                        if a.attr.id.as_str() == "exc_info"
                                            && matches!(a.value.as_ref(), Expr::Name(n) if n.id.as_str() == "sys")
                                )
                    ) =>
            {
                self.edits.push(Fix::safe_edit(Edit::range_replacement(
                    "sys.exception()".to_owned(),
                    expr.range(),
                )));
            }

            _ => {}
        }
        walk_expr(self, expr);
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
    fn datetime_utc() {
        check(
            "import datetime\ntz = datetime.timezone.utc\n",
            "import datetime\ntz = datetime.UTC\n",
        );
    }

    #[test]
    fn sys_exc_info() {
        check(
            indoc! {"
                import sys
                err = sys.exc_info()[1]
            "},
            indoc! {"
                import sys
                err = sys.exception()
            "},
        );
    }

    #[test]
    fn other_datetime_unchanged() {
        check(
            "import datetime\ntz = datetime.timezone.utc_offset\n",
            "import datetime\ntz = datetime.timezone.utc_offset\n",
        );
    }
}
