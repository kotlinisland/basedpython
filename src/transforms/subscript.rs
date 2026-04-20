use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::{Ranged, TextRange};

use crate::symbol_table::SymbolTable;

/// Normalizes tuple subscripts so the key is unambiguously a 1-tuple:
///   x[a, b]    ->  x[((a, b),)]
///   x[(a, b)]  ->  x[((a, b),)]
///
/// Only fires in value-context subscripts — a subscript whose value resolves
/// to a type (e.g. `Literal[1, 2]`, `dict[str, int]`) keeps its multi-arg slice
/// as type arguments, which is semantically different from a tuple key.
pub struct SubscriptNormalizer<'src, 'sym> {
    source: &'src str,
    symbols: &'sym SymbolTable,
    edits: Vec<(TextRange, String)>,
}

impl<'src, 'sym> SubscriptNormalizer<'src, 'sym> {
    pub fn new(source: &'src str, symbols: &'sym SymbolTable) -> Self {
        Self {
            source,
            symbols,
            edits: Vec::new(),
        }
    }

    pub fn into_edits(self) -> Vec<(TextRange, String)> {
        self.edits
    }

    /// Whether `X[...]` treats `...` as type arguments rather than a subscript
    /// key. If true, don't normalize.
    fn is_type_subscript(&self, value: &Expr) -> bool {
        match value {
            Expr::Name(n) => match self.symbols.resolve(n.id.as_str(), n.range().start()) {
                Some(k) => k.subscript_is_type_context(),
                // Unresolved → assume type (covers builtins like `list`/`dict`
                // and imported names we can't see through).
                None => true,
            },
            Expr::Attribute(a) => match a.value.as_ref() {
                Expr::Name(base) => matches!(
                    self.symbols.resolve(base.id.as_str(), base.range().start()),
                    Some(crate::symbol_table::BindingKind::Import) | None
                ),
                _ => false,
            },
            _ => false,
        }
    }
}

impl<'src, 'sym, 'ast> Visitor<'ast> for SubscriptNormalizer<'src, 'sym> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let Expr::Subscript(sub) = expr
            && let Expr::Tuple(tuple) = sub.slice.as_ref()
            && !self.is_type_subscript(&sub.value)
        {
            let range = tuple.range();
            let start = usize::from(range.start());
            let end = usize::from(range.end());
            let inner = &self.source[start..end];
            let normalized = if tuple.parenthesized {
                format!("{inner},")
            } else {
                format!("({inner}),")
            };
            self.edits.push((range, normalized));
            // Visit value but not slice — we've already handled this subscript
            self.visit_expr(&sub.value);
            return;
        }
        walk_expr(self, expr);
    }
}

#[cfg(test)]
mod tests {
    use crate::transpile;

    fn check(input: &str, expected: &str) {
        assert_eq!(transpile(input, &crate::Config::default()).unwrap(), expected);
    }

    #[test]
    fn parenthesized_tuple_slice() {
        // `x` is unresolved — default "type" wouldn't normalize. Use a clearly
        // value-bound name so the normalization fires.
        check("d = {}\nv = d[(a, b)]\n", "d = {}\nv = d[(a, b),]\n");
    }

    #[test]
    fn bare_tuple_slice() {
        check("d = {}\nv = d[a, b]\n", "d = {}\nv = d[(a, b),]\n");
    }

    #[test]
    fn non_tuple_slice_unchanged() {
        check("d = {}\nv = d[a]\n", "d = {}\nv = d[a]\n");
    }

    #[test]
    fn nested_subscript() {
        check(
            "d = {}\nv = d[(a, b)][c]\n",
            "d = {}\nv = d[(a, b),][c]\n",
        );
    }

    #[test]
    fn type_alias_subscript_not_normalized() {
        check(
            "from typing import Literal\na: Literal[1, 2]\n",
            "from typing import Literal\na: Literal[1, 2]\n",
        );
    }

    #[test]
    fn class_subscript_not_normalized() {
        check(
            "class A: ...\nx: A[int, str]\n",
            "class A: ...\nx: A[int, str]\n",
        );
    }

    #[test]
    fn unknown_name_treated_as_type() {
        // `list` isn't in our symbol table (it's a builtin) — unresolved
        // defaults to type context, so the slice is NOT normalized.
        check("a: list[int, str]\n", "a: list[int, str]\n");
    }
}
