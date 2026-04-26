//! auto-quotes forward self-references in class definitions
//!
//! `class A(list[A])` → `class A(list["A"])`
//!
//! the class name appearing as a subscript slice argument in base classes or
//! the class body is replaced with a string literal — a PEP 484 forward
//! reference resolvable by type checkers without `from __future__ import annotations`
//!
//! fires when the name is inside a subscript slice; direct bases (`class A(A):`)
//! are left alone — that is a runtime error regardless of quoting

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
        let class_name = class.name.id.as_str();

        if let Some(args) = &class.arguments {
            for base in &args.args {
                self.find_self_refs_in_subscript_slice(base, class_name);
            }
        }

        self.quote_self_refs_in_body(&class.body, class_name);
    }

    fn quote_self_refs_in_body(&mut self, stmts: &[Stmt], class_name: &str) {
        for stmt in stmts {
            match stmt {
                Stmt::Expr(e) => self.walk_expr_for_subscripts(e.value.as_ref(), class_name),
                Stmt::Assign(a) => self.walk_expr_for_subscripts(a.value.as_ref(), class_name),
                Stmt::AnnAssign(a) => {
                    self.find_self_refs_in_subscript_slice(a.annotation.as_ref(), class_name);
                    if let Some(value) = &a.value {
                        self.walk_expr_for_subscripts(value.as_ref(), class_name);
                    }
                }
                Stmt::FunctionDef(f) => {
                    for param in f.parameters.iter_non_variadic_params() {
                        if let Some(ann) = &param.parameter.annotation {
                            self.find_self_refs_in_subscript_slice(ann, class_name);
                        }
                    }
                    if let Some(var) = &f.parameters.vararg {
                        if let Some(ann) = &var.annotation {
                            self.find_self_refs_in_subscript_slice(ann, class_name);
                        }
                    }
                    if let Some(kwarg) = &f.parameters.kwarg {
                        if let Some(ann) = &kwarg.annotation {
                            self.find_self_refs_in_subscript_slice(ann, class_name);
                        }
                    }
                    if let Some(ret) = &f.returns {
                        self.find_self_refs_in_subscript_slice(ret, class_name);
                    }
                }
                _ => {}
            }
        }
    }

    // only descends into callee/attribute positions — `f(A)` untouched, `list[A]()` → `list["A"]()`
    fn walk_expr_for_subscripts(&mut self, expr: &Expr, class_name: &str) {
        match expr {
            Expr::Subscript(s) => {
                self.quote_name_in_type_arg(s.slice.as_ref(), class_name);
                self.walk_expr_for_subscripts(&s.value, class_name);
            }
            Expr::Call(c) => {
                self.walk_expr_for_subscripts(&c.func, class_name);
            }
            Expr::Attribute(a) => {
                self.walk_expr_for_subscripts(&a.value, class_name);
            }
            _ => {}
        }
    }

    fn find_self_refs_in_subscript_slice(&mut self, expr: &Expr, class_name: &str) {
        match expr {
            Expr::Subscript(s) => {
                self.quote_name_in_type_arg(s.slice.as_ref(), class_name);
            }
            Expr::BinOp(b) => {
                // `A | B` union base — propagate into both arms
                self.find_self_refs_in_subscript_slice(&b.left, class_name);
                self.find_self_refs_in_subscript_slice(&b.right, class_name);
            }
            _ => {}
        }
    }

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
                // value position is the type name (e.g. `list`) — skip it, only descend into slice
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
    fn body_expr_stmt_call() {
        check(
            indoc! {"
                class A(list[A], dict[int]):
                    list[A]()
            "},
            indoc! {"
                class A(list[\"A\"], dict[int]):
                    list[\"A\"]()
            "},
        );
    }

    #[test]
    fn body_ann_assign() {
        check(
            indoc! {"
                class A(list[A]):
                    x: list[A] = list[A]()
            "},
            indoc! {"
                class A(list[\"A\"]):
                    x: list[\"A\"] = list[\"A\"]()
            "},
        );
    }

    #[test]
    fn body_method_annotations() {
        check(
            indoc! {"
                class A(list[A]):
                    def method(self, x: list[A]) -> list[A]: ...
            "},
            indoc! {"
                class A(list[\"A\"]):
                    def method(self, x: list[\"A\"]) -> list[\"A\"]: ...
            "},
        );
    }

    #[test]
    fn body_method_body_not_quoted() {
        // method body runs after A is defined — no quoting needed
        check(
            indoc! {"
                class A(list[A]):
                    def method(self):
                        return list[A]()
            "},
            indoc! {"
                class A(list[\"A\"]):
                    def method(self):
                        return list[A]()
            "},
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
