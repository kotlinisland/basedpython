//! AST pass that rewrites expressions with direct 3.10-compatible equivalents.
//!
//! All rewrites are gated on `config.min_version` — if the runtime already
//! supports the feature natively the expression is left alone.
//!
//! - `datetime.UTC`   → `datetime.timezone.utc`          (added 3.11)
//! - `sys.exception()`→ `sys.exc_info()[1]`              (added 3.11)
//! - `math.exp2(x)`   → `2 ** (x)`                       (added 3.11)

use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_python_ast::{Expr, ModModule, PythonVersion, Stmt};
use ruff_text_size::{Ranged, TextRange};

use crate::Config;

use super::ast_driver::{AstPass, PassContext};

pub(crate) struct CompatRewrite<'src> {
    source: &'src str,
    config: Config,
}

impl<'src> CompatRewrite<'src> {
    pub(crate) fn new(source: &'src str, config: Config) -> Self {
        Self { source, config }
    }
}

impl AstPass for CompatRewrite<'_> {
    fn run(&self, module: &mut ModModule, ctx: &mut PassContext) {
        if self.config.min_version >= PythonVersion::PY311 {
            return;
        }
        let mut state = State {
            source: self.source,
            edits: Vec::new(),
        };
        for stmt in &module.body {
            state.visit_stmt(stmt);
        }
        ctx.text_edits.extend(state.edits);
    }
}

struct State<'src> {
    source: &'src str,
    edits: Vec<(TextRange, String)>,
}

impl State<'_> {
    fn src(&self, range: TextRange) -> &str {
        &self.source[usize::from(range.start())..usize::from(range.end())]
    }

    fn check_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Attribute(attr)
                if attr.attr.id.as_str() == "UTC"
                    && matches!(attr.value.as_ref(), Expr::Name(n) if n.id.as_str() == "datetime") =>
            {
                self.edits
                    .push((expr.range(), "datetime.timezone.utc".to_owned()));
            }
            Expr::Call(call)
                if call.arguments.args.is_empty()
                    && call.arguments.keywords.is_empty()
                    && matches!(
                        call.func.as_ref(),
                        Expr::Attribute(a)
                            if a.attr.id.as_str() == "exception"
                                && matches!(a.value.as_ref(), Expr::Name(n) if n.id.as_str() == "sys")
                    ) =>
            {
                self.edits
                    .push((expr.range(), "sys.exc_info()[1]".to_owned()));
            }
            Expr::Call(call)
                if call.arguments.args.len() == 1
                    && call.arguments.keywords.is_empty()
                    && matches!(
                        call.func.as_ref(),
                        Expr::Attribute(a)
                            if a.attr.id.as_str() == "exp2"
                                && matches!(a.value.as_ref(), Expr::Name(n) if n.id.as_str() == "math")
                    ) =>
            {
                let arg_src = self.src(call.arguments.args[0].range()).to_owned();
                self.edits.push((expr.range(), format!("2 ** ({arg_src})")));
            }
            _ => {}
        }
    }
}

impl<'ast> Visitor<'ast> for State<'_> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        self.check_expr(expr);
        walk_expr(self, expr);
    }
}

#[cfg(test)]
mod tests {
    use crate::{Config, transpile};
    use indoc::indoc;
    use ruff_python_ast::PythonVersion;

    fn check(input: &str, expected: &str) {
        assert_eq!(
            transpile(input, &Config::test_default()).unwrap(),
            crate::python_passthrough::lazify_expected(expected)
        );
    }

    fn check_version(input: &str, expected: &str, version: PythonVersion) {
        let config = Config {
            min_version: version,
            ..Config::test_default()
        };
        assert_eq!(
            transpile(input, &config).unwrap(),
            crate::python_passthrough::lazify_expected(expected)
        );
    }

    #[test]
    fn datetime_utc_rewrite() {
        check(
            "import datetime\ntz = datetime.UTC\n",
            "import datetime\ntz = datetime.timezone.utc\n",
        );
    }

    #[test]
    fn datetime_utc_no_rewrite_on_311() {
        check_version(
            "import datetime\ntz = datetime.UTC\n",
            "import datetime\ntz = datetime.UTC\n",
            PythonVersion::PY311,
        );
    }

    #[test]
    fn sys_exception_rewrite() {
        check(
            indoc! {"
                import sys
                err = sys.exception()
            "},
            indoc! {"
                import sys
                err = sys.exc_info()[1]
            "},
        );
    }

    #[test]
    fn math_exp2_rewrite() {
        check(
            indoc! {"
                import math
                y = math.exp2(x)
            "},
            indoc! {"
                import math
                y = 2 ** (x)
            "},
        );
    }

    #[test]
    fn math_exp2_compound_arg() {
        check("y = math.exp2(a + b)\n", "y = 2 ** (a + b)\n");
    }
}
