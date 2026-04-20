//! Rewrites intersection types in annotation positions.
//!
//! `a: A & B`            → `a: Intersection[A, B]`
//! `a: A & B & C`        → `a: Intersection[A, B, C]`
//! `a: (A & B) | C`      → `a: Intersection[A, B] | C`
//! `a: list[A & B]`      → `a: list[Intersection[A, B]]`
//!
//! Uses `Intersection` from `ty_extensions`, the basedpython type-extensions
//! package.  Fires only in syntactic annotation positions, so bitwise-AND in
//! value expressions is never affected.

use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{Expr, Operator, Parameters, Stmt};
use ruff_text_size::{Ranged, TextRange};

pub struct IntersectionType<'src> {
    source: &'src str,
    pub edits: Vec<(TextRange, String)>,
    pub needs_import: bool,
}

impl<'src> IntersectionType<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            edits: Vec::new(),
            needs_import: false,
        }
    }

    fn src(&self, range: TextRange) -> &str {
        &self.source[usize::from(range.start())..usize::from(range.end())]
    }

    /// Recursively rewrite a type expression, returning `Some(new_text)` if
    /// any intersection was found (and thus the whole expr needs replacing),
    /// or `None` if the source text can be used unchanged.
    fn rewrite(&mut self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::BinOp(b) if matches!(b.op, Operator::BitAnd) => {
                // Flatten the left-associative `&` chain into [A, B, C, ...].
                let mut operands: Vec<&Expr> = Vec::new();
                collect_bitand(expr, &mut operands);
                let parts: Vec<String> = operands
                    .iter()
                    .map(|e| self.rewrite(e).unwrap_or_else(|| self.src(e.range()).to_owned()))
                    .collect();
                self.needs_import = true;
                Some(format!("Intersection[{}]", parts.join(", ")))
            }

            Expr::BinOp(b) if matches!(b.op, Operator::BitOr) => {
                let l = self.rewrite(&b.left);
                let r = self.rewrite(&b.right);
                if l.is_some() || r.is_some() {
                    let ls = l.unwrap_or_else(|| self.src(b.left.range()).to_owned());
                    let rs = r.unwrap_or_else(|| self.src(b.right.range()).to_owned());
                    Some(format!("{ls} | {rs}"))
                } else {
                    None
                }
            }

            Expr::Subscript(s) => {
                let slice_rewrite = match s.slice.as_ref() {
                    Expr::Tuple(t) if !t.parenthesized => {
                        let rewrites: Vec<Option<String>> =
                            t.elts.iter().map(|e| self.rewrite(e)).collect();
                        if rewrites.iter().any(|r| r.is_some()) {
                            let parts: Vec<String> = rewrites
                                .into_iter()
                                .zip(t.elts.iter())
                                .map(|(r, e)| {
                                    r.unwrap_or_else(|| self.src(e.range()).to_owned())
                                })
                                .collect();
                            Some(parts.join(", "))
                        } else {
                            None
                        }
                    }
                    slice => self.rewrite(slice),
                };
                slice_rewrite.map(|s_text| {
                    format!("{}[{s_text}]", self.src(s.value.range()))
                })
            }

            _ => None,
        }
    }

    fn visit_annotation(&mut self, ann: &Expr) {
        if let Some(rewrite) = self.rewrite(ann) {
            self.edits.push((ann.range(), rewrite));
        }
    }

    fn visit_parameters(&mut self, params: &Parameters) {
        for p in params.iter_non_variadic_params() {
            if let Some(ann) = &p.parameter.annotation {
                self.visit_annotation(ann);
            }
        }
        if let Some(v) = &params.vararg {
            if let Some(ann) = &v.annotation {
                self.visit_annotation(ann);
            }
        }
        if let Some(k) = &params.kwarg {
            if let Some(ann) = &k.annotation {
                self.visit_annotation(ann);
            }
        }
    }
}

impl<'src, 'ast> Visitor<'ast> for IntersectionType<'src> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        match stmt {
            Stmt::AnnAssign(a) => {
                self.visit_annotation(&a.annotation);
            }
            Stmt::TypeAlias(a) => {
                self.visit_annotation(&a.value);
                return; // don't recurse into the alias value again
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
}

fn collect_bitand<'a>(expr: &'a Expr, out: &mut Vec<&'a Expr>) {
    if let Expr::BinOp(b) = expr
        && matches!(b.op, Operator::BitAnd)
    {
        collect_bitand(&b.left, out);
        collect_bitand(&b.right, out);
    } else {
        out.push(expr);
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
    fn simple_two_type() {
        check(
            "a: A & B\n",
            indoc! {"
                from ty_extensions import Intersection
                a: Intersection[A, B]
            "},
        );
    }

    #[test]
    fn three_types() {
        check(
            "a: A & B & C\n",
            indoc! {"
                from ty_extensions import Intersection
                a: Intersection[A, B, C]
            "},
        );
    }

    #[test]
    fn intersection_with_union() {
        check(
            "a: (A & B) | C\n",
            indoc! {"
                from ty_extensions import Intersection
                a: Intersection[A, B] | C
            "},
        );
    }

    #[test]
    fn nested_inside_list() {
        check(
            "a: list[A & B]\n",
            indoc! {"
                from ty_extensions import Intersection
                a: list[Intersection[A, B]]
            "},
        );
    }

    #[test]
    fn function_parameter() {
        check(
            indoc! {"
                def f(x: A & B) -> A & C:
                    pass
            "},
            indoc! {"
                from ty_extensions import Intersection
                def f(x: Intersection[A, B]) -> Intersection[A, C]:
                    pass
            "},
        );
    }

    #[test]
    fn value_context_unchanged() {
        check("x = A & B\n", "x = A & B\n");
    }

    #[test]
    fn augmented_assign_unchanged() {
        check("x &= B\n", "x &= B\n");
    }
}
