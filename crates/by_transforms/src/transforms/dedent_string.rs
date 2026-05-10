//! strips common indentation from triple-quoted multiline strings

use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_python_ast::{Expr, ModModule, Stmt, StringFlags};
use ruff_text_size::{Ranged, TextRange};

use super::ast_driver::{AstPass, PassContext};

pub(crate) struct DedentString<'src> {
    source: &'src str,
}

impl<'src> DedentString<'src> {
    pub(crate) fn new(source: &'src str) -> Self {
        Self { source }
    }
}

impl AstPass for DedentString<'_> {
    fn run(&self, module: &mut ModModule, ctx: &mut PassContext) {
        let mut state = State {
            source: self.source,
            edits: Vec::new(),
            errors: Vec::new(),
        };
        for stmt in &module.body {
            state.visit_stmt(stmt);
        }
        ctx.text_edits.extend(state.edits);
        ctx.errors.extend(state.errors);
    }
}

struct State<'src> {
    source: &'src str,
    edits: Vec<(TextRange, String)>,
    errors: Vec<String>,
}

impl State<'_> {
    fn dedent(&mut self, string_expr: &dyn Ranged) {
        let start = usize::from(string_expr.range().start());
        let end = usize::from(string_expr.range().end());
        let raw = &self.source[start..end];
        match dedent_triple_string(raw) {
            DedentOutcome::Transformed(out) => {
                self.edits.push((string_expr.range(), out));
            }
            DedentOutcome::Error(msg) => self.errors.push(msg),
            DedentOutcome::Unchanged => {}
        }
    }
}

impl<'ast> Visitor<'ast> for State<'_> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        match expr {
            Expr::StringLiteral(string_expr) => {
                if let Some(part) = string_expr.as_single_part_string() {
                    let flags = part.flags;
                    if flags.is_triple_quoted() {
                        self.dedent(string_expr);
                    }
                }
            }
            Expr::FString(string_expr) => {
                if let Some(fstring) = string_expr.as_single_part_fstring() {
                    if fstring.flags.is_triple_quoted() {
                        self.dedent(string_expr);
                    }
                }
            }
            Expr::TString(string_expr) => {
                if let Some(tstring) = string_expr.as_single_part_tstring() {
                    if tstring.flags.is_triple_quoted() {
                        self.dedent(string_expr);
                    }
                }
            }
            _ => {}
        }
        walk_expr(self, expr);
    }
}

pub(crate) enum DedentOutcome {
    Unchanged,
    Transformed(String),
    Error(String),
}

fn dedent_triple_string(raw: &str) -> DedentOutcome {
    let Some(quote_start) = raw.find("\"\"\"").or_else(|| raw.find("'''")) else {
        return DedentOutcome::Unchanged;
    };
    let prefix = &raw[..quote_start];
    let quote = &raw[quote_start..quote_start + 3];
    let after_open = &raw[quote_start + 3..];

    if !after_open.ends_with(quote) {
        return DedentOutcome::Unchanged;
    }
    let inner = &after_open[..after_open.len() - 3];

    if !inner.starts_with('\n') {
        return DedentOutcome::Unchanged;
    }
    let inner = &inner[1..];

    let lines: Vec<&str> = inner.split('\n').collect();
    let Some(last) = lines.last() else {
        return DedentOutcome::Unchanged;
    };
    if !last.chars().all(|c| c == ' ' || c == '\t') {
        return DedentOutcome::Unchanged;
    }
    let closing_indent = *last;

    let content_lines = &lines[..lines.len() - 1];
    if content_lines.is_empty() {
        return DedentOutcome::Unchanged;
    }

    let has_text = content_lines.iter().any(|l| !l.trim().is_empty());
    let indent = find_common_indent(content_lines);

    // closing `"""` indented more than the actual content of the string has
    // no consistent dedent interpretation — refuse rather than silently produce
    // a misaligned literal
    if has_text && closing_indent.len() > indent.len() {
        return DedentOutcome::Error(format!(
            "closing `{quote}` is indented more than the content of the triple-quoted string"
        ));
    }

    if indent.is_empty() {
        return DedentOutcome::Unchanged;
    }

    let deindented: Vec<&str> = content_lines
        .iter()
        .map(|l| {
            if l.trim().is_empty() {
                ""
            } else {
                &l[indent.len()..]
            }
        })
        .collect();

    let content = deindented.join("\n");
    DedentOutcome::Transformed(format!("{prefix}{quote}\\\n{content}\\\n{quote}"))
}

fn find_common_indent(lines: &[&str]) -> String {
    let mut indent: Option<String> = None;
    for line in lines.iter().filter(|l| !l.trim().is_empty()) {
        let line_indent = leading_whitespace(line).to_owned();
        indent = Some(match indent {
            None => line_indent,
            Some(prev) => common_string_prefix(&prev, &line_indent),
        });
    }
    indent.unwrap_or_default()
}

fn leading_whitespace(s: &str) -> &str {
    let end = s.find(|c: char| c != ' ' && c != '\t').unwrap_or(s.len());
    &s[..end]
}

