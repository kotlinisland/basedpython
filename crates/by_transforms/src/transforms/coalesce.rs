use ruff_diagnostics::{Edit, Fix};
use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_python_ast::{Expr, Operator, Stmt};
use ruff_text_size::{Ranged, TextRange};

use super::ast_driver::{PassContext, TypeAwarePass};
use crate::type_info::TypeInfo;

/// rewrites `a ?? b` to `a if a is not None else b`
pub(crate) struct NoneCoalesce<'src> {
    source: &'src str,
    pub(crate) edits: Vec<Fix>,
}

impl<'src> NoneCoalesce<'src> {
    pub(crate) fn new(source: &'src str) -> Self {
        Self {
            source,
            edits: Vec::new(),
        }
    }

    fn src(&self, range: TextRange) -> &str {
        &self.source[usize::from(range.start())..usize::from(range.end())]
    }
}

fn expand_none_chain(expr: &Expr, source: &str) -> Option<String> {
    let (form, guards) = super::none_chain::expand_chain(expr, source)?;
    Some(super::none_chain::build_expansion(&guards, &form, "_t"))
}

impl<'ast> Visitor<'ast> for NoneCoalesce<'_> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let Expr::BinOp(b) = expr
            && matches!(b.op, Operator::Coalesce)
        {
            // a literal LHS that is statically known to be non-None has no
            // need for the `is not None` guard. emit just the LHS — python's
            // own constant-fold will warn on `1 is not None` otherwise
            if is_non_none_literal(&b.left) {
                let lhs = self.src(b.left.range());
                self.edits.push(Fix::safe_edit(Edit::range_replacement(
                    lhs.to_owned(),
                    expr.range(),
                )));
                return;
            }
            let rhs = self.src(b.right.range());
            let replacement = match expand_none_chain(&b.left, self.source) {
                Some(expanded) => format!("_t if (_t := {expanded}) is not None else {rhs}"),
                None => {
                    let lhs = self.src(b.left.range());
                    if is_trivially_pure(&b.left) {
                        format!("{lhs} if {lhs} is not None else {rhs}")
                    } else {
                        // hoist to walrus so LHS is evaluated exactly once
                        format!("_t if (_t := {lhs}) is not None else {rhs}")
                    }
                }
            };
            self.edits.push(Fix::safe_edit(Edit::range_replacement(
                replacement,
                expr.range(),
            )));
            return;
        }
        walk_expr(self, expr);
    }
}

/// Expressions whose evaluation has no side effects and whose value is
/// stable across two reads — safe to re-emit in both branches of the
/// rewrite without changing program semantics
fn is_trivially_pure(expr: &Expr) -> bool {
    match expr {
        Expr::Name(_)
        | Expr::NumberLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BytesLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral(_)
        | Expr::EllipsisLiteral(_) => true,
        Expr::UnaryOp(u) => is_trivially_pure(&u.operand),
        _ => false,
    }
}

/// Literals whose value cannot be `None`. `??` on these is a no-op and the
/// `is not None` rewrite would otherwise produce a `SyntaxWarning` from
/// CPython's constant-folder
fn is_non_none_literal(expr: &Expr) -> bool {
    match expr {
        Expr::NumberLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BytesLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::FString(_) => true,
        // `-1` / `+1` etc. — the inner literal still isn't None
        Expr::UnaryOp(u) => is_non_none_literal(&u.operand),
        _ => false,
    }
}

pub(crate) struct NoneCoalescePass<'src> {
    source: &'src str,
}

impl<'src> NoneCoalescePass<'src> {
    pub(crate) fn new(source: &'src str) -> Self {
        Self { source }
    }
}

impl TypeAwarePass for NoneCoalescePass<'_> {
    fn run(&self, stmts: &[Stmt], _types: &dyn TypeInfo, ctx: &mut PassContext) {
        let mut inner = NoneCoalesce::new(self.source);
        for stmt in stmts {
            inner.visit_stmt(stmt);
        }
        for fix in inner.edits {
            for edit in fix.edits() {
                let range = edit.range();
                let repl = edit.content().unwrap_or_default().to_owned();
                ctx.text_edits.push((range, repl));
            }
        }
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
    fn basic_coalesce() {
        check("x = a ?? b\n", "x = a if a is not None else b\n");
    }

    #[test]
    fn coalesce_with_optional_chain() {
        check(
            indoc::indoc! {"
                def f(a):
                    a?.a.b ?? 1
            "},
            indoc::indoc! {"
                def f(a):
                    _t if (_t := None if a is None else a.a.b) is not None else 1
            "},
        );
    }

    #[test]
    fn python_unchanged() {
        let src = "x = a if a is not None else b\n";
        unchanged(src);
    }

    #[test]
    fn call_lhs_hoisted_to_walrus_single_eval() {
        check(
            "x = f() ?? 1\n",
            "x = _t if (_t := f()) is not None else 1\n",
        );
    }

    #[test]
    fn attr_lhs_hoisted_to_walrus_single_eval() {
        check(
            "x = a.b ?? 1\n",
            "x = _t if (_t := a.b) is not None else 1\n",
        );
    }

    #[test]
    fn literal_lhs_collapses() {
        check("x = 1 ?? 2\n", "x = 1\n");
        check("x = (1 ?? 2) + 3\n", "x = (1) + 3\n");
        check("x = -5 ?? 2\n", "x = -5\n");
        check("x = \"a\" ?? \"b\"\n", "x = \"a\"\n");
    }
}
