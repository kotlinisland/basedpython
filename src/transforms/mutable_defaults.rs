use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{Expr, Stmt, StmtFunctionDef};
use ruff_text_size::{Ranged, TextRange, TextSize};

/// Replaces mutable default arguments (`[]`, `{}`, `set()` literals) with a
/// `_MISSING` sentinel, and injects a guard at the top of each function body:
///
///   def f(x=[]):        →   def f(x=_MISSING):
///       ...                     if x is _MISSING:
///                                   x = []
///                               ...
pub struct MutableDefaultFixer<'src> {
    source: &'src str,
    pub edits: Vec<(TextRange, String)>,
    pub needs_sentinel: bool,
}

impl<'src> MutableDefaultFixer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            edits: Vec::new(),
            needs_sentinel: false,
        }
    }

    fn is_mutable_literal(expr: &Expr) -> bool {
        matches!(expr, Expr::List(_) | Expr::Dict(_) | Expr::Set(_))
    }

    // Returns (line_start_offset, indent_str) for the line containing `pos`.
    fn line_info(&self, pos: TextSize) -> (TextSize, &str) {
        let offset = usize::from(pos);
        let line_start = self.source[..offset]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let rest = &self.source[line_start..offset];
        let ws_len = rest.len() - rest.trim_start().len();
        (
            TextSize::from(line_start as u32),
            &self.source[line_start..line_start + ws_len],
        )
    }

    fn process_function(&mut self, func: &StmtFunctionDef) {
        let params = func.parameters.as_ref();
        let all_params = params
            .posonlyargs
            .iter()
            .chain(params.args.iter())
            .chain(params.kwonlyargs.iter());

        // (param name, default source range, default source text)
        let mut fixups: Vec<(String, TextRange, String)> = Vec::new();

        for param in all_params {
            if let Some(default) = &param.default
                && Self::is_mutable_literal(default)
            {
                let name = param.parameter.name.id.to_string();
                let range = default.range();
                let src =
                    self.source[usize::from(range.start())..usize::from(range.end())].to_owned();
                fixups.push((name, range, src));
            }
        }

        if fixups.is_empty() {
            return;
        }

        self.needs_sentinel = true;

        for (_, range, _) in &fixups {
            self.edits.push((*range, "_MISSING".to_owned()));
        }

        // Insert guards after docstring (if any), before first real statement.
        let skip = if matches!(
            func.body.first(),
            Some(Stmt::Expr(e)) if matches!(e.value.as_ref(), Expr::StringLiteral(_))
        ) {
            1
        } else {
            0
        };

        let Some(insert_stmt) = func.body.get(skip).or_else(|| func.body.first()) else {
            return;
        };

        let (insert_at, _) = self.line_info(insert_stmt.range().start());
        let (func_line_start, func_indent) = self.line_info(func.range().start());

        if insert_at == func_line_start {
            // One-liner (`def f(x=[]): ...`): expand the body to multi-line.
            let body_indent = format!("{func_indent}    ");
            let mut guard = String::new();
            for (name, _, default_src) in &fixups {
                guard.push_str(&format!(
                    "{body_indent}if {name} is _MISSING:\n{body_indent}    {name} = {default_src}\n"
                ));
            }
            // Replace the ` <body>` part (space + all body stmts) with the
            // expanded multi-line form.
            let space_pos = usize::from(func.body[0].range().start()) - 1;
            let body_end = usize::from(func.body.last().unwrap().range().end());
            let mut expansion = String::from("\n");
            expansion.push_str(&guard);
            for stmt in &func.body {
                let s = usize::from(stmt.range().start());
                let e = usize::from(stmt.range().end());
                expansion.push_str(&body_indent);
                expansion.push_str(&self.source[s..e]);
                expansion.push('\n');
            }
            let range = TextRange::new(
                TextSize::from(space_pos as u32),
                TextSize::from(body_end as u32),
            );
            self.edits.push((range, expansion));
        } else {
            let indent = self.line_info(insert_stmt.range().start()).1.to_owned();
            let mut guard = String::new();
            for (name, _, default_src) in &fixups {
                guard.push_str(&format!(
                    "{indent}if {name} is _MISSING:\n{indent}    {name} = {default_src}\n"
                ));
            }
            self.edits
                .push((TextRange::new(insert_at, insert_at), guard));
        }
    }
}

impl<'src, 'ast> Visitor<'ast> for MutableDefaultFixer<'src> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        if let Stmt::FunctionDef(func) = stmt {
            self.process_function(func);
        }
        walk_stmt(self, stmt);
    }
}

#[cfg(test)]
mod tests {
    use crate::transpile;
    use indoc::indoc;

    fn check(input: &str, expected: &str) {
        assert_eq!(transpile(input, &crate::Config::default()).unwrap(), expected);
    }

    #[test]
    fn list_default() {
        check(
            indoc! {"
                def f(x=[]):
                    pass
            "},
            indoc! {"
                _MISSING = object()
                def f(x=_MISSING):
                    if x is _MISSING:
                        x = []
                    pass
            "},
        );
    }

    #[test]
    fn dict_default() {
        check(
            indoc! {"
                def f(x={}):
                    pass
            "},
            indoc! {"
                _MISSING = object()
                def f(x=_MISSING):
                    if x is _MISSING:
                        x = {}
                    pass
            "},
        );
    }

    #[test]
    fn set_default() {
        check(
            indoc! {"
                def f(x={1, 2}):
                    pass
            "},
            indoc! {"
                _MISSING = object()
                def f(x=_MISSING):
                    if x is _MISSING:
                        x = {1, 2}
                    pass
            "},
        );
    }

    #[test]
    fn scalar_default_unchanged() {
        check(
            indoc! {"
                def f(x=0):
                    pass
            "},
            indoc! {"
                def f(x=0):
                    pass
            "},
        );
    }

    #[test]
    fn none_default_unchanged() {
        check(
            indoc! {"
                def f(x=None):
                    pass
            "},
            indoc! {"
                def f(x=None):
                    pass
            "},
        );
    }

    #[test]
    fn multiple_mutable_defaults() {
        check(
            indoc! {"
                def f(x=[], y={}):
                    pass
            "},
            indoc! {"
                _MISSING = object()
                def f(x=_MISSING, y=_MISSING):
                    if x is _MISSING:
                        x = []
                    if y is _MISSING:
                        y = {}
                    pass
            "},
        );
    }

    #[test]
    fn preserves_docstring() {
        check(
            indoc! {r#"
                def f(x=[]):
                    """doc"""
                    pass
            "#},
            indoc! {r#"
                _MISSING = object()
                def f(x=_MISSING):
                    """doc"""
                    if x is _MISSING:
                        x = []
                    pass
            "#},
        );
    }

    #[test]
    fn sentinel_defined_once_for_multiple_functions() {
        check(
            indoc! {"
                def f(x=[]):
                    pass
                def g(y={}):
                    pass
            "},
            indoc! {"
                _MISSING = object()
                def f(x=_MISSING):
                    if x is _MISSING:
                        x = []
                    pass
                def g(y=_MISSING):
                    if y is _MISSING:
                        y = {}
                    pass
            "},
        );
    }

    #[test]
    fn ellipsis() {
        check(
            "def f(x=[]): ...",
            indoc! {"
                _MISSING = object()
                def f(x=_MISSING):
                    if x is _MISSING:
                        x = []
                    ...
            "},
        );
    }
}
