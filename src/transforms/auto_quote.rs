//! Auto-quotes forward self-references in class base subscripts.
//!
//! `class A(list[A])` → `class A(list["A"])`
//!
//! When the class name appears as a name expression inside a subscript slice
//! in the base-class list, it is replaced with a string literal.  This makes
//! the reference a proper PEP 484 forward reference that can be resolved by
//! type checkers without `from __future__ import annotations`.
//!
//! Only fires when the occurrence is inside a subscript slice, not when it is
//! a direct base (e.g. `class A(A):` is left alone — that is a runtime error
//! regardless of quoting, and not the auto-quoting use-case).

use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{Expr, Stmt, StmtClassDef};
use ruff_text_size::{Ranged, TextRange};

pub struct AutoQuote<'src> {
    source: &'src str,
    pub edits: Vec<(TextRange, String)>,
}

impl<'src> AutoQuote<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            edits: Vec::new(),
        }
    }

    fn process_class(&mut self, class: &StmtClassDef) {
        let Some(args) = &class.arguments else {
            return;
        };
        let class_name = class.name.id.as_str();

        for base in &args.args {
            // Only look inside subscripts, not at direct base references.
            self.find_self_refs_in_subscript_slice(base, class_name);
        }
    }

    /// Recursively walk `expr` looking for subscripts, then quote any
    /// occurrence of `class_name` inside their slice.
    fn find_self_refs_in_subscript_slice(&mut self, expr: &Expr, class_name: &str) {
        match expr {
            Expr::Subscript(s) => {
                self.quote_name_in_type_arg(s.slice.as_ref(), class_name);
            }
            Expr::BinOp(b) => {
                // `A | B` in a base — propagate into both arms.
                self.find_self_refs_in_subscript_slice(&b.left, class_name);
                self.find_self_refs_in_subscript_slice(&b.right, class_name);
            }
            _ => {}
        }
    }

    /// Recursively scan a type-argument expression for `class_name` and quote
    /// every occurrence.
    fn quote_name_in_type_arg(&mut self, expr: &Expr, class_name: &str) {
        match expr {
            Expr::Name(n) if n.id.as_str() == class_name => {
                let raw = &self.source[usize::from(n.range().start())..usize::from(n.range().end())];
                self.edits.push((n.range(), format!("\"{raw}\"")));
            }
            Expr::Tuple(t) => {
                for e in &t.elts {
                    self.quote_name_in_type_arg(e, class_name);
                }
            }
            Expr::Subscript(s) => {
                // The value position is a type name, not a type arg — skip it.
                self.quote_name_in_type_arg(s.slice.as_ref(), class_name);
            }
            Expr::BinOp(b) => {
                self.quote_name_in_type_arg(&b.left, class_name);
                self.quote_name_in_type_arg(&b.right, class_name);
            }
            _ => {}
        }
    }
}

impl<'src, 'ast> Visitor<'ast> for AutoQuote<'src> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        if let Stmt::ClassDef(c) = stmt {
            self.process_class(c);
        }
        walk_stmt(self, stmt);
    }
}

#[cfg(test)]
mod tests {
    use crate::{transpile, Config};
    use indoc::indoc;

    fn check(input: &str, expected: &str) {
        assert_eq!(transpile(input, &Config::default()).unwrap(), expected);
    }

    #[test]
    fn simple_self_reference() {
        check("class A(list[A]): ...\n", "class A(list[\"A\"]): ...\n");
    }

    #[test]
    fn nested_self_reference() {
        check(
            "class Tree(Node[Tree]): ...\n",
            "class Tree(Node[\"Tree\"]): ...\n",
        );
    }

    #[test]
    fn self_ref_in_union() {
        check(
            "class A(list[A | None]): ...\n",
            "class A(list[\"A\" | None]): ...\n",
        );
    }

    #[test]
    fn self_ref_in_nested_subscript() {
        check(
            "class A(dict[str, list[A]]): ...\n",
            "class A(dict[str, list[\"A\"]]): ...\n",
        );
    }

    #[test]
    fn direct_base_not_quoted() {
        // `class A(A):` is a direct base, not a subscript arg — leave alone.
        check("class A(A): ...\n", "class A(A): ...\n");
    }

    #[test]
    fn other_names_not_quoted() {
        check(
            "class A(list[B]): ...\n",
            "class A(list[B]): ...\n",
        );
    }

    #[test]
    fn multiple_occurrences() {
        check(
            "class A(Union[A, A]): ...\n",
            "class A(Union[\"A\", \"A\"]): ...\n",
        );
    }

    #[test]
    fn nested_class_inner_quotes_own_name() {
        check(
            indoc! {"
                class Outer:
                    class Inner(list[Inner]): ...
            "},
            indoc! {"
                class Outer:
                    class Inner(list[\"Inner\"]): ...
            "},
        );
    }
}
