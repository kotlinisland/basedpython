use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{Expr, Parameters, Stmt};
use ruff_text_size::Ranged;

/// Rewrites tuple literal types in annotation positions.
///
/// `a: (int, str)` → `a: tuple[int, str]`
///
/// Only fires in syntactic annotation positions (AnnAssign.annotation,
/// parameter annotations, FunctionDef.returns). Value positions are left alone.
pub struct TupleLiteralType<'src> {
    source: &'src str,
    pub edits: Vec<(ruff_text_size::TextRange, String)>,
}

impl<'src> TupleLiteralType<'src> {
    pub fn new(source: &'src str) -> Self {
        Self { source, edits: Vec::new() }
    }

    fn src(&self, range: ruff_text_size::TextRange) -> &str {
        &self.source[usize::from(range.start())..usize::from(range.end())]
    }

    /// Returns a rewritten annotation string if any transformation is needed,
    /// or `None` if the expression requires no change.
    fn transform_annotation(&self, expr: &Expr) -> Option<String> {
        match expr {
            // Parenthesized tuple: `(int, str)` → `tuple[int, str]`
            Expr::Tuple(t) if t.parenthesized => {
                let inner = t.elts.iter()
                    .map(|e| self.transform_annotation(e)
                        .unwrap_or_else(|| self.src(e.range()).to_owned()))
                    .collect::<Vec<_>>()
                    .join(", ");
                Some(format!("tuple[{inner}]"))
            }

            // `A | B` — propagate into both arms
            Expr::BinOp(b) => {
                let left = self.transform_annotation(&b.left);
                let right = self.transform_annotation(&b.right);
                if left.is_some() || right.is_some() {
                    let l = left.unwrap_or_else(|| self.src(b.left.range()).to_owned());
                    let r = right.unwrap_or_else(|| self.src(b.right.range()).to_owned());
                    Some(format!("{l} | {r}"))
                } else {
                    None
                }
            }

            // `X[...]` — propagate into slice only
            Expr::Subscript(s) => {
                // The slice of a subscript may be a non-parenthesized tuple
                // (e.g. `dict[str, int]` parses the slice as an unparenthesized
                // Tuple). Propagate into each element individually rather than
                // wrapping the whole slice in `tuple[...]`.
                let slice_rewrite = match s.slice.as_ref() {
                    Expr::Tuple(t) if !t.parenthesized => {
                        let rewrites: Vec<Option<String>> = t.elts.iter()
                            .map(|e| self.transform_annotation(e))
                            .collect();
                        if rewrites.iter().any(|r| r.is_some()) {
                            let parts = rewrites.into_iter().zip(t.elts.iter())
                                .map(|(r, e)| r.unwrap_or_else(|| self.src(e.range()).to_owned()))
                                .collect::<Vec<_>>()
                                .join(", ");
                            Some(parts)
                        } else {
                            None
                        }
                    }
                    slice => self.transform_annotation(slice),
                };
                if let Some(slice_str) = slice_rewrite {
                    let value_str = self.src(s.value.range());
                    Some(format!("{value_str}[{slice_str}]"))
                } else {
                    None
                }
            }

            _ => None,
        }
    }

    fn visit_annotation(&mut self, expr: &Expr) {
        if let Some(rewritten) = self.transform_annotation(expr) {
            self.edits.push((expr.range(), rewritten));
        }
    }

    fn visit_parameters(&mut self, params: &Parameters) {
        for param in params.iter_non_variadic_params() {
            if let Some(ann) = &param.parameter.annotation {
                self.visit_annotation(ann);
            }
        }
        if let Some(var) = &params.vararg {
            if let Some(ann) = &var.annotation {
                self.visit_annotation(ann);
            }
        }
        if let Some(kwarg) = &params.kwarg {
            if let Some(ann) = &kwarg.annotation {
                self.visit_annotation(ann);
            }
        }
    }
}

impl<'src, 'ast> Visitor<'ast> for TupleLiteralType<'src> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        match stmt {
            Stmt::AnnAssign(a) => {
                self.visit_annotation(&a.annotation);
                // Don't walk into a.value — that's a value context.
                // Do walk nested statements (none in AnnAssign, but be safe).
            }
            Stmt::FunctionDef(f) => {
                self.visit_parameters(&f.parameters);
                if let Some(ret) = &f.returns {
                    self.visit_annotation(ret);
                }
                for s in &f.body {
                    self.visit_stmt(s);
                }
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, _expr: &'ast Expr) {
        // Intentionally empty: we only process annotations via visit_annotation,
        // never arbitrary expressions.
    }
}

#[cfg(test)]
mod tests {
    use crate::{transpile, Config};
    use indoc::indoc;

    fn check(input: &str, expected: &str) {
        assert_eq!(transpile(input, &Config::default()).unwrap(), expected);
    }

    #[test]
    fn simple_tuple_annotation() {
        check("a: (int, str)\n", "a: tuple[int, str]\n");
    }

    #[test]
    fn single_element_tuple() {
        check("a: (int,)\n", "a: tuple[int]\n");
    }

    #[test]
    fn nested_tuple() {
        check("a: (int, (str, float))\n", "a: tuple[int, tuple[str, float]]\n");
    }

    #[test]
    fn tuple_in_union() {
        check("a: (int, str) | None\n", "a: tuple[int, str] | None\n");
    }

    #[test]
    fn tuple_in_subscript_slice() {
        check("a: list[(int, str)]\n", "a: list[tuple[int, str]]\n");
    }

    #[test]
    fn subscript_non_parenthesized_tuple_propagated() {
        // dict[str, (int, float)] — the `str, (int, float)` is an unparenthesized
        // tuple in the slice; only the parenthesized inner tuple should be rewritten
        check("a: dict[str, (int, float)]\n", "a: dict[str, tuple[int, float]]\n");
    }

    #[test]
    fn function_parameter_annotation() {
        check(
            indoc! {"
                def f(x: (int, str)) -> (bool, float):
                    pass
            "},
            indoc! {"
                def f(x: tuple[int, str]) -> tuple[bool, float]:
                    pass
            "},
        );
    }

    #[test]
    fn value_context_not_rewritten() {
        // Assignment value should NOT be rewritten
        check("a: int = (int, str)\n", "a: int = (int, str)\n");
    }

    #[test]
    fn plain_subscript_unchanged() {
        check("a: list[int]\n", "a: list[int]\n");
    }

    #[test]
    fn non_annotation_tuple_unchanged() {
        check("x = (1, 2)\n", "x = (1, 2)\n");
    }
}
