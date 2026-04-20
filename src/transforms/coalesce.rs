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

impl<'src, 'ast> Visitor<'ast> for NoneCoalesce<'src> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let Expr::BinOp(b) = expr
            && matches!(b.op, Operator::Coalesce)
        {
            let lhs = self.src(b.left.range());
            let rhs = self.src(b.right.range());
            self.edits.push((
                expr.range(),
                format!("{lhs} if {lhs} is not None else {rhs}"),
            ));
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
}
