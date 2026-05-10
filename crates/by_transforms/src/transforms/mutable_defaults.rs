//! AST pass: replaces non-scalar default arguments with a `_MISSING`
//! sentinel and injects a guard at the top of each function body.
//!
//!   def f(x=[]):        →   def f(x=_MISSING):
//!       ...                     if x is _MISSING:
//!                                   x = []
//!                               ...
//!
//! Only number, bool, None, string, and ellipsis literals (and unary +/-
//! on a number) are kept as-is; everything else is re-evaluated per call.

use std::cell::Cell;

use ruff_python_ast::name::Name;
use ruff_python_ast::visitor::transformer::{Transformer, walk_stmt};
use ruff_python_ast::{
    AtomicNodeIndex, CmpOp, Expr, ExprCompare, ExprContext, ExprName, ExprNoneLiteral, ModModule,
    Stmt, StmtAssign, StmtFunctionDef, StmtIf, UnaryOp,
};
use ruff_text_size::TextRange;

use super::ast_driver::{AstPass, PassContext};

pub(crate) struct MutableDefaults {
    changed: Cell<bool>,
    ever_changed: Cell<bool>,
}

impl MutableDefaults {
    pub(crate) fn new() -> Self {
        Self {
            changed: Cell::new(false),
            ever_changed: Cell::new(false),
        }
    }
}

impl AstPass for MutableDefaults {
    fn run(&self, module: &mut ModModule, ctx: &mut PassContext) {
        for (idx, stmt) in module.body.iter_mut().enumerate() {
            self.changed.set(false);
            self.visit_stmt(stmt);
            if self.changed.get() {
                ctx.changed.push(idx);
            }
        }
        if self.ever_changed.get() {
            ctx.required_imports.push("_MISSING = object()".to_owned());
        }
    }
}

fn is_immutable_scalar(expr: &Expr) -> bool {
    match expr {
        Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::EllipsisLiteral(_) => true,
        Expr::UnaryOp(u)
            if matches!(u.op, UnaryOp::USub | UnaryOp::UAdd)
                && matches!(&*u.operand, Expr::NumberLiteral(_)) =>
        {
            true
        }
        _ => false,
    }
}

fn missing_name(ctx: ExprContext) -> Expr {
    Expr::Name(ExprName {
        node_index: AtomicNodeIndex::NONE,
        range: TextRange::default(),
        id: Name::from("_MISSING"),
        ctx,
    })
}

fn build_guard(param_name: &str, default: Expr) -> Stmt {
    let target = Expr::Name(ExprName {
        node_index: AtomicNodeIndex::NONE,
        range: TextRange::default(),
        id: Name::from(param_name),
        ctx: ExprContext::Load,
    });
    let test = Expr::Compare(ExprCompare {
        node_index: AtomicNodeIndex::NONE,
        range: TextRange::default(),
        left: Box::new(target),
        ops: Box::new([CmpOp::Is]),
        comparators: Box::new([missing_name(ExprContext::Load)]),
    });
    let assign = Stmt::Assign(StmtAssign {
        node_index: AtomicNodeIndex::NONE,
        range: TextRange::default(),
        targets: vec![Expr::Name(ExprName {
            node_index: AtomicNodeIndex::NONE,
            range: TextRange::default(),
            id: Name::from(param_name),
            ctx: ExprContext::Store,
        })],
        value: Box::new(default),
    });
    Stmt::If(StmtIf {
        node_index: AtomicNodeIndex::NONE,
        range: TextRange::default(),
        test: Box::new(test),
        body: vec![assign].into(),
        elif_else_clauses: Vec::new(),
    })
}

impl Transformer for MutableDefaults {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        if let Stmt::FunctionDef(f) = stmt {
            process_function(f, &self.changed, &self.ever_changed);
        }
        walk_stmt(self, stmt);
    }
}

fn process_function(f: &mut StmtFunctionDef, changed: &Cell<bool>, ever: &Cell<bool>) {
    let mut guards: Vec<Stmt> = Vec::new();
    let process_default = |default: &mut Box<Expr>, name: &str, guards: &mut Vec<Stmt>| {
        if is_immutable_scalar(default) {
            return false;
        }
        let original_default = std::mem::replace(
            default.as_mut(),
            Expr::NoneLiteral(ExprNoneLiteral {
                node_index: AtomicNodeIndex::NONE,
                range: TextRange::default(),
            }),
        );
        guards.push(build_guard(name, original_default));
        **default = missing_name(ExprContext::Load);
        true
    };
    let params = f.parameters.as_mut();
    let mut any = false;
    for pw in params
        .posonlyargs
        .iter_mut()
        .chain(params.args.iter_mut())
        .chain(params.kwonlyargs.iter_mut())
    {
        let name = pw.parameter.name.id.to_string();
        if let Some(d) = pw.default.as_mut() {
            if process_default(d, &name, &mut guards) {
                any = true;
            }
        }
    }
    if !any {
        return;
    }
    // insert guards after a leading docstring if present
    let docstring_count = if let Some(Stmt::Expr(e)) = f.body.first() {
        usize::from(matches!(e.value.as_ref(), Expr::StringLiteral(_)))
    } else {
        0
    };
    for (i, guard) in guards.into_iter().enumerate() {
        f.body.insert(docstring_count + i, guard);
    }
    changed.set(true);
    ever.set(true);
}

