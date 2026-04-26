use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_python_ast::{Expr, Operator, Stmt};
use ruff_text_size::{Ranged, TextRange};

/// rewrites `a ?? b` to `a if a is not None else b`
pub struct NoneCoalesce<'src> {
    source: &'src str,
    pub edits: Vec<(TextRange, String)>,
}

impl<'src> NoneCoalesce<'src> {
    pub fn new(source: &'src str) -> Self {
        Self { source, edits: Vec::new() }
    }

    fn src(&self, range: TextRange) -> &str {
        &self.source[usize::from(range.start())..usize::from(range.end())]
    }
}

fn expand_none_chain(expr: &Expr, source: &str) -> Option<String> {
    let (form, guards) = super::none_chain::expand_chain(expr, source)?;
    Some(super::none_chain::build_expansion(&guards, &form, "_t"))
}

impl<'src, 'ast> Visitor<'ast> for NoneCoalesce<'src> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let Expr::BinOp(b) = expr
            && matches!(b.op, Operator::Coalesce)
        {
            let rhs = self.src(b.right.range());
            let replacement = match expand_none_chain(&b.left, self.source) {
                Some(expanded) => format!("_t if (_t := {expanded}) is not None else {rhs}"),
                None => {
                    let lhs = self.src(b.left.range());
                    format!("{lhs} if {lhs} is not None else {rhs}")
                }
            };
            self.edits.push((expr.range(), replacement));
            return;
        }
        walk_expr(self, expr);
    }
}

#[cfg(test)]
mod tests {
    use crate::{transpile, Config};

    fn check(input: &str, expected: &str) {
        assert_eq!(transpile(input, &Config::default()).unwrap(), expected);
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
}
