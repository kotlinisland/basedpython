//! Reverse-style normalization for type subscripts:
//!   `dict[(int, str)]` → `dict[int, str]`
//!
//! When a type subscript's slice is a parenthesized tuple, strip the parens.
//! Both forms parse to the same AST, but basedpython's preferred surface form
//! omits the redundant parentheses.
//!
//! Value-context subscripts (e.g. `d[(a, b),]` produced by the forward
//! `subscript` transform) are intentionally left alone here: unwrapping the
//! synthesized 1-tuple is lossy when an author wrote a real 1-tuple key.

use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::{Ranged, TextRange, TextSize};

use crate::symbol_table::{BindingKind, SymbolTable};

pub struct SubscriptReverse<'src, 'sym> {
    source: &'src str,
    symbols: &'sym SymbolTable,
    pub edits: Vec<(TextRange, String)>,
}

impl<'src, 'sym> SubscriptReverse<'src, 'sym> {
    pub fn new(source: &'src str, symbols: &'sym SymbolTable) -> Self {
        Self {
            source,
            symbols,
            edits: Vec::new(),
        }
    }

    fn is_type_subscript(&self, value: &Expr) -> bool {
        match value {
            Expr::Name(n) => match self.symbols.resolve(n.id.as_str(), n.range().start()) {
                Some(k) => k.subscript_is_type_context(),
                // Unresolved → assume type (matches the forward transform's default).
                None => true,
            },
            Expr::Attribute(a) => match a.value.as_ref() {
                Expr::Name(base) => matches!(
                    self.symbols.resolve(base.id.as_str(), base.range().start()),
                    Some(BindingKind::Import) | None
                ),
                _ => false,
            },
            _ => false,
        }
    }
}

impl<'src, 'sym, 'ast> Visitor<'ast> for SubscriptReverse<'src, 'sym> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let Expr::Subscript(s) = expr
            && self.is_type_subscript(&s.value)
            && let Expr::Tuple(t) = s.slice.as_ref()
            && t.parenthesized
        {
            // Strip exactly one outer paren on each side of the tuple's range.
            let outer = t.range();
            let inner_start = outer.start() + TextSize::from(1);
            let inner_end = outer.end() - TextSize::from(1);
            let inner =
                self.source[usize::from(inner_start)..usize::from(inner_end)].to_owned();
            self.edits.push((outer, inner));
        }
        walk_expr(self, expr);
    }
}

#[cfg(test)]
mod tests {
    use crate::{Config, reverse_transpile};

    fn check(input: &str, expected: &str) {
        assert_eq!(reverse_transpile(input, &Config::default()).unwrap(), expected);
    }

    #[test]
    fn dict_with_paren_tuple() {
        check("a: dict[(int, str)]\n", "a: dict[int, str]\n");
    }

    #[test]
    fn list_with_paren_tuple() {
        // `list[(int,)]` is unusual but the rule applies the same way.
        check("a: tuple[(int, str, float)]\n", "a: tuple[int, str, float]\n");
    }

    #[test]
    fn already_unparenthesized_unchanged() {
        check("a: dict[int, str]\n", "a: dict[int, str]\n");
    }

    #[test]
    fn scalar_slice_unchanged() {
        check("a: list[int]\n", "a: list[int]\n");
    }

    #[test]
    fn value_subscript_unchanged() {
        // Value subscripts are intentionally not normalized in reverse: the
        // forward transform's 1-tuple-of-tuple wrapping isn't safely reversible
        // (would change the meaning of an author-written 1-tuple key).
        check(
            "d = {}\nv = d[(a, b),]\n",
            "d = {}\nv = d[(a, b),]\n",
        );
    }

    #[test]
    fn nested_inside_optional() {
        check(
            "a: dict[(int, str)] | None\n",
            "a: dict[int, str] | None\n",
        );
    }
}
