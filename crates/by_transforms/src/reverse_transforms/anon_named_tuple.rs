//! Reverse of `crate::transforms::anon_named_tuple`.
//!
//! Detects synthesized `_AnonNamedTuple_<hash>` classes (recognized by name
//! prefix and a `NamedTuple` base) and rewrites references to the anonymous
//! named tuple surface form `(name: T, name: T, ...)`. The class definitions
//! themselves are deleted.
//!
//! User-defined `NamedTuple` subclasses (anything not matching the synth name
//! prefix) are left untouched.

use std::collections::HashMap;

use ruff_diagnostics::{Edit, Fix};
use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_python_ast::{Expr, Stmt, StmtClassDef};
use ruff_text_size::Ranged;

const SYNTH_NAME_PREFIX: &str = "_AnonNamedTuple_";

pub(crate) struct AnonNamedTupleReverse<'src> {
    source: &'src str,
    /// `class_name → reverse-rendered "(name: T, ...)"` text (type form).
    rendered: HashMap<String, String>,
    /// `class_name → ordered field names` for rewriting constructor call
    /// sites back to value-form `(name=arg, ...)`.
    field_names: HashMap<String, Vec<String>>,
    pub(crate) edits: Vec<Fix>,
}

impl<'src> AnonNamedTupleReverse<'src> {
    pub(crate) fn new(source: &'src str, stmts: &[Stmt]) -> Self {
        let mut this = Self {
            source,
            rendered: HashMap::new(),
            field_names: HashMap::new(),
            edits: Vec::new(),
        };
        for stmt in stmts {
            if let Stmt::ClassDef(class) = stmt {
                if let Some((rendered, names)) = this.try_render_synthesized(class) {
                    let class_name = class.name.id.as_str().to_owned();
                    this.rendered.insert(class_name.clone(), rendered);
                    this.field_names.insert(class_name, names);
                    let mut end = class.range().end();
                    let bytes = this.source.as_bytes();
                    let end_idx = usize::from(end);
                    if end_idx < bytes.len() && bytes[end_idx] == b'\n' {
                        end += ruff_text_size::TextSize::from(1);
                    }
                    this.edits.push(Fix::safe_edit(Edit::range_deletion(
                        ruff_text_size::TextRange::new(class.range().start(), end),
                    )));
                }
            }
        }
        this
    }

    fn try_render_synthesized(&self, class: &StmtClassDef) -> Option<(String, Vec<String>)> {
        if !class.name.id.as_str().starts_with(SYNTH_NAME_PREFIX) {
            return None;
        }
        let base_is_named_tuple = class.arguments.as_ref().is_some_and(|args| {
            args.args.iter().any(|arg| match arg {
                Expr::Name(name) => name.id == "NamedTuple",
                Expr::Attribute(attr) => attr.attr.id == "NamedTuple",
                _ => false,
            })
        });
        if !base_is_named_tuple {
            return None;
        }

        let mut fields: Vec<(String, String)> = Vec::new();
        for s in &class.body {
            match s {
                Stmt::AnnAssign(a) => {
                    let Expr::Name(target) = a.target.as_ref() else {
                        return None;
                    };
                    // a default value carries author intent we can't preserve
                    // in the anonymous form
                    if a.value.is_some() {
                        return None;
                    }
                    let field_name = target.id.as_str().to_owned();
                    let type_src = self.src(a.annotation.range()).to_owned();
                    fields.push((field_name, type_src));
                }
                _ => return None,
            }
        }
        if fields.is_empty() {
            return None;
        }

        let mut out = String::from("(");
        for (i, (name, ty)) in fields.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(name);
            out.push_str(": ");
            out.push_str(ty);
        }
        out.push(')');
        let names = fields.into_iter().map(|(n, _)| n).collect();
        Some((out, names))
    }

    fn src(&self, range: ruff_text_size::TextRange) -> &str {
        &self.source[usize::from(range.start())..usize::from(range.end())]
    }
}

