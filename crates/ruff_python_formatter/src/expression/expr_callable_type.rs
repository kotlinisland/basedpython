use ruff_python_ast::ExprCallableType;
use ruff_text_size::Ranged;

use crate::prelude::*;
use crate::verbatim::verbatim_text;

#[derive(Default)]
pub struct FormatExprCallableType;

impl FormatNodeRule<ExprCallableType> for FormatExprCallableType {
    fn fmt_fields(&self, item: &ExprCallableType, f: &mut PyFormatter) -> FormatResult<()> {
        verbatim_text(item.range()).fmt(f)
    }
}
