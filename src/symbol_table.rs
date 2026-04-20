//! Scope-aware symbol table for a single module.
//!
//! Classifies each local binding so other transforms can ask "is this name a
//! type?" without reinventing resolution. Single-file only — imports are
//! recorded but treated as opaque (we can't see what they resolve to until
//! project-wide resolution lands).
//!
//! Lookup is lexical: start from the innermost scope enclosing the use site
//! and walk up the parent chain. All bindings in a scope are visible from any
//! position in it, modelling Python's late-binding semantics for function and
//! class bodies.

use std::collections::HashMap;

use ruff_python_ast::{ExceptHandler, Expr, Stmt, TypeParam};
use ruff_text_size::{Ranged, TextRange, TextSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingKind {
    /// `class X:`
    Class,
    /// `type X = ...`
    TypeAlias,
    /// `X: TypeAlias = ...`
    TypeAliasAnnotated,
    /// `X = TypeVar(...)`, `X = ParamSpec(...)`, `X = TypeVarTuple(...)`
    TypeVarLike,
    /// Function parameter.
    Parameter,
    /// `def f():`
    Function,
    /// `from m import X`, `import X`, `X = TypeAliasType(...)`
    Import,
    /// Anything else — a regular value assignment.
    Value,
}

impl BindingKind {
    /// Whether a subscript `X[...]` with this binding as `X` should treat the
    /// slice as type arguments (so `1 | 2` inside becomes `Literal[1, 2]`).
    ///
    /// `Import` is treated as type-like: without cross-file resolution we'd
    /// rather over-propagate (which loses nothing for non-type imports that
    /// nobody subscripts anyway) than under-propagate and silently drop user
    /// intent like `from typing import List; a: List[1 | 2]`.
    pub fn subscript_is_type_context(self) -> bool {
        matches!(
            self,
            Self::Class | Self::TypeAlias | Self::TypeAliasAnnotated | Self::Import
        )
    }
}

pub struct Binding {
    pub kind: BindingKind,
}

pub struct Scope {
    pub parent: Option<usize>,
    pub range: TextRange,
    pub bindings: HashMap<String, Binding>,
}

pub struct SymbolTable {
    pub scopes: Vec<Scope>, // index 0 is the module scope
}

impl SymbolTable {
    pub fn build(source: &str, stmts: &[Stmt]) -> Self {
        let module_range = TextRange::new(
            TextSize::from(0),
            TextSize::from(source.len() as u32),
        );
        let mut builder = Builder {
            scopes: vec![Scope {
                parent: None,
                range: module_range,
                bindings: HashMap::new(),
            }],
            stack: vec![0],
        };
        for stmt in stmts {
            builder.visit_stmt(stmt);
        }
        SymbolTable {
            scopes: builder.scopes,
        }
    }

    /// Innermost scope whose range contains `pos`.
    pub fn scope_at(&self, pos: TextSize) -> usize {
        let mut best = 0;
        let mut best_size = self.scopes[0].range.len();
        for (i, scope) in self.scopes.iter().enumerate().skip(1) {
            if scope.range.contains(pos) && scope.range.len() <= best_size {
                best = i;
                best_size = scope.range.len();
            }
        }
        best
    }

    /// Resolve a name at a use site, walking up the scope chain.
    ///
    /// Returns `None` if the name isn't bound anywhere in the file; callers
    /// should treat that as "unknown" (likely a builtin or external import).
    pub fn resolve(&self, name: &str, use_pos: TextSize) -> Option<BindingKind> {
        let mut scope_id = self.scope_at(use_pos);
        loop {
            if let Some(b) = self.scopes[scope_id].bindings.get(name) {
                return Some(b.kind);
            }
            match self.scopes[scope_id].parent {
                Some(p) => scope_id = p,
                None => return None,
            }
        }
    }
}

struct Builder {
    scopes: Vec<Scope>,
    stack: Vec<usize>,
}

impl Builder {
    fn current(&self) -> usize {
        *self.stack.last().expect("scope stack is non-empty")
    }

    fn bind(&mut self, name: &str, kind: BindingKind) {
        let id = self.current();
        self.scopes[id]
            .bindings
            .insert(name.to_owned(), Binding { kind });
    }

    fn push(&mut self, range: TextRange) {
        let parent = self.current();
        let id = self.scopes.len();
        self.scopes.push(Scope {
            parent: Some(parent),
            range,
            bindings: HashMap::new(),
        });
        self.stack.push(id);
    }

    fn pop(&mut self) {
        self.stack.pop();
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::ClassDef(c) => {
                self.bind(c.name.id.as_str(), BindingKind::Class);
                self.push(c.range());
                if let Some(tp) = &c.type_params {
                    for p in &tp.type_params {
                        self.bind(type_param_name(p), BindingKind::TypeVarLike);
                    }
                }
                for s in &c.body {
                    self.visit_stmt(s);
                }
                self.pop();
            }
            Stmt::FunctionDef(f) => {
                self.bind(f.name.id.as_str(), BindingKind::Function);
                self.push(f.range());
                if let Some(tp) = &f.type_params {
                    for p in &tp.type_params {
                        self.bind(type_param_name(p), BindingKind::TypeVarLike);
                    }
                }
                for p in f.parameters.iter_non_variadic_params() {
                    self.bind(p.parameter.name.id.as_str(), BindingKind::Parameter);
                }
                if let Some(v) = &f.parameters.vararg {
                    self.bind(v.name.id.as_str(), BindingKind::Parameter);
                }
                if let Some(k) = &f.parameters.kwarg {
                    self.bind(k.name.id.as_str(), BindingKind::Parameter);
                }
                for s in &f.body {
                    self.visit_stmt(s);
                }
                self.pop();
            }
            Stmt::TypeAlias(a) => {
                if let Expr::Name(name) = a.name.as_ref() {
                    self.bind(name.id.as_str(), BindingKind::TypeAlias);
                }
            }
            Stmt::AnnAssign(a) => {
                if let Expr::Name(name) = a.target.as_ref() {
                    let kind = if is_typealias_annotation(&a.annotation) {
                        BindingKind::TypeAliasAnnotated
                    } else if a.value.as_deref().is_some_and(is_typevarlike_call) {
                        BindingKind::TypeVarLike
                    } else {
                        BindingKind::Value
                    };
                    self.bind(name.id.as_str(), kind);
                }
            }
            Stmt::Assign(a) => {
                let kind = if is_typevarlike_call(&a.value) {
                    BindingKind::TypeVarLike
                } else if is_typealiastype_call(&a.value) {
                    BindingKind::TypeAlias
                } else {
                    BindingKind::Value
                };
                for target in &a.targets {
                    if let Expr::Name(name) = target {
                        self.bind(name.id.as_str(), kind);
                    }
                }
            }
            Stmt::Import(imp) => {
                for alias in &imp.names {
                    let name = match &alias.asname {
                        Some(a) => a.id.as_str().to_owned(),
                        None => alias
                            .name
                            .id
                            .as_str()
                            .split('.')
                            .next()
                            .unwrap()
                            .to_owned(),
                    };
                    self.bind(&name, BindingKind::Import);
                }
            }
            Stmt::ImportFrom(imp) => {
                for alias in &imp.names {
                    let name = match &alias.asname {
                        Some(a) => a.id.as_str(),
                        None => alias.name.id.as_str(),
                    };
                    self.bind(name, BindingKind::Import);
                }
            }
            Stmt::If(s) => {
                for x in &s.body {
                    self.visit_stmt(x);
                }
                for c in &s.elif_else_clauses {
                    for x in &c.body {
                        self.visit_stmt(x);
                    }
                }
            }
            Stmt::While(s) => {
                for x in &s.body {
                    self.visit_stmt(x);
                }
                for x in &s.orelse {
                    self.visit_stmt(x);
                }
            }
            Stmt::For(s) => {
                if let Expr::Name(name) = s.target.as_ref() {
                    self.bind(name.id.as_str(), BindingKind::Value);
                }
                for x in &s.body {
                    self.visit_stmt(x);
                }
                for x in &s.orelse {
                    self.visit_stmt(x);
                }
            }
            Stmt::With(s) => {
                for item in &s.items {
                    if let Some(vars) = &item.optional_vars {
                        if let Expr::Name(name) = vars.as_ref() {
                            self.bind(name.id.as_str(), BindingKind::Value);
                        }
                    }
                }
                for x in &s.body {
                    self.visit_stmt(x);
                }
            }
            Stmt::Try(s) => {
                for x in &s.body {
                    self.visit_stmt(x);
                }
                for h in &s.handlers {
                    let ExceptHandler::ExceptHandler(h) = h;
                    if let Some(n) = &h.name {
                        self.bind(n.id.as_str(), BindingKind::Value);
                    }
                    for x in &h.body {
                        self.visit_stmt(x);
                    }
                }
                for x in &s.orelse {
                    self.visit_stmt(x);
                }
                for x in &s.finalbody {
                    self.visit_stmt(x);
                }
            }
            _ => {}
        }
    }
}

