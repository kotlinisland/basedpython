use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{Decorator, Expr, Parameters, Stmt};
use ruff_text_size::{Ranged, TextSize};

/// Byte offset of the start of the line containing `pos`. Lines begin at
/// either offset 0 or one byte past the previous `\n`
pub(crate) fn line_start(source: &str, pos: TextSize) -> TextSize {
    let offset = usize::from(pos);
    let start = source[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    TextSize::try_from(start).expect("line start fits u32")
}

/// Leading-whitespace slice of the line containing `pos`. Empty when the line
/// has no indentation or `pos` falls inside the indentation prefix
pub(crate) fn line_indent(source: &str, pos: TextSize) -> &str {
    let line_start = usize::from(line_start(source, pos));
    let offset = usize::from(pos);
    let rest = &source[line_start..offset];
    let ws_len = rest.len() - rest.trim_start().len();
    &source[line_start..line_start + ws_len]
}

/// True when `dec` is a synthetic decorator emitted by the parser for a
/// basedpython modifier keyword (e.g. `let`, `final`, `abstract`,
/// `decorator_keyword`) rather than a user-written `@…`. Synthetic nodes
/// have no `@` byte at their range start in the source
pub(crate) fn is_synthetic_decorator(source: &str, dec: &Decorator) -> bool {
    let start = usize::from(dec.range().start());
    source.as_bytes().get(start).copied() != Some(b'@')
}

/// Invoke `on_ann` on every annotation expression reachable from `stmt`.
/// Covers `AnnAssign` targets, `TypeAlias` RHS, function parameter
/// annotations (regular, vararg, kwarg), return annotations, and recurses
/// into nested function bodies. Used by the reverse transforms (callable,
/// `not_type`, `tuple_type`, intersection) to share annotation-site discovery
pub(crate) fn for_each_annotation_in_stmt<F: FnMut(&Expr)>(stmt: &Stmt, mut on_ann: F) {
    let mut walker = AnnotationWalker {
        on_ann: &mut on_ann,
    };
    walker.visit_stmt(stmt);
}

struct AnnotationWalker<'f, F: FnMut(&Expr)> {
    on_ann: &'f mut F,
}

impl<F: FnMut(&Expr)> AnnotationWalker<'_, F> {
    fn walk_parameters(&mut self, params: &Parameters) {
        for p in params.iter_non_variadic_params() {
            if let Some(ann) = &p.parameter.annotation {
                (self.on_ann)(ann);
            }
        }
        if let Some(v) = &params.vararg
            && let Some(ann) = &v.annotation
        {
            (self.on_ann)(ann);
        }
        if let Some(k) = &params.kwarg
            && let Some(ann) = &k.annotation
        {
            (self.on_ann)(ann);
        }
    }
}

impl<'ast, F: FnMut(&Expr)> Visitor<'ast> for AnnotationWalker<'_, F> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        match stmt {
            Stmt::AnnAssign(a) => (self.on_ann)(&a.annotation),
            Stmt::TypeAlias(a) => (self.on_ann)(&a.value),
            Stmt::FunctionDef(f) => {
                self.walk_parameters(&f.parameters);
                if let Some(ret) = &f.returns {
                    (self.on_ann)(ret);
                }
                for s in &f.body {
                    self.visit_stmt(s);
                }
            }
            _ => walk_stmt(self, stmt),
        }
    }
}
