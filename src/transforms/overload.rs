//! Rewrites consecutive bodyless function declarations with the same name to
//! `@overload`-decorated stubs.
//!
//! ```python
//! def f(a: int) -> int
//! def f(a: str) -> str
//! def f(a): ...
//! ```
//! →
//! ```python
//! @overload
//! def f(a: int) -> int: ...
//! @overload
//! def f(a: str) -> str: ...
//! def f(a): ...
//! ```
//!
//! Also adds a `: ...` stub body to any standalone bodyless function def
//! (not part of an overload run).
//!
//! Rules:
//! - A run of ≥ 2 consecutive defs with the same name where all but the last
//!   have an empty body becomes an overload group.
//! - Standalone bodyless defs get `: ...` appended (no `@overload`).
//! - Fires at every nesting level (module, class body, nested function body).

use ruff_python_ast::visitor::{Visitor, walk_body, walk_stmt};
use ruff_python_ast::{Expr, Stmt, StmtFunctionDef};
use ruff_text_size::{Ranged, TextRange, TextSize};

pub struct Overload<'src> {
    source: &'src str,
    pub edits: Vec<(TextRange, String)>,
    pub needs_overload: bool,
}

impl<'src> Overload<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            edits: Vec::new(),
            needs_overload: false,
        }
    }

    fn line_indent(&self, pos: TextSize) -> &str {
        let offset = usize::from(pos);
        let line_start = self.source[..offset]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let rest = &self.source[line_start..offset];
        let ws_len = rest.len() - rest.trim_start().len();
        &self.source[line_start..line_start + ws_len]
    }

    fn is_abstract(&self, func: &StmtFunctionDef) -> bool {
        func.decorator_list.iter().any(|dec| {
            let start = usize::from(dec.range().start());
            self.source.as_bytes().get(start).copied() != Some(b'@')
                && matches!(&dec.expression, Expr::Name(n) if n.id.as_str() == "abstract")
        })
    }

    fn add_stub_body(&mut self, func: &StmtFunctionDef) {
        let end = func.range().end();
        let body = if self.is_abstract(func) { ": raise NotImplementedError" } else { ": ..." };
        self.edits.push((TextRange::new(end, end), body.to_owned()));
    }

    fn add_overload_stub(&mut self, func: &StmtFunctionDef) {
        self.needs_overload = true;
        let indent = self.line_indent(func.range().start()).to_owned();
        let start = func.range().start();
        self.edits.push((
            TextRange::new(start, start),
            format!("@overload\n{indent}"),
        ));
        let end = func.range().end();
        self.edits
            .push((TextRange::new(end, end), ": ...".to_owned()));
    }

    fn process_body(&mut self, body: &[Stmt]) {
        let mut i = 0;
        while i < body.len() {
            let Stmt::FunctionDef(first) = &body[i] else {
                i += 1;
                continue;
            };
            let name = first.name.id.as_str();

            // Find the extent of the run of consecutive defs with this name.
            let mut run_end = i + 1;
            while run_end < body.len() {
                if let Stmt::FunctionDef(f) = &body[run_end] {
                    if f.name.id.as_str() == name {
                        run_end += 1;
                        continue;
                    }
                }
                break;
            }

            if run_end > i + 1 {
                // Multiple consecutive defs with the same name.
                let all_but_last_bodyless = body[i..run_end - 1].iter().all(|s| {
                    matches!(s, Stmt::FunctionDef(f) if f.body.is_empty())
                });

                if all_but_last_bodyless {
                    // Proper overload group: annotate all but the last.
                    for stmt in &body[i..run_end - 1] {
                        if let Stmt::FunctionDef(f) = stmt {
                            self.add_overload_stub(f);
                        }
                    }
                    // The last (implementation) gets `: ...` only if bodyless.
                    if let Stmt::FunctionDef(last) = &body[run_end - 1] {
                        if last.body.is_empty() {
                            self.add_stub_body(last);
                        }
                    }
                } else {
                    // Not a clean overload group — handle individually.
                    for stmt in &body[i..run_end] {
                        if let Stmt::FunctionDef(f) = stmt {
                            if f.body.is_empty() {
                                self.add_stub_body(f);
                            }
                        }
                    }
                }
            } else if first.body.is_empty() {
                // Standalone bodyless def.
                self.add_stub_body(first);
            }

            i = run_end;
        }
    }
}

impl<'src, 'ast> Visitor<'ast> for Overload<'src> {
    fn visit_body(&mut self, body: &'ast [Stmt]) {
        self.process_body(body);
        walk_body(self, body);
    }

    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
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
    fn simple_overload() {
        check(
            indoc! {"
                def f(a: int) -> int
                def f(a: str) -> str
                def f(a): ...
            "},
            indoc! {"
                from typing import overload
                @overload
                def f(a: int) -> int: ...
                @overload
                def f(a: str) -> str: ...
                def f(a): ...
            "},
        );
    }

    #[test]
    fn overload_in_class() {
        check(
            indoc! {"
                class A:
                    def method(self, x: int) -> int
                    def method(self, x: str) -> str
                    def method(self, x): ...
            "},
            indoc! {"
                from typing import overload
                class A:
                    @overload
                    def method(self, x: int) -> int: ...
                    @overload
                    def method(self, x: str) -> str: ...
                    def method(self, x): ...
            "},
        );
    }

    #[test]
    fn single_def_not_overloaded() {
        check(
            "def f(a: int) -> int: ...\n",
            "def f(a: int) -> int: ...\n",
        );
    }

    #[test]
    fn bodyless_single_def_gets_stub() {
        check("def f(a: int) -> int\n", "def f(a: int) -> int: ...\n");
    }

    #[test]
    fn regular_arg_unchanged() {
        check("def f(a: int, b: str) -> bool: ...\n", "def f(a: int, b: str) -> bool: ...\n");
    }
}
