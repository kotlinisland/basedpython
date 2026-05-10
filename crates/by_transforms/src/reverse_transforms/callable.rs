//! reverse of `crate::transforms::callable`:
//!   `Callable[[int], int]`       → `(int) -> int`
//!   `Callable[[int, str], bool]` → `(int, str) -> bool`
//!   `Callable[[], None]`         → `() -> None`
//!
//! only fires in annotation positions when `Callable` resolves to the typing import

use ruff_diagnostics::{Edit, Fix};
use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::{Ranged, TextRange};

use crate::type_info::TypeInfo;

pub(crate) struct CallableReverse<'src> {
    source: &'src str,
    types: &'src dyn TypeInfo,
    pub(crate) edits: Vec<Fix>,
}

impl<'src> CallableReverse<'src> {
    pub(crate) fn new(source: &'src str, types: &'src dyn TypeInfo) -> Self {
        Self {
            source,
            types,
            edits: Vec::new(),
        }
    }

    fn src(&self, range: TextRange) -> &str {
        &self.source[usize::from(range.start())..usize::from(range.end())]
    }

    fn is_callable_name(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Name(n) => n.id.as_str() == "Callable" && self.types.subscript_is_type_context(n),
            Expr::Attribute(a) => {
                a.attr.id.as_str() == "Callable"
                    && matches!(a.value.as_ref(), Expr::Name(n) if self.types.attr_base_is_type_context(n))
            }
            _ => false,
        }
    }

    fn rewrite(&mut self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Subscript(s) if self.is_callable_name(&s.value) => {
                let Expr::Tuple(t) = s.slice.as_ref() else {
                    return None;
                };
                if t.parenthesized || t.elts.len() != 2 {
                    return None;
                }
                let Expr::List(args_list) = &t.elts[0] else {
                    return None;
                };
                let args_str = args_list
                    .elts
                    .iter()
                    .map(|a| {
                        self.rewrite(a)
                            .unwrap_or_else(|| self.src(a.range()).to_owned())
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let ret = &t.elts[1];
                let ret_str = self
                    .rewrite(ret)
                    .unwrap_or_else(|| self.src(ret.range()).to_owned());
                Some(format!("({args_str}) -> {ret_str}"))
            }

            Expr::BinOp(b) => {
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
                        if rewrites.iter().any(Option::is_some) {
                            let parts: Vec<String> = rewrites
                                .into_iter()
                                .zip(t.elts.iter())
                                .map(|(r, e)| r.unwrap_or_else(|| self.src(e.range()).to_owned()))
                                .collect();
                            Some(parts.join(", "))
                        } else {
                            None
                        }
                    }
                    slice => self.rewrite(slice),
                };
                slice_rewrite.map(|s_text| format!("{}[{s_text}]", self.src(s.value.range())))
            }

            _ => None,
        }
    }

    fn visit_annotation(&mut self, ann: &Expr) {
        if let Some(rewrite) = self.rewrite(ann) {
            self.edits.push(Fix::safe_edit(Edit::range_replacement(
                rewrite,
                ann.range(),
            )));
        }
    }
}

impl<'ast> Visitor<'ast> for CallableReverse<'_> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        crate::transforms::source_util::for_each_annotation_in_stmt(stmt, |ann| {
            self.visit_annotation(ann);
        });
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
    fn simple_callable() {
        check(
            "from typing import Callable\na: Callable[[int], int]\n",
            "from typing import Callable\na: (int) -> int\n",
        );
    }

    #[test]
    fn no_args() {
        check(
            "from typing import Callable\na: Callable[[], None]\n",
            "from typing import Callable\na: () -> None\n",
        );
    }

    #[test]
    fn multi_args() {
        check(
            "from typing import Callable\na: Callable[[int, str], bool]\n",
            "from typing import Callable\na: (int, str) -> bool\n",
        );
    }

    #[test]
    fn callable_in_union() {
        check(
            "from typing import Callable\na: Callable[[int], int] | None\n",
            "from typing import Callable\na: (int) -> int | None\n",
        );
    }

    #[test]
    fn nested_callable() {
        check(
            "from typing import Callable\na: Callable[[int], Callable[[str], bool]]\n",
            "from typing import Callable\na: (int) -> (str) -> bool\n",
        );
    }

    #[test]
    fn callable_in_function_signature() {
        check(
            indoc! {"
                from typing import Callable
                def f(x: Callable[[int], bool]) -> Callable[[str], None]:
                    pass
            "},
            indoc! {"
                from typing import Callable
                def f(x: (int) -> bool) -> (str) -> None:
                    pass
            "},
        );
    }

    #[test]
    fn callable_inside_list_subscript() {
        check(
            "from typing import Callable\na: list[Callable[[int], int]]\n",
            "from typing import Callable\na: list[(int) -> int]\n",
        );
    }

    #[test]
    fn shadowed_callable_unchanged() {
        check(
            indoc! {"
                Callable = object()
                a: Callable[[int], int]
            "},
            indoc! {"
                Callable = object()
                a: Callable[[int], int]
            "},
        );
    }
}