#[cfg(test)]
mod tests {
    use crate::python_passthrough::unchanged;
    use crate::transpile;
    use indoc::indoc;

    fn check(input: &str, expected: &str) {
        assert_eq!(
            transpile(input, &crate::Config::test_default()).unwrap(),
            crate::python_passthrough::lazify_expected(expected)
        );
    }

    #[test]
    fn list_default() {
        check(
            indoc! {"
                def f(x=[]):
                    pass
            "},
            indoc! {"
                _MISSING = object()
                def f(x=_MISSING):
                    if x is _MISSING:
                        x = []
                    pass
            "},
        );
    }

    #[test]
    fn dict_default() {
        check(
            indoc! {"
                def f(x={}):
                    pass
            "},
            indoc! {"
                _MISSING = object()
                def f(x=_MISSING):
                    if x is _MISSING:
                        x = {}
                    pass
            "},
        );
    }

    #[test]
    fn set_default() {
        check(
            indoc! {"
                def f(x={1, 2}):
                    pass
            "},
            indoc! {"
                _MISSING = object()
                def f(x=_MISSING):
                    if x is _MISSING:
                        x = {1, 2}
                    pass
            "},
        );
    }

    #[test]
    fn scalar_default_unchanged() {
        check(
            indoc! {"
                def f(x=0):
                    pass
            "},
            indoc! {"
                def f(x=0):
                    pass
            "},
        );
    }

    #[test]
    fn ellipsis_default_unchanged() {
        unchanged(indoc! {"
                def f(x=...):
                    pass
            "});
    }

    #[test]
    fn none_default_unchanged() {
        check(
            indoc! {"
                def f(x=None):
                    pass
            "},
            indoc! {"
                def f(x=None):
                    pass
            "},
        );
    }

    #[test]
    fn multiple_mutable_defaults() {
        check(
            indoc! {"
                def f(x=[], y={}):
                    pass
            "},
            indoc! {"
                _MISSING = object()
                def f(x=_MISSING, y=_MISSING):
                    if x is _MISSING:
                        x = []
                    if y is _MISSING:
                        y = {}
                    pass
            "},
        );
    }

    #[test]
    fn preserves_docstring() {
        check(
            indoc! {r#"
                def f(x=[]):
                    """doc"""
                    pass
            "#},
            indoc! {r#"
                _MISSING = object()
                def f(x=_MISSING):
                    """doc"""
                    if x is _MISSING:
                        x = []
                    pass
            "#},
        );
    }

    #[test]
    fn sentinel_defined_once_for_multiple_functions() {
        check(
            indoc! {"
                def f(x=[]):
                    pass
                def g(y={}):
                    pass
            "},
            indoc! {"
                _MISSING = object()
                def f(x=_MISSING):
                    if x is _MISSING:
                        x = []
                    pass
                def g(y=_MISSING):
                    if y is _MISSING:
                        y = {}
                    pass
            "},
        );
    }

    #[test]
    fn fstring_default() {
        check(
            indoc! {r#"
                data = "fdsa"
                def f(a=f"asdf{data}"):
                    print(a)
            "#},
            indoc! {r#"
                _MISSING = object()
                data = "fdsa"
                def f(a=_MISSING):
                    if a is _MISSING:
                        a = f"asdf{data}"
                    print(a)
            "#},
        );
    }

    #[test]
    fn tstring_default() {
        check(
            indoc! {r#"
                data = "fdsa"
                def f(a=t"asdf{data}"):
                    print(a)
            "#},
            indoc! {r#"
                _MISSING = object()
                data = "fdsa"
                def f(a=_MISSING):
                    if a is _MISSING:
                        a = t"asdf{data}"
                    print(a)
            "#},
        );
    }

    #[test]
    fn default_references_earlier_param() {
        check(
            indoc! {"
                def f(a, b = a + 1):
                    print(a)


                f(1)
                f(2)
            "},
            indoc! {"
                _MISSING = object()
                def f(a, b=_MISSING):
                    if b is _MISSING:
                        b = a + 1
                    print(a)


                f(1)
                f(2)
            "},
        );
    }

    #[test]
    fn multiline_signature_inline_ellipsis_body() {
        check(
            indoc! {"
                def f(
                    a: int = []
                ) -> int: ...
            "},
            indoc! {"
                _MISSING = object()
                def f(a: int=_MISSING) -> int:
                    if a is _MISSING:
                        a = []
                    ...
            "},
        );
    }

    #[test]
    fn ellipsis() {
        check(
            "def f(x=[]): ...",
            indoc! {"
                _MISSING = object()
                def f(x=_MISSING):
                    if x is _MISSING:
                        x = []
                    ...
            "},
        );
    }
}