impl<'ast> Visitor<'ast> for AnonNamedTupleReverse<'_> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let Expr::Call(call) = expr {
            if let Expr::Name(callee) = call.func.as_ref() {
                if let Some(names) = self.field_names.get(callee.id.as_str()) {
                    let positional = &call.arguments.args;
                    if positional.len() == names.len()
                        && call.arguments.keywords.is_empty()
                        && positional.iter().all(|a| !a.is_starred_expr())
                    {
                        let mut out = String::from("(");
                        for (i, (name, arg)) in names.iter().zip(positional.iter()).enumerate() {
                            if i > 0 {
                                out.push_str(", ");
                            }
                            out.push_str(name);
                            out.push('=');
                            out.push_str(self.src(arg.range()));
                        }
                        out.push(')');
                        self.edits
                            .push(Fix::safe_edit(Edit::range_replacement(out, call.range())));
                        return;
                    }
                }
            }
        }
        if let Expr::Name(name) = expr {
            if let Some(rendered) = self.rendered.get(name.id.as_str()).cloned() {
                self.edits.push(Fix::safe_edit(Edit::range_replacement(
                    rendered,
                    name.range(),
                )));
            }
        }
        walk_expr(self, expr);
    }
}

#[cfg(test)]
mod tests {
    use crate::{Config, reverse_transpile};
    use indoc::indoc;

    fn check(input: &str, expected: &str) {
        assert_eq!(
            reverse_transpile(input, &Config::test_default()).unwrap(),
            expected
        );
    }

    #[test]
    fn type_alias_round_trip() {
        check(
            indoc! {"
                from typing import NamedTuple
                class _AnonNamedTuple_abc12345(NamedTuple):
                    name: str
                    age: int

                a = _AnonNamedTuple_abc12345
            "},
            indoc! {"
                from typing import NamedTuple

                a = (name: str, age: int)
            "},
        );
    }

    #[test]
    fn parameter_and_return() {
        check(
            indoc! {"
                from typing import NamedTuple
                class _AnonNamedTuple_x(NamedTuple):
                    name: str
                    age: int

                def foo(x: _AnonNamedTuple_x) -> _AnonNamedTuple_x:
                    return (\"asdf\", 1)
            "},
            indoc! {"
                from typing import NamedTuple

                def foo(x: (name: str, age: int)) -> (name: str, age: int):
                    return (\"asdf\", 1)
            "},
        );
    }

    #[test]
    fn constructor_call_round_trip() {
        // `_AnonNamedTuple_xxx("asdf", 1)` → `(name="asdf", age=1)`.
        check(
            indoc! {"
                from typing import NamedTuple
                class _AnonNamedTuple_x(NamedTuple):
                    name: str
                    age: int

                a = _AnonNamedTuple_x(\"asdf\", 1)
            "},
            indoc! {"
                from typing import NamedTuple

                a = (name=\"asdf\", age=1)
            "},
        );
    }

    #[test]
    fn return_constructor_call_round_trip() {
        // The constructor call rewrites to value-form `(name=v, ...)`. Forward
        // transpile of the value-form re-emits the constructor, so this is a
        // closed loop.
        check(
            indoc! {"
                from typing import NamedTuple
                class _AnonNamedTuple_x(NamedTuple):
                    name: str
                    age: int

                def f() -> _AnonNamedTuple_x:
                    return _AnonNamedTuple_x(\"asdf\", 1)
            "},
            indoc! {"
                from typing import NamedTuple

                def f() -> (name: str, age: int):
                    return (name=\"asdf\", age=1)
            "},
        );
    }

    #[test]
    fn user_defined_named_tuple_unchanged() {
        // Anything not matching `_AnonNamedTuple_` prefix stays as-is.
        let input = indoc! {"
            from typing import NamedTuple
            class Point(NamedTuple):
                x: int
                y: int
        "};
        check(input, input);
    }

    #[test]
    fn synth_with_default_values_preserved() {
        // Synthesized class shouldn't have defaults, but if a similarly-named
        // class does, we don't roll it back.
        let input = indoc! {"
            from typing import NamedTuple
            class _AnonNamedTuple_z(NamedTuple):
                name: str = \"x\"
                age: int = 0
        "};
        check(input, input);
    }
}
