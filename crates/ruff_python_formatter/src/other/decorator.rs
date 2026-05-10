use ruff_formatter::write;
use ruff_python_ast::Decorator;
use ruff_text_size::Ranged;

use crate::expression::maybe_parenthesize_expression;
use crate::expression::parentheses::Parenthesize;
use crate::verbatim::verbatim_text;
use crate::{has_skip_comment, prelude::*};

#[derive(Default)]
pub struct FormatDecorator;

impl FormatNodeRule<Decorator> for FormatDecorator {
    fn fmt_fields(&self, item: &Decorator, f: &mut PyFormatter) -> FormatResult<()> {
        let comments = f.context().comments();
        let trailing = comments.trailing(item);

        // basedpython modifier keywords (`final`, `abstract`, `data`, etc.) are
        // synthesized as decorators by the parser. The decorator's source range
        // covers the keyword text plus trailing whitespace up to the following
        // `class`/`def` token, so it doesn't begin with `@`. Emit those
        // verbatim so we don't rewrite `final class A` to `@final\nclass A`.
        if is_synthetic_modifier(item, f.context().source()) {
            return verbatim_text(item.range()).fmt(f);
        }

        if has_skip_comment(trailing, f.context().source()) {
            comments.mark_verbatim_node_comments_formatted(item.into());

            verbatim_text(item.range()).fmt(f)
        } else {
            let Decorator {
                expression,
                range: _,
                node_index: _,
            } = item;

            write!(
                f,
                [
                    token("@"),
                    maybe_parenthesize_expression(expression, item, Parenthesize::Optional)
                ]
            )
        }
    }
}

pub(crate) fn is_synthetic_modifier(item: &Decorator, source: &str) -> bool {
    source
        .as_bytes()
        .get(usize::from(item.range().start()))
        .copied()
        != Some(b'@')
}
