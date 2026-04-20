//! Polyfills the PEP 646 starred-type syntax in variadic parameter annotations.
//!
//! `def f(*args: *tuple[int, ...])` → `def f(*args: Unpack[tuple[int, ...]])`
//!
//! The starred form (`*T`) is native in Python 3.11+; for earlier targets the
//! equivalent `Unpack[T]` form must be used instead.  The transform fires only
//! on `*args` parameter annotations and leaves every other starred expression
//! untouched.

use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::{Ranged, TextRange};

use crate::config::{Config, PythonVersion};

pub struct UnpackSyntax<'src> {
    source: &'src str,
    config: Config,
    pub edits: Vec<(TextRange, String)>,
    pub needs_import: bool,
}

impl<'src> UnpackSyntax<'src> {
    pub fn new(source: &'src str, config: Config) -> Self {
        Self {
            source,
            config,
            edits: Vec::new(),
            needs_import: false,
        }
    }

    fn src(&self, range: TextRange) -> &str {
        &self.source[usize::from(range.start())..usize::from(range.end())]
    }

    fn process_vararg_annotation(&mut self, ann: &Expr) {
        if self.config.min_version >= PythonVersion::V311 {
            return;
        }
        let Expr::Starred(starred) = ann else {
            return;
        };
        let inner = self.src(starred.value.range()).to_owned();
        self.needs_import = true;
        self.edits
            .push((ann.range(), format!("Unpack[{inner}]")));
    }
}

impl<'src, 'ast> Visitor<'ast> for UnpackSyntax<'src> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        if let Stmt::FunctionDef(f) = stmt {
            // Only the annotation of the variadic positional parameter
            // (`*args`) is subject to this transform.
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
    use crate::{transpile, Config};
    use crate::config::PythonVersion;
    use indoc::indoc;

    fn check(input: &str, expected: &str) {
        assert_eq!(transpile(input, &Config::default()).unwrap(), expected);
    }

    #[test]
    fn rewrites_starred_vararg_annotation() {
        check(
            "def f(*args: *tuple[int, ...]): ...\n",
            indoc! {"
                from typing import Unpack
                def f(*args: Unpack[tuple[int, ...]]): ...
            "},
        );
    }

    #[test]
    fn no_rewrite_on_311() {
        let config = Config { min_version: PythonVersion::V311 };
        assert_eq!(
            transpile("def f(*args: *tuple[int, ...]): ...\n", &config).unwrap(),
            "def f(*args: *tuple[int, ...]): ...\n",
        );
    }

    #[test]
    fn nested_function() {
        check(
            indoc! {"
                class A:
                    def method(self, *args: *tuple[str, ...]): ...
            "},
            indoc! {"
                from typing import Unpack
                class A:
                    def method(self, *args: Unpack[tuple[str, ...]]): ...
            "},
        );
    }

    #[test]
    fn regular_arg_annotation_unchanged() {
        check(
            "def f(x: int): ...\n",
            "def f(x: int): ...\n",
        );
    }
}
