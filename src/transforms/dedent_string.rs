use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_python_ast::{Expr, Stmt, StringFlags};
use ruff_text_size::{Ranged, TextRange};

/// strips common indentation from triple-quoted multiline strings at compile time
///
/// ```bython
/// text = """
///     start-of-line
///     """
/// ```
/// →
/// ```python
/// text = """\
/// start-of-line\
/// """
/// ```
///
/// fires on all triple-quoted single-part strings (plain, f/t/r)
/// that open with `"""\n` and whose content is consistently indented
pub struct DedentString<'src> {
    source: &'src str,
    pub edits: Vec<(TextRange, String)>,
}

impl<'src> DedentString<'src> {
    pub fn new(source: &'src str) -> Self {
        Self { source, edits: Vec::new() }
    }

    fn thing(&mut self, string_expr: &dyn Ranged) {
        let start = usize::from(string_expr.range().start());
        let end = usize::from(string_expr.range().end());
        let raw = &self.source[start..end];
        if let Some(transformed) = dedent_triple_string(raw) {
            self.edits.push((string_expr.range(), transformed));
        }
    }
}

impl<'src, 'ast> Visitor<'ast> for DedentString<'src> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        match expr {
            Expr::StringLiteral(string_expr) => {
                if let Some(part) = string_expr.as_single_part_string() {
                    let flags = part.flags;
                    if flags.is_triple_quoted() {
                        self.thing(string_expr)
                    }
                }
            }
            Expr::FString(string_expr) => {
                if let Some(fstring) = string_expr.as_single_part_fstring() {
                    if fstring.flags.is_triple_quoted() {
                        self.thing(string_expr)
                    }
                }
            }
            Expr::TString(string_expr) => {
                if let Some(tstring) = string_expr.as_single_part_tstring() {
                    if tstring.flags.is_triple_quoted() {
                        self.thing(string_expr)
                    }
                }
            }
            _ => {}
        }
        walk_expr(self, expr);
    }
}


fn dedent_triple_string(raw: &str) -> Option<String> {
    let quote_start = raw.find("\"\"\"").or_else(|| raw.find("'''"))?;
    let prefix = &raw[..quote_start];
    let quote = &raw[quote_start..quote_start + 3];
    let after_open = &raw[quote_start + 3..];

    if !after_open.ends_with(quote) {
        return None;
    }
    let inner = &after_open[..after_open.len() - 3];

    if !inner.starts_with('\n') {
        return None;
    }
    let inner = &inner[1..];

    let lines: Vec<&str> = inner.split('\n').collect();
    let last = lines.last()?;
    if !last.chars().all(|c| c == ' ' || c == '\t') {
        return None;
    }

    let content_lines = &lines[..lines.len() - 1];
    if content_lines.is_empty() {
        return None;
    }

    let indent = find_common_indent(content_lines);
    if indent.is_empty() {
        return None;
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
    Some(format!("{prefix}{quote}\\\n{content}\\\n{quote}"))
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
    use super::dedent_triple_string;
    use crate::{transpile, Config};
    use indoc::indoc;

    fn check(input: &str, expected: &str) {
        assert_eq!(transpile(input, &Config::default()).unwrap(), expected);
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
        check(
            "text = \"\"\"hello\"\"\"\n",
            "text = \"\"\"hello\"\"\"\n",
        );
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
        assert_eq!(
            dedent_triple_string("\"\"\"\n    hello\n    \"\"\""),
            Some("\"\"\"\\\nhello\\\n\"\"\"".to_owned())
        );
    }

    #[test]
    fn unit_dedent_no_indent_returns_none() {
        assert_eq!(dedent_triple_string("\"\"\"\nhello\n\"\"\""), None);
    }
}
