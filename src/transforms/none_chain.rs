use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::{Ranged, TextRange};

/// rewrites `a?.b` to `None if a is None else a.b`
/// and `a?.b()` to `None if a is None else a.b()`
pub struct NoneChain<'src> {
    source: &'src str,
    pub edits: Vec<(TextRange, String)>,
}

impl<'src> NoneChain<'src> {
    pub fn new(source: &'src str) -> Self {
        Self { source, edits: Vec::new() }
    }

    fn src(&self, range: TextRange) -> &str {
        &self.source[usize::from(range.start())..usize::from(range.end())]
    }
}

impl<'src, 'ast> Visitor<'ast> for NoneChain<'src> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let Expr::Attribute(attr) = expr
            && attr.optional
        {
            let obj = self.src(attr.value.range());
            let field = attr.attr.as_str();
            let full = format!("{obj}.{field}");
            self.edits.push((
                expr.range(),
                format!("None if {obj} is None else {full}"),
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
    fn basic_chain() {
        check(
            "x = a?.b\n",
            "x = None if a is None else a.b\n",
        );
    }
}
