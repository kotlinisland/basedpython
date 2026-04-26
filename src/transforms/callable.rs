//! rewrites callable type syntax in annotation positions
//!
//! `(int) -> int`             → `Callable[[int], int]`
//! `(int, str) -> bool`       → `Callable[[int, str], bool]`
//! `() -> None`               → `Callable[[], None]`
//! `(int) -> (str) -> bool`   → `Callable[[int], Callable[[str], bool]]`

use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{Expr, ExprCallableType, Parameters, Stmt};
use ruff_text_size::{Ranged, TextRange};

pub struct CallableSyntax<'src> {
    source: &'src str,
    pub edits: Vec<(TextRange, String)>,
    pub needs_import: bool,
}

impl<'src> CallableSyntax<'src> {
    pub fn new(source: &'src str) -> Self {
        Self { source, edits: Vec::new(), needs_import: false }
    }

    fn src(&self, range: TextRange) -> &str {
        &self.source[usize::from(range.start())..usize::from(range.end())]
    }

    pub fn rewrite(&mut self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::CallableType(ExprCallableType { args, returns, .. }) => {
                self.needs_import = true;
                let args_str = args
                    .iter()
                    .map(|a| self.rewrite(a).unwrap_or_else(|| self.src(a.range()).to_owned()))
                    .collect::<Vec<_>>()
                    .join(", ");
                let ret_str = self
                    .rewrite(returns)
                    .unwrap_or_else(|| self.src(returns.range()).to_owned());
                Some(format!("Callable[[{args_str}], {ret_str}]"))
            }

            Expr::BinOp(b) => {
                let l = self.rewrite(&b.left);
                let r = self.rewrite(&b.right);
                if l.is_some() || r.is_some() {
                    let op = b.op.as_str();
                    let ls = l.unwrap_or_else(|| self.src(b.left.range()).to_owned());
                    let rs = r.unwrap_or_else(|| self.src(b.right.range()).to_owned());
                    Some(format!("{ls} {op} {rs}"))
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

/// if `expr` is `Subscript(Name("__let__"|"__classvar__"), slice)`, returns the slice
pub fn synthetic_let_slice<'a>(expr: &'a Expr) -> Option<&'a Expr> {
    if let Expr::Subscript(s) = expr {
        if let Expr::Name(n) = s.value.as_ref() {
            if matches!(n.id.as_str(), "__let__" | "__classvar__") {
                return Some(s.slice.as_ref());
            }
        }
    }
    None
}

impl<'src, 'ast> Visitor<'ast> for CallableSyntax<'src> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        match stmt {
            Stmt::AnnAssign(a) => {
                // for __let__[T] / __classvar__[T] subscripts, only rewrite the
                // slice — modifiers handles the full prefix including Final[...] wrapping
                let effective_ann = synthetic_let_slice(a.annotation.as_ref())
                    .unwrap_or_else(|| a.annotation.as_ref());
                self.visit_annotation(effective_ann);
            }
            Stmt::TypeAlias(a) => {
                self.visit_annotation(&a.value);
                return;
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

#[cfg(test)]
mod tests {
    use crate::{transpile, Config};
    use indoc::indoc;

    fn check(input: &str, expected: &str) {
        assert_eq!(transpile(input, &Config::default()).unwrap(), expected);
    }

    #[test]
    fn simple_callable() {
        check(
            "a: (int) -> int\n",
            indoc! {"
                from typing import Callable
                a: Callable[[int], int]
            "},
        );
    }

    #[test]
    fn no_args() {
        check(
            "a: () -> None\n",
            indoc! {"
                from typing import Callable
                a: Callable[[], None]
            "},
        );
    }

    #[test]
    fn multi_args() {
        check(
            "a: (int, str) -> bool\n",
            indoc! {"
                from typing import Callable
                a: Callable[[int, str], bool]
            "},
        );
    }

    #[test]
    fn callable_in_union() {
        check(
            "a: (int) -> int | None\n",
            indoc! {"
                from typing import Callable
                a: Callable[[int], int] | None
            "},
        );
    }

    #[test]
    fn callable_as_return_type() {
        check(
            indoc! {"
                def f(x: (int) -> bool) -> (str) -> None:
                    pass
            "},
            indoc! {"
                from typing import Callable
                def f(x: Callable[[int], bool]) -> Callable[[str], None]:
                    pass
            "},
        );
    }

    #[test]
    fn nested_callable() {
        check(
            "a: (int) -> (str) -> bool\n",
            indoc! {"
                from typing import Callable
                a: Callable[[int], Callable[[str], bool]]
            "},
        );
    }

    #[test]
    fn callable_inside_subscript() {
        check(
            "a: list[(int) -> int]\n",
            indoc! {"
                from typing import Callable
                a: list[Callable[[int], int]]
            "},
        );
    }

    #[test]
    fn value_context_not_rewritten() {
        check("x = (int)\n", "x = (int)\n");
    }
}
