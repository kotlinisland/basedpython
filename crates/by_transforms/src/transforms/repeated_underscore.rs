//! AST rewrite that renames repeated `_` parameters in function and lambda
//! definitions.
//!
//! basedpython allows `def f(_, _, _): ...` as a shorthand for ignoring
//! multiple positional parameters. python rejects this with a duplicate-
//! parameter error, so each `_` after the first is renamed to a fresh
//! `_<n>` (`_2`, `_3`, ...). references to `_` inside the body are left
//! alone and resolve to the first parameter

use std::cell::Cell;
use std::collections::HashSet;

use ruff_python_ast::name::Name;
use ruff_python_ast::visitor::transformer::{Transformer, walk_expr, walk_stmt};
use ruff_python_ast::{Expr, Parameter, Parameters, Stmt};

pub(crate) struct RepeatedUnderscore {
    changed: Cell<bool>,
}

impl RepeatedUnderscore {
    pub(crate) fn new() -> Self {
        Self {
            changed: Cell::new(false),
        }
    }

    pub(crate) fn changed_cell(&self) -> &Cell<bool> {
        &self.changed
    }

    fn rename(&self, params: &mut Parameters) {
        let mut seen_underscore = false;
        let mut taken: HashSet<String> = HashSet::new();
        for p in params.iter() {
            taken.insert(p.name().to_string());
        }
        let mut next = 2u32;
        let next_name = |next: &mut u32, taken: &mut HashSet<String>| -> String {
            loop {
                let candidate = format!("_{}", *next);
                *next += 1;
                if !taken.contains(&candidate) {
                    taken.insert(candidate.clone());
                    return candidate;
                }
            }
        };
        let process = |p: &mut Parameter,
                       seen: &mut bool,
                       next: &mut u32,
                       taken: &mut HashSet<String>,
                       changed: &Cell<bool>| {
            if p.name.as_str() != "_" {
                return;
            }
            if !*seen {
                *seen = true;
                return;
            }
            let new_name = next_name(next, taken);
            p.name.id = Name::from(new_name.as_str());
            changed.set(true);
        };
        for pw in params
            .posonlyargs
            .iter_mut()
            .chain(params.args.iter_mut())
            .chain(params.kwonlyargs.iter_mut())
        {
            process(
                &mut pw.parameter,
                &mut seen_underscore,
                &mut next,
                &mut taken,
                &self.changed,
            );
        }
        if let Some(v) = params.vararg.as_deref_mut() {
            process(
                v,
                &mut seen_underscore,
                &mut next,
                &mut taken,
                &self.changed,
            );
        }
        if let Some(k) = params.kwarg.as_deref_mut() {
            process(
                k,
                &mut seen_underscore,
                &mut next,
                &mut taken,
                &self.changed,
            );
        }
    }
}

impl Transformer for RepeatedUnderscore {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        if let Stmt::FunctionDef(f) = stmt {
            self.rename(&mut f.parameters);
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&self, expr: &mut Expr) {
        if let Expr::Lambda(l) = expr
            && let Some(params) = l.parameters.as_deref_mut()
        {
            self.rename(params);
        }
        walk_expr(self, expr);
    }
}

#[cfg(test)]
mod tests {
    use crate::python_passthrough::unchanged;
    use crate::{Config, transpile};

    fn check(input: &str, expected: &str) {
        assert_eq!(transpile(input, &Config::test_default()).unwrap(), expected);
    }

    #[test]
    fn two_underscores() {
        check(
            "def f(_, _):\n    print(_)\n",
            "def f(_, _2):\n    print(_)\n",
        );
    }

    #[test]
    fn three_underscores() {
        check(
            "def g(_, _, _, x):\n    return _\n",
            "def g(_, _2, _3, x):\n    return _\n",
        );
    }

    #[test]
    fn interleaved() {
        check(
            "def h(a, _, b, _):\n    return a + b\n",
            "def h(a, _, b, _2):\n    return a + b\n",
        );
    }

    #[test]
    fn lambda_underscores() {
        check("f = lambda _, _: 1\n", "f = lambda _, _2: 1\n");
    }

    #[test]
    fn nested_function() {
        // codegen emits a blank line before nested function defs
        check(
            "def outer(_, _):\n    def inner(_, _):\n        return _\n    return _\n",
            "def outer(_, _2):\n\n    def inner(_, _2):\n        return _\n    return _\n",
        );
    }

    #[test]
    fn existing_collision() {
        check(
            "def f(_, _2, _):\n    return _\n",
            "def f(_, _2, _3):\n    return _\n",
        );
    }

    #[test]
    fn single_underscore_unchanged() {
        unchanged("def f(_):\n    return _\n");
    }

    #[test]
    fn no_underscore_unchanged() {
        unchanged("def f(a, b):\n    return a + b\n");
    }

    #[test]
    fn vararg_underscore() {
        check(
            "def f(_, *_):\n    return _\n",
            "def f(_, *_2):\n    return _\n",
        );
    }
}
