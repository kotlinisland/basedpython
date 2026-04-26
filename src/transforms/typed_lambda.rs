use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_python_ast::{Expr, Parameters, Stmt};
use ruff_text_size::{Ranged, TextRange};

/// strips basedpython typed lambda syntax down to standard python:
///   `lambda (a: int, b: str) -> int: a + b`  →  `lambda a, b: a + b`
pub struct TypedLambda {
    pub edits: Vec<(TextRange, String)>,
}

impl TypedLambda {
    pub fn new(_source: &str) -> Self {
        Self {
            edits: Vec::new(),
        }
    }

    fn is_typed(params: &Parameters, returns: &Option<Box<Expr>>) -> bool {
        returns.is_some()
            || params
                .iter_non_variadic_params()
                .any(|p| p.parameter.annotation.is_some())
            || params
                .vararg
                .as_ref()
                .is_some_and(|p| p.annotation.is_some())
            || params
                .kwarg
                .as_ref()
                .is_some_and(|p| p.annotation.is_some())
    }

    fn params_to_names(params: &Parameters) -> String {
        let mut parts: Vec<String> = Vec::new();
        for p in &params.posonlyargs {
            parts.push(p.parameter.name.as_str().to_owned());
        }
        if !params.posonlyargs.is_empty() {
            parts.push("/".to_owned());
        }
        for p in &params.args {
            parts.push(p.parameter.name.as_str().to_owned());
        }
        if let Some(vararg) = &params.vararg {
            parts.push(format!("*{}", vararg.name.as_str()));
        } else if !params.kwonlyargs.is_empty() {
            parts.push("*".to_owned());
        }
        for p in &params.kwonlyargs {
            parts.push(p.parameter.name.as_str().to_owned());
        }
        if let Some(kwarg) = &params.kwarg {
            parts.push(format!("**{}", kwarg.name.as_str()));
        }
        parts.join(", ")
    }

}

impl<'ast> Visitor<'ast> for TypedLambda {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let Expr::Lambda(lambda) = expr {
            if let Some(params) = &lambda.parameters {
                if Self::is_typed(params, &lambda.returns) {
                    let names = Self::params_to_names(params);
                    // parse_parameters(FunctionDef) includes `(` and `)` in range
                    let open_paren = params.range().start();
                    let end = if let Some(ret) = &lambda.returns {
                        ret.range().end()
                    } else {
                        params.range().end()
                    };
                    let edit_range = TextRange::new(open_paren, end);
                    self.edits.push((edit_range, names));
                }
            }
            // still visit body (may contain nested lambdas)
            walk_expr(self, expr);
            return;
        }
        walk_expr(self, expr);
    }
}

#[cfg(test)]
mod tests {
    use crate::transpile;

    fn check(input: &str, expected: &str) {
        assert_eq!(transpile(input, &crate::Config::default()).unwrap(), expected);
    }

    #[test]
    fn typed_lambda_basic() {
        check(
            "a = lambda (a: int, b: str) -> int: a\n",
            "a = lambda a, b: a\n",
        );
    }

    #[test]
    fn typed_lambda_no_return() {
        check(
            "a = lambda (x: int): x\n",
            "a = lambda x: x\n",
        );
    }

    #[test]
    fn typed_lambda_only_return() {
        check(
            "a = lambda () -> int: 42\n",
            "a = lambda : 42\n",
        );
    }

    #[test]
    fn untyped_lambda_unchanged() {
        check("a = lambda x, y: x + y\n", "a = lambda x, y: x + y\n");
    }

    #[test]
    fn typed_lambda_with_star_args() {
        check(
            "a = lambda (*args, **kwargs) -> int: 0\n",
            "a = lambda *args, **kwargs: 0\n",
        );
    }
}
