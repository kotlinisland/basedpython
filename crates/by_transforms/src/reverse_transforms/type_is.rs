//! reverse of `crate::transforms::type_is`:
//!   `def f(a) -> TypeIs[T]:` → `def f(a) -> a is T:`
//!
//! the rewrite needs the first parameter name to reconstruct the basedpython
//! `name is T` form. `TypeIs` from `typing` or `typing_extensions` is
//! recognized

use ruff_diagnostics::{Edit, Fix};
use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::{Ranged, TextRange};

use crate::type_info::TypeInfo;

pub(crate) struct TypeIsReverse<'src> {
    source: &'src str,
    types: &'src dyn TypeInfo,
    pub(crate) edits: Vec<Fix>,
}

impl<'src> TypeIsReverse<'src> {
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

    fn is_type_is(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Name(n) => n.id.as_str() == "TypeIs" && self.types.subscript_is_type_context(n),
            Expr::Attribute(a) => {
                a.attr.id.as_str() == "TypeIs"
                    && matches!(a.value.as_ref(), Expr::Name(n) if self.types.attr_base_is_type_context(n))
            }
            _ => false,
        }
    }
}

impl<'ast> Visitor<'ast> for TypeIsReverse<'_> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        if let Stmt::FunctionDef(f) = stmt {
            if let Some(ret) = &f.returns
                && let Expr::Subscript(s) = ret.as_ref()
                && self.is_type_is(&s.value)
            {
                let first_name = f
                    .parameters
                    .posonlyargs
                    .first()
                    .map(|p| p.parameter.name.id.as_str())
                    .or_else(|| {
                        f.parameters
                            .args
                            .first()
                            .map(|p| p.parameter.name.id.as_str())
                    })
                    .unwrap_or("a");
                let inner = self.src(s.slice.range());
                // a multi-line union relied on the `TypeIs[...]` brackets for line
                // continuation; once the brackets are gone the bare `name is ...`
                // form must parenthesize it to stay a single valid expression
                // (e.g. `inspect.isroutine`). single-line types need no parens.
                let replacement = if inner.contains('\n') {
                    format!("{first_name} is ({inner})")
                } else {
                    format!("{first_name} is {inner}")
                };
                self.edits.push(Fix::safe_edit(Edit::range_replacement(
                    replacement,
                    ret.range(),
                )));
            }
            for s in &f.body {
                self.visit_stmt(s);
            }
            return;
        }
        walk_stmt(self, stmt);
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
    fn typeis_param_annotation() {
        check(
            indoc! {"
                from typing import TypeIs
                def f(x) -> TypeIs[int]: ...
            "},
            indoc! {"
                from typing import TypeIs
                def f(x) -> x is int
            "},
        );
    }

    #[test]
    fn typeis_return_annotation_module_scope() {
        check(
            indoc! {"
                from typing import TypeIs
                def is_str(x: object) -> TypeIs[str]:
                    return isinstance(x, str)
            "},
            indoc! {"
                from typing import TypeIs
                def is_str(x: object) -> x is str:
                    return x is str
            "},
        );
    }

    #[test]
    fn unrelated_subscript_left_alone() {
        check("x: list[int]\n", "x: list[int]\n");
    }

    #[test]
    fn typeis_multiline_union_parenthesized() {
        // a multi-line union inside `TypeIs[...]` relied on the brackets for line
        // continuation; the bare `name is ...` form must parenthesize it (e.g.
        // `inspect.isroutine`), otherwise the continuation lines don't parse
        check(
            indoc! {"
                from typing import TypeIs
                def f(
                    x: object,
                ) -> TypeIs[
                    int
                    | str
                    | bytes
                ]: ...
            "},
            indoc! {"
                from typing import TypeIs
                def f(
                    x: object,
                ) -> x is (int
                    | str
                    | bytes)
            "},
        );
    }
}
