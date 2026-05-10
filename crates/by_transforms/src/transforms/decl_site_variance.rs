//! AST pass: strips def-site `out` / `in` / `in out` keyword from
//! `class C[out T]` type-parameter declarations. variance info preserved
//! on the AST node and consumed by ty's type checker directly — this
//! only deletes surface bytes so output is valid Python.
//!
//! use-site variance stripped upstream by [`use_site_variance::strip`]. when
//! [`generics`](super::generics) polyfills the type-params header into
//! `Generic[_T]`, its wider replacement wins via `ast_driver`'s first-wins
//! dedup and this pass's narrow deletion becomes a no-op for that stmt

use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{Stmt, TypeParam};
use ruff_text_size::{Ranged, TextRange};

use super::ast_driver::{PassContext, TypeAwarePass};
use crate::type_info::TypeInfo;

pub(crate) struct VarianceStripPass;

impl VarianceStripPass {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl TypeAwarePass for VarianceStripPass {
    fn run(&self, stmts: &[Stmt], _types: &dyn TypeInfo, ctx: &mut PassContext) {
        let mut state = State { edits: Vec::new() };
        for stmt in stmts {
            state.visit_stmt(stmt);
        }
        for (range, repl) in state.edits {
            ctx.text_edits.push((range, repl));
        }
    }
}

struct State {
    edits: Vec<(TextRange, String)>,
}

impl<'ast> Visitor<'ast> for State {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        let type_params = match stmt {
            Stmt::ClassDef(c) => c.type_params.as_deref(),
            Stmt::FunctionDef(f) => f.type_params.as_deref(),
            _ => None,
        };

        if let Some(tp) = type_params {
            for param in &tp.type_params {
                if let TypeParam::TypeVar(tv) = param
                    && tv.variance.is_some()
                {
                    let prefix = TextRange::new(tv.range().start(), tv.name.range().start());
                    self.edits.push((prefix, String::new()));
                }
            }
        }

        walk_stmt(self, stmt);
    }
}

#[cfg(test)]
mod tests {
    use crate::{Config, PythonVersion, transpile};
    use indoc::indoc;

    fn check_py312(input: &str, expected: &str) {
        let config = Config {
            min_version: PythonVersion::PY312,
            ..Config::test_default()
        };
        assert_eq!(transpile(input, &config).unwrap(), expected);
    }

    #[test]
    fn strips_out_keyword_on_class() {
        check_py312("class C[out T]: ...\n", "class C[T]: ...\n");
    }

    #[test]
    fn strips_in_keyword_on_class() {
        check_py312("class C[in T]: ...\n", "class C[T]: ...\n");
    }

    #[test]
    fn strips_in_out_keyword_on_class() {
        check_py312("class C[in out T]: ...\n", "class C[T]: ...\n");
    }

    #[test]
    fn strips_on_function() {
        check_py312(
            indoc! {"
                def f[out T](x: T) -> T:
                    return x
            "},
            indoc! {"
                def f[T](x: T) -> T:
                    return x
            "},
        );
    }

    #[test]
    fn invariant_typevar_untouched() {
        check_py312("class C[T]: ...\n", "class C[T]: ...\n");
    }
}
