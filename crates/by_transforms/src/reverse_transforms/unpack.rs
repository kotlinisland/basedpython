//! reverse of `crate::transforms::unpack`:
//!   `*args: Unpack[T]` → `*args: *T`
//!
//! only fires on vararg annotations when `Unpack` resolves to the typing import

use ruff_diagnostics::{Edit, Fix};
use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::{Ranged, TextRange};

use crate::type_info::TypeInfo;

pub(crate) struct UnpackReverse<'src> {
    source: &'src str,
    types: &'src dyn TypeInfo,
    pub(crate) edits: Vec<Fix>,
}

impl<'src> UnpackReverse<'src> {
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

    fn is_unpack_name(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Name(n) => n.id.as_str() == "Unpack" && self.types.subscript_is_type_context(n),
            Expr::Attribute(a) => {
                a.attr.id.as_str() == "Unpack"
                    && matches!(a.value.as_ref(), Expr::Name(n) if self.types.attr_base_is_type_context(n))
            }
            _ => false,
        }
    }

    fn process_vararg_annotation(&mut self, ann: &Expr) {
        let Expr::Subscript(s) = ann else {
            return;
        };
        if !self.is_unpack_name(&s.value) {
            return;
        }
        let inner = self.src(s.slice.range()).to_owned();
        self.edits.push(Fix::safe_edit(Edit::range_replacement(
            format!("*{inner}"),
            ann.range(),
        )));
    }
}

impl<'ast> Visitor<'ast> for UnpackReverse<'_> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        if let Stmt::FunctionDef(f) = stmt {
            if let Some(vararg) = &f.parameters.vararg {
                if let Some(ann) = &vararg.annotation {
                    self.process_vararg_annotation(ann);
                }
            }
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
    fn basic_unpack() {
        check(
            indoc! {"
                from typing import Unpack
                def f(*args: Unpack[tuple[int, ...]]): ...
            "},
            indoc! {"
                from typing import Unpack
                def f(*args: *tuple[int, ...])
            "},
        );
    }

    #[test]
    fn nested_function() {
        check(
            indoc! {"
                from typing import Unpack
                class A:
                    def method(self, *args: Unpack[tuple[str, ...]]): ...
            "},
            indoc! {"
                from typing import Unpack
                class A:
                    def method(self, *args: *tuple[str, ...])
            "},
        );
    }

    #[test]
    fn regular_arg_unchanged_by_unpack() {
        // unpack reverse leaves it alone; empty-declarations strips `: ...`
        check("def f(x: int): ...\n", "def f(x: int)\n");
    }

    #[test]
    fn shadowed_unchanged() {
        check(
            indoc! {"
                Unpack = object()
                def f(*args: Unpack[tuple[int, ...]]): ...
            "},
            indoc! {"
                Unpack = object()
                def f(*args: Unpack[tuple[int, ...]])
            "},
        );
    }
}