fn common_string_prefix(a: &str, b: &str) -> String {
    a.chars()
        .zip(b.chars())
        .take_while(|(x, y)| x == y)
        .map(|(c, _)| c)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{DedentOutcome, dedent_triple_string};
    use crate::python_passthrough::unchanged;
    use crate::{Config, transpile};
    use indoc::indoc;

    fn check(input: &str, expected: &str) {
        assert_eq!(
            transpile(input, &Config::test_default()).unwrap(),
            crate::python_passthrough::lazify_expected(expected)
        );
    }

    #[test]
    fn basic_dedent() {
        check(
            indoc! {r#"
                text = """
                    start-of-line
                    |
                    """
            "#},
            indoc! {r#"
                text = """\
                start-of-line
                |\
                """
            "#},
        );
    }

    #[test]
    fn fstring_dedent() {
        check(
            indoc! {r#"
                a = "asdf"
                text = f"""
                    start-of-line{a}
                    |
                    """
            "#},
            indoc! {r#"
                a = "asdf"
                text = f"""\
                start-of-line{a}
                |\
                """
            "#},
        );
    }

    #[test]
    fn tstring_dedent() {
        check(
            indoc! {r#"
                a = "asdf"
                text = t"""
                    start-of-line{a}
                    |
                    """
            "#},
            indoc! {r#"
                a = "asdf"
                text = t"""\
                start-of-line{a}
                |\
                """
            "#},
        );
    }

    #[test]
    fn rstring_dedent() {
        check(
            indoc! {r#"
                text = r"""
                    start-of-line
                    |
                    """
            "#},
            indoc! {r#"
                text = r"""\
                start-of-line
                |\
                """
            "#},
        );
    }

    #[test]
    fn multiline_content() {
        check(
            indoc! {r#"
                text = """
                    line 1
                    line 2
                    """
            "#},
            indoc! {r#"
                text = """\
                line 1
                line 2\
                """
            "#},
        );
    }

    #[test]
    fn single_quoted_triple() {
        check(
            "text = '''\n    hello\n    '''\n",
            "text = '''\\\nhello\\\n'''\n",
        );
    }

    #[test]
    fn single_quoted_unchanged() {
        check("text = \"hello\"\n", "text = \"hello\"\n");
    }

    #[test]
    fn not_indented_unchanged() {
        // no common indent → no transform
        check(
            "text = \"\"\"\nhello\n\"\"\"\n",
            "text = \"\"\"\nhello\n\"\"\"\n",
        );
    }

    #[test]
    fn inline_triple_unchanged() {
        // no leading newline → no transform
        check("text = \"\"\"hello\"\"\"\n", "text = \"\"\"hello\"\"\"\n");
    }

    #[test]
    fn unicode_prefix_dedented() {
        check(
            "text = u\"\"\"\n    hello\n    \"\"\"\n",
            "text = u\"\"\"\\\nhello\\\n\"\"\"\n",
        );
    }

    #[test]
    fn unit_find_common_indent_spaces() {
        assert_eq!(super::find_common_indent(&["    a", "    b"]), "    ");
    }

    #[test]
    fn unit_find_common_indent_partial() {
        assert_eq!(super::find_common_indent(&["    a", "  b"]), "  ");
    }

    #[test]
    fn unit_dedent_basic() {
        match dedent_triple_string("\"\"\"\n    hello\n    \"\"\"") {
            DedentOutcome::Transformed(s) => assert_eq!(s, "\"\"\"\\\nhello\\\n\"\"\""),
            _ => panic!("expected Transformed"),
        }
    }

    #[test]
    fn unit_dedent_no_indent_returns_unchanged() {
        assert!(matches!(
            dedent_triple_string("\"\"\"\nhello\n\"\"\""),
            DedentOutcome::Unchanged
        ));
    }

    #[test]
    fn closing_more_indented_errors() {
        let err = transpile(
            indoc! {r#"
                text = """
                  asdf
                    """
            "#},
            &Config::test_default(),
        )
        .unwrap_err();
        assert!(
            err.contains("closing") && err.contains("indented more"),
            "got: {err}"
        );
    }

    #[test]
    fn closing_more_indented_zero_content_indent_errors() {
        let err = transpile(
            indoc! {r#"
                text = """
                asdf
                    """
            "#},
            &Config::test_default(),
        )
        .unwrap_err();
        assert!(err.contains("indented more"), "got: {err}");
    }

    #[test]
    fn closing_equal_indent_ok() {
        // closing at exactly the content's min indent — fine, dedents to col 0
        check(
            indoc! {r#"
                text = """
                    asdf
                    """
            "#},
            indoc! {r#"
                text = """\
                asdf\
                """
            "#},
        );
    }

    #[test]
    fn closing_more_indented_blank_content_unchanged() {
        // no real text inside the triple; the over-indent rule doesn't apply
        check(
            "text = \"\"\"\n\n    \"\"\"\n",
            "text = \"\"\"\n\n    \"\"\"\n",
        );
    }

    #[test]
    fn python_unchanged() {
        unchanged(indoc! {r#"
            text = """
                hello
                """
        "#});
    }
}
