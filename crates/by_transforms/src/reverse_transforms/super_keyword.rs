//! reverse of `crate::transforms::super_keyword`:
//!   `super()`           → `super`
//!   `super(T, self)`    → `super[T]`
//!
//! the targeted form matches the canonical two-arg call shape only — pivot
//! is any expression, owner is a name (typically `self`). the lowered
//! `super(C.__mro__[…], self)` form is **not** reversed: that shape is
//! intentionally preserved so it round-trips through `reverse_transpile`
//! unchanged

use ruff_diagnostics::{Edit, Fix};
use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::{Ranged, TextRange};

pub(crate) struct SuperKeywordReverse<'src> {
    source: &'src str,
    pub(crate) edits: Vec<Fix>,
}

impl<'src> SuperKeywordReverse<'src> {
    pub(crate) fn new(source: &'src str) -> Self {
        Self {
            source,
            edits: Vec::new(),
        }
    }

    fn src(&self, range: TextRange) -> &str {
        &self.source[usize::from(range.start())..usize::from(range.end())]
    }

    fn is_super_name(expr: &Expr) -> bool {
        matches!(expr, Expr::Name(n) if n.id.as_str() == "super")
    }

    fn rewrite_call(&self, call: &ruff_python_ast::ExprCall) -> Option<String> {
        if !Self::is_super_name(&call.func) {
            return None;
        }
        let args = &call.arguments;
        if !args.keywords.is_empty() {
            return None;
        }
        match args.args.as_ref() {
            [] => Some("super".to_owned()),
            [pivot, owner] => {
                if !matches!(owner, Expr::Name(_)) {
                    return None;
                }
                if pivot_uses_mro(pivot) {
                    return None;
                }
                Some(format!("super[{}]", self.src(pivot.range())))
            }
            _ => None,
        }
    }
}

/// whether `pivot` references `.__mro__` anywhere — indicates the lowered
/// `super(C.__mro__[…], self)` shape, which we leave unchanged
fn pivot_uses_mro(pivot: &Expr) -> bool {
    match pivot {
        Expr::Attribute(a) => a.attr.id.as_str() == "__mro__" || pivot_uses_mro(&a.value),
        Expr::Subscript(s) => pivot_uses_mro(&s.value) || pivot_uses_mro(&s.slice),
        Expr::BinOp(b) => pivot_uses_mro(&b.left) || pivot_uses_mro(&b.right),
        Expr::Call(c) => {
            pivot_uses_mro(&c.func)
                || c.arguments.args.iter().any(pivot_uses_mro)
                || c.arguments
                    .keywords
                    .iter()
                    .any(|kw| pivot_uses_mro(&kw.value))
        }
        _ => false,
    }
}

impl<'ast> Visitor<'ast> for SuperKeywordReverse<'_> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let Expr::Call(call) = expr
            && let Some(replacement) = self.rewrite_call(call)
        {
            self.edits.push(Fix::safe_edit(Edit::range_replacement(
                replacement,
                expr.range(),
            )));
            return;
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
    fn zero_arg_super_attr() {
        check(
            indoc! {"
                class B(A):
                    def f(self):
                        super().x
            "},
            indoc! {"
                class B(A):
                    def f(self):
                        super.x
            "},
        );
    }

    #[test]
    fn zero_arg_super_call() {
        check(
            indoc! {"
                class B(A):
                    def f(self):
                        super().f()
            "},
            indoc! {"
                class B(A):
                    def f(self):
                        super.f()
            "},
        );
    }

    #[test]
    fn two_arg_super_attr() {
        check(
            indoc! {"
                class C(A, B):
                    def f(self):
                        super(A, self).x
            "},
            indoc! {"
                class C(A, B):
                    def f(self):
                        super[A].x
            "},
        );
    }

    #[test]
    fn two_arg_super_call() {
        check(
            indoc! {"
                class C(A, B):
                    def f(self):
                        super(B, self).f()
            "},
            indoc! {"
                class C(A, B):
                    def f(self):
                        super[B].f()
            "},
        );
    }

    #[test]
    fn mro_form_unchanged() {
        // the lowered `super(C.__mro__[…], self)` shape is intentionally NOT
        // reversed; preserved verbatim for round-trip stability
        check(
            "class C(A, B):\n    def f(self):\n        super(C.__mro__[C.__mro__.index(B) - 1], self).x\n",
            "class C(A, B):\n    def f(self):\n        super(C.__mro__[C.__mro__.index(B) - 1], self).x\n",
        );
    }

    #[test]
    fn one_arg_super_unchanged() {
        // unbound super (single-arg form) has no sugar; leave alone
        check(
            "class B(A):\n    def f(self):\n        super(B).x\n",
            "class B(A):\n    def f(self):\n        super(B).x\n",
        );
    }
}