fn type_param_name(p: &TypeParam) -> &str {
    match p {
        TypeParam::TypeVar(tv) => tv.name.id.as_str(),
        TypeParam::TypeVarTuple(tvt) => tvt.name.id.as_str(),
        TypeParam::ParamSpec(ps) => ps.name.id.as_str(),
    }
}

fn is_typealias_annotation(expr: &Expr) -> bool {
    match expr {
        Expr::Name(n) => n.id.as_str() == "TypeAlias",
        Expr::Attribute(a) => a.attr.id.as_str() == "TypeAlias",
        _ => false,
    }
}

fn call_callee_name(expr: &Expr) -> Option<&str> {
    let Expr::Call(c) = expr else {
        return None;
    };
    match c.func.as_ref() {
        Expr::Name(n) => Some(n.id.as_str()),
        Expr::Attribute(a) => Some(a.attr.id.as_str()),
        _ => None,
    }
}

fn is_typevarlike_call(expr: &Expr) -> bool {
    matches!(
        call_callee_name(expr),
        Some("TypeVar" | "TypeVarTuple" | "ParamSpec")
    )
}

fn is_typealiastype_call(expr: &Expr) -> bool {
    matches!(call_callee_name(expr), Some("TypeAliasType" | "NewType"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruff_python_parser::parse_module;

    fn build(source: &str) -> SymbolTable {
        let parsed = parse_module(source).unwrap();
        SymbolTable::build(source, parsed.suite())
    }

    #[test]
    fn module_class() {
        let src = "class A: ...\n";
        let t = build(src);
        assert_eq!(
            t.resolve("A", TextSize::from(0)),
            Some(BindingKind::Class)
        );
    }

    #[test]
    fn module_type_alias() {
        let src = "type X = int\n";
        let t = build(src);
        assert_eq!(
            t.resolve("X", TextSize::from(0)),
            Some(BindingKind::TypeAlias)
        );
    }

    #[test]
    fn module_value() {
        let src = "x = 5\n";
        let t = build(src);
        assert_eq!(
            t.resolve("x", TextSize::from(0)),
            Some(BindingKind::Value)
        );
    }

    #[test]
    fn module_typevar() {
        let src = "T = TypeVar(\"T\")\n";
        let t = build(src);
        assert_eq!(
            t.resolve("T", TextSize::from(0)),
            Some(BindingKind::TypeVarLike)
        );
    }

    #[test]
    fn ann_assign_with_typealias() {
        let src = "X: TypeAlias = int\n";
        let t = build(src);
        assert_eq!(
            t.resolve("X", TextSize::from(0)),
            Some(BindingKind::TypeAliasAnnotated)
        );
    }

    #[test]
    fn module_import_from() {
        let src = "from typing import Literal\n";
        let t = build(src);
        assert_eq!(
            t.resolve("Literal", TextSize::from(0)),
            Some(BindingKind::Import)
        );
    }

    #[test]
    fn module_import_from_with_alias() {
        let src = "from typing import Literal as L\n";
        let t = build(src);
        assert_eq!(
            t.resolve("L", TextSize::from(0)),
            Some(BindingKind::Import)
        );
        assert_eq!(t.resolve("Literal", TextSize::from(0)), None);
    }

    #[test]
    fn module_import_dotted() {
        let src = "import os.path\n";
        let t = build(src);
        assert_eq!(
            t.resolve("os", TextSize::from(0)),
            Some(BindingKind::Import)
        );
    }

    #[test]
    fn function_scope_parameters() {
        let src = "def f(x, y):\n    pass\n";
        let t = build(src);
        // Use position inside the function body
        let body_pos = TextSize::from((src.find("pass").unwrap()) as u32);
        assert_eq!(t.resolve("x", body_pos), Some(BindingKind::Parameter));
        assert_eq!(t.resolve("y", body_pos), Some(BindingKind::Parameter));
        // f itself is visible
        assert_eq!(t.resolve("f", body_pos), Some(BindingKind::Function));
    }

    #[test]
    fn parameter_shadows_outer() {
        let src = "type X = int\ndef f(X):\n    pass\n";
        let t = build(src);
        let body_pos = TextSize::from((src.find("pass").unwrap()) as u32);
        // Inside f, X resolves to the parameter
        assert_eq!(t.resolve("X", body_pos), Some(BindingKind::Parameter));
        // At module scope, X is the type alias
        assert_eq!(
            t.resolve("X", TextSize::from(0)),
            Some(BindingKind::TypeAlias)
        );
    }

    #[test]
    fn class_type_params() {
        let src = "class Foo[T]:\n    pass\n";
        let t = build(src);
        let body_pos = TextSize::from((src.find("pass").unwrap()) as u32);
        assert_eq!(t.resolve("T", body_pos), Some(BindingKind::TypeVarLike));
    }

    #[test]
    fn forward_reference_within_function() {
        // Python late-binds names inside functions; uses can refer to
        // declarations that appear later at the module level.
        let src = "def f():\n    return X\ntype X = int\n";
        let t = build(src);
        let use_pos = TextSize::from((src.find("return X").unwrap() + 7) as u32);
        assert_eq!(t.resolve("X", use_pos), Some(BindingKind::TypeAlias));
    }

    #[test]
    fn unresolved_name() {
        let src = "a = 1\n";
        let t = build(src);
        assert_eq!(t.resolve("list", TextSize::from(0)), None);
    }

    #[test]
    fn subscript_is_type_context_classification() {
        assert!(BindingKind::Class.subscript_is_type_context());
        assert!(BindingKind::TypeAlias.subscript_is_type_context());
        assert!(BindingKind::Import.subscript_is_type_context());
        assert!(!BindingKind::Value.subscript_is_type_context());
        assert!(!BindingKind::Parameter.subscript_is_type_context());
        assert!(!BindingKind::Function.subscript_is_type_context());
        assert!(!BindingKind::TypeVarLike.subscript_is_type_context());
    }
}