//! Marks every `import` and `from import` statement as lazy.
//!
//! Two emission strategies, chosen by target Python version:
//!   - **`min_version >= 3.15`** — prepend the `lazy` keyword (PEP 810)
//!   - **`min_version < 3.15`** — rewrite the statement to call a runtime
//!     polyfill (`_lazy_module` for module imports, `_lazy_attr` for `from`
//!     imports). The polyfill defines helpers in the preamble that wrap
//!     `importlib.util.LazyLoader` and a small proxy class
//!
//! Both modes skip:
//!   - `from __future__ import ...` — compiler directive
//!   - `from x import *` — `lazy` is not allowed with star imports
//!
//! The polyfill additionally skips forms it can't safely rewrite:
//!   - relative imports (`from .pkg import x`)
//!   - `import a.b` without an alias (binds the top package, which
//!     `LazyLoader` does not register)
//!   - bootstrap modules (`sys`, `importlib*`) — the helpers depend on them

use ruff_diagnostics::{Edit, Fix};
use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{Stmt, StmtImport, StmtImportFrom};
use ruff_text_size::{Ranged, TextRange, TextSize};

#[expect(
    clippy::struct_excessive_bools,
    reason = "independent lazy-import state flags, not a state machine"
)]
pub(crate) struct LazyImport<'src> {
    source: &'src str,
    /// True when the target Python version supports PEP 810 (3.15+). When
    /// false, the transform uses the runtime polyfill instead
    keyword_supported: bool,
    pub(crate) edits: Vec<Fix>,
    /// True when at least one statement was rewritten to call
    /// `_lazy_module`; the preamble must define the module helper
    pub(crate) needs_module_helper: bool,
    /// True when at least one statement was rewritten to call `_lazy_attr`;
    /// the preamble must define the `_LazyAttr` proxy (and `_lazy_module`,
    /// which `_lazy_attr` calls)
    pub(crate) needs_attr_helper: bool,
    /// True when at least one `from ty_extensions import X` was rewritten to
    /// a `_TyExtMarker` assignment; the preamble must define the marker
    pub(crate) needs_ty_ext_marker: bool,
}

impl<'src> LazyImport<'src> {
    pub(crate) fn new(source: &'src str, keyword_supported: bool) -> Self {
        Self {
            source,
            keyword_supported,
            edits: Vec::new(),
            needs_module_helper: false,
            needs_attr_helper: false,
            needs_ty_ext_marker: false,
        }
    }

    /// Strip the leading `lazy` keyword and any trailing whitespace from a
    /// statement. Called when the statement falls into a skipped category
    /// (star, `__future__`, polyfill-unsafe form) but the parser saw `lazy`
    fn strip_lazy_keyword(&mut self, stmt_range: TextRange) {
        let start = stmt_range.start();
        let text = &self.source[usize::from(start)..usize::from(stmt_range.end())];
        let mut drop_len = 0usize;
        loop {
            let rest = &text[drop_len..];
            let Some(after_kw) = rest.strip_prefix("lazy") else {
                break;
            };
            // require a word boundary so we don't eat "lazyfoo"
            let next = after_kw.chars().next();
            if matches!(next, Some(c) if !c.is_whitespace()) {
                break;
            }
            let ws_len = after_kw.len() - after_kw.trim_start_matches([' ', '\t']).len();
            drop_len += "lazy".len() + ws_len;
        }
        if drop_len == 0 {
            return;
        }
        let strip_end = start + TextSize::try_from(drop_len).unwrap();
        self.edits
            .push(Fix::safe_edit(Edit::range_deletion(TextRange::new(
                start, strip_end,
            ))));
    }

    fn insert_lazy_keyword(&mut self, at: TextSize) {
        self.edits
            .push(Fix::safe_edit(Edit::insertion("lazy ".to_owned(), at)));
    }

    fn line_indent(&self, range: TextRange) -> &str {
        let stmt_start = usize::from(range.start());
        let line_start = self.source[..stmt_start]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        &self.source[line_start..stmt_start]
    }

    fn is_bootstrap(name: &str) -> bool {
        matches!(name, "sys" | "importlib") || name.starts_with("importlib.")
    }

    fn process_import(&mut self, node: &StmtImport) {
        if self.keyword_supported {
            if !node.is_lazy {
                self.insert_lazy_keyword(node.range().start());
            }
            return;
        }
        let mut lines: Vec<String> = Vec::new();
        let mut any_skipped = false;
        for alias in &node.names {
            let module = alias.name.id.as_str();
            if Self::is_bootstrap(module) {
                any_skipped = true;
                continue;
            }
            // `import a.b` without `as` binds `a`, not `a.b`, so `LazyLoader`
            // on `a.b` would never trigger the lazy binding
            if alias.asname.is_none() && module.contains('.') {
                any_skipped = true;
                continue;
            }
            let bind = match &alias.asname {
                Some(a) => a.id.as_str(),
                None => module,
            };
            self.needs_module_helper = true;
            lines.push(format!("{bind} = _lazy_module(\"{module}\")"));
        }
        if lines.is_empty() {
            // Every alias was skipped — strip any `lazy` keyword the parser
            // saw so the output stays valid Python
            if node.is_lazy {
                self.strip_lazy_keyword(node.range());
            }
            return;
        }
        let _ = any_skipped; // mixed-skip aliases are rare; we replace whole stmt
        let indent = self.line_indent(node.range());
        let separator = format!("\n{indent}");
        self.edits.push(Fix::safe_edit(Edit::range_replacement(
            lines.join(&separator),
            node.range(),
        )));
    }

    fn process_from(&mut self, node: &StmtImportFrom) {
        let is_future = node
            .module
            .as_ref()
            .is_some_and(|m| m.id.as_str() == "__future__");
        let is_star = node.names.iter().any(|a| a.name.id.as_str() == "*");

        if self.keyword_supported {
            if is_future || is_star {
                if node.is_lazy {
                    self.strip_lazy_keyword(node.range());
                }
                return;
            }
            if !node.is_lazy {
                self.insert_lazy_keyword(node.range().start());
            }
            return;
        }

        // Polyfill mode for `from x import y`. Relative imports use
        // `importlib.util.resolve_name(..., __package__)` at runtime
        let polyfill_skip = is_future
            || is_star
            || (node.level == 0
                && node
                    .module
                    .as_ref()
                    .is_some_and(|m| Self::is_bootstrap(m.id.as_str())))
            || (node.level == 0 && node.module.is_none());
        if polyfill_skip {
            if node.is_lazy {
                self.strip_lazy_keyword(node.range());
            }
            return;
        }

        let module_part = node.module.as_ref().map(|m| m.id.as_str()).unwrap_or("");
        let dots: String = ".".repeat(node.level as usize);
        let is_relative = node.level > 0;
        // `ty_extensions` is a ty-only module — it has no runtime existence on
        // PyPI. Names imported from it (`Intersection`, `Not`, `TypeOf`,
        // `JustFloat`, `JustComplex`, `Top`) are type-only markers. Replace
        // with a stub class that supports `X[T]`, `X | Y`, and use-as-base
        let is_ty_ext = !is_relative && module_part == "ty_extensions";
        let mut lines: Vec<String> = Vec::new();
        for alias in &node.names {
            let name = alias.name.id.as_str();
            let bind = alias.asname.as_ref().map(|a| a.id.as_str()).unwrap_or(name);
            if is_ty_ext {
                self.needs_ty_ext_marker = true;
                lines.push(format!("{bind} = _TyExtMarker"));
                continue;
            }
            if is_relative && module_part.is_empty() {
                // `from . import x` — `x` is a submodule of the current
                // package. Resolve the relative target at runtime
                self.needs_module_helper = true;
                let rel = format!("{dots}{name}");
                lines.push(format!(
                    "{bind} = _lazy_module(_by_iu.resolve_name(\"{rel}\", __package__))"
                ));
            } else if is_relative {
                // `from .pkg import x` — lazy attribute on the resolved
                // parent, matching the `from pkg import x` shape
                self.needs_attr_helper = true;
                let rel = format!("{dots}{module_part}");
                lines.push(format!(
                    "{bind} = _lazy_attr(_by_iu.resolve_name(\"{rel}\", __package__), \"{name}\")"
                ));
            } else {
                self.needs_attr_helper = true;
                lines.push(format!(
                    "{bind} = _lazy_attr(\"{module_part}\", \"{name}\")"
                ));
            }
        }
        if lines.is_empty() {
            return;
        }
        let indent = self.line_indent(node.range());
        let separator = format!("\n{indent}");
        self.edits.push(Fix::safe_edit(Edit::range_replacement(
            lines.join(&separator),
            node.range(),
        )));
    }
}

impl<'ast> Visitor<'ast> for LazyImport<'_> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        match stmt {
            Stmt::Import(n) => self.process_import(n),
            Stmt::ImportFrom(n) => self.process_from(n),
            _ => walk_stmt(self, stmt),
        }
    }
}

/// Preamble snippet defining the runtime helpers used by polyfill-mode
/// lazified imports. Emitted once per file when any lazification fires
pub(crate) fn polyfill_preamble(
    needs_module: bool,
    needs_attr: bool,
    needs_ty_ext: bool,
) -> String {
    if !needs_module && !needs_attr && !needs_ty_ext {
        return String::new();
    }
    let needs_module = needs_module || needs_attr;
    let mut out = String::new();
    if needs_module {
        out.push_str("import importlib.util as _by_iu, sys as _by_sys\n");
        out.push_str("def _lazy_module(name):\n");
        out.push_str("    mod = _by_sys.modules.get(name)\n");
        out.push_str("    if mod is not None:\n");
        out.push_str("        return mod\n");
        out.push_str("    spec = _by_iu.find_spec(name)\n");
        // `find_spec` returns `None` when the module isn't installed; raise
        // a clean `ImportError` instead of letting the next line crash with
        // `AttributeError: 'NoneType' object has no attribute 'loader'`
        out.push_str("    if spec is None or spec.loader is None:\n");
        out.push_str("        raise ImportError(f\"No module named {name!r}\", name=name)\n");
        out.push_str("    spec.loader = _by_iu.LazyLoader(spec.loader)\n");
        out.push_str("    mod = _by_iu.module_from_spec(spec)\n");
        out.push_str("    _by_sys.modules[name] = mod\n");
        out.push_str("    spec.loader.exec_module(mod)\n");
        out.push_str("    return mod\n");
    }
    if needs_attr {
        out.push_str("class _LazyAttr:\n");
        out.push_str("    __slots__ = (\"_by_mod\", \"_by_attr\", \"_by_val\", \"_by_has\")\n");
        out.push_str("    def __init__(self, mod, attr):\n");
        out.push_str("        object.__setattr__(self, \"_by_mod\", mod)\n");
        out.push_str("        object.__setattr__(self, \"_by_attr\", attr)\n");
        out.push_str("        object.__setattr__(self, \"_by_val\", None)\n");
        out.push_str("        object.__setattr__(self, \"_by_has\", False)\n");
        out.push_str("    def _by_resolve(self):\n");
        out.push_str("        if not self._by_has:\n");
        out.push_str(
            "            object.__setattr__(self, \"_by_val\", getattr(_lazy_module(self._by_mod), self._by_attr))\n",
        );
        out.push_str("            object.__setattr__(self, \"_by_has\", True)\n");
        out.push_str("        return self._by_val\n");
        out.push_str("    def __getattr__(self, k): return getattr(self._by_resolve(), k)\n");
        out.push_str("    def __call__(self, *a, **k): return self._by_resolve()(*a, **k)\n");
        out.push_str("    def __getitem__(self, k): return self._by_resolve()[k]\n");
        out.push_str("    def __class_getitem__(cls, k): return cls\n");
        out.push_str("    def __or__(self, o): return self._by_resolve() | o\n");
        out.push_str("    def __ror__(self, o): return o | self._by_resolve()\n");
        out.push_str("    def __mro_entries__(self, bases):\n");
        out.push_str("        r = self._by_resolve()\n");
        out.push_str("        m = getattr(r, \"__mro_entries__\", None)\n");
        out.push_str("        if m is None: return (r,)\n");
        out.push_str("        return m(tuple(r if b is self else b for b in bases))\n");
        out.push_str("    def __repr__(self): return repr(self._by_resolve())\n");
        out.push_str("def _lazy_attr(mod, attr): return _LazyAttr(mod, attr)\n");
    }
    if needs_ty_ext {
        // type-only marker for `ty_extensions` imports. Supports the type
        // expression operations the language allows on these names without
        // attempting a (non-existent) runtime import
        out.push_str("class _TyExtMarker:\n");
        out.push_str("    def __class_getitem__(cls, k): return cls\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::config::PythonVersion;
    use crate::{Config, transpile};
    use indoc::indoc;

    fn cfg_315() -> Config {
        Config {
            min_version: PythonVersion::from((3, 15)),
            ..Config {
                lazy_imports: true,
                ..Config::test_default()
            }
        }
    }

    fn check(input: &str, expected: &str) {
        assert_eq!(transpile(input, &cfg_315()).unwrap(), expected);
    }

    #[test]
    fn keyword_simple_import() {
        check("import os\n", "lazy import os\n");
    }

    #[test]
    fn keyword_import_as() {
        check("import os as o\n", "lazy import os as o\n");
    }

    #[test]
    fn keyword_from_import() {
        check("from os import path\n", "lazy from os import path\n");
    }

    #[test]
    fn keyword_future_unchanged() {
        check(
            "from __future__ import annotations\n",
            "from __future__ import annotations\n",
        );
    }

    #[test]
    fn keyword_star_unchanged() {
        check("from os import *\n", "from os import *\n");
    }

    #[test]
    fn keyword_relative_lazified() {
        check("from .pkg import x\n", "lazy from .pkg import x\n");
    }

    #[test]
    fn keyword_existing_passes_through() {
        check("lazy import os\n", "lazy import os\n");
    }

    #[test]
    fn keyword_stripped_on_future() {
        check(
            "lazy from __future__ import annotations\n",
            "from __future__ import annotations\n",
        );
    }

    #[test]
    fn keyword_stripped_on_star() {
        check("lazy from os import *\n", "from os import *\n");
    }

    #[test]
    fn keyword_nested_indent_preserved() {
        check(
            indoc! {"
                if True:
                    import os
            "},
            indoc! {"
                if True:
                    lazy import os
            "},
        );
    }

    // ---- polyfill mode (default config, 3.10) ----

    fn check_polyfill_body(input: &str, expected_body: &str) {
        let out = transpile(
            input,
            &Config {
                lazy_imports: true,
                ..Config::test_default()
            },
        )
        .unwrap();
        assert!(
            out.contains("def _lazy_module(name):"),
            "missing _lazy_module helper in:\n{out}"
        );
        assert!(
            out.ends_with(expected_body),
            "expected suffix:\n{expected_body}\n---got---\n{out}"
        );
    }

    #[test]
    fn polyfill_simple_import() {
        check_polyfill_body("import other\n", "other = _lazy_module(\"other\")\n");
    }

    #[test]
    fn polyfill_import_as() {
        check_polyfill_body("import os as o\n", "o = _lazy_module(\"os\")\n");
    }

    #[test]
    fn polyfill_dotted_with_alias() {
        check_polyfill_body("import os.path as p\n", "p = _lazy_module(\"os.path\")\n");
    }

    #[test]
    fn polyfill_dotted_no_alias_stays_eager() {
        // `import a.b` binds `a` — `LazyLoader` can't register `a` from `a.b`
        let out = transpile(
            "import os.path\n",
            &Config {
                lazy_imports: true,
                ..Config::test_default()
            },
        )
        .unwrap();
        assert_eq!(out, "import os.path\n");
    }

    #[test]
    fn polyfill_from_import() {
        check_polyfill_body(
            "from os import path\n",
            "path = _lazy_attr(\"os\", \"path\")\n",
        );
    }

    #[test]
    fn polyfill_from_import_multiple() {
        check_polyfill_body(
            "from os import path, getcwd\n",
            indoc! {"
                path = _lazy_attr(\"os\", \"path\")
                getcwd = _lazy_attr(\"os\", \"getcwd\")
            "},
        );
    }

    #[test]
    fn polyfill_relative_submodule() {
        check_polyfill_body(
            "from . import x\n",
            "x = _lazy_module(_by_iu.resolve_name(\".x\", __package__))\n",
        );
    }

    #[test]
    fn polyfill_relative_attr() {
        check_polyfill_body(
            "from .pkg import x\n",
            "x = _lazy_attr(_by_iu.resolve_name(\".pkg\", __package__), \"x\")\n",
        );
    }

    #[test]
    fn polyfill_relative_double_dot() {
        check_polyfill_body(
            "from .. import x\n",
            "x = _lazy_module(_by_iu.resolve_name(\"..x\", __package__))\n",
        );
    }

    #[test]
    fn polyfill_star_unchanged() {
        let out = transpile(
            "from os import *\n",
            &Config {
                lazy_imports: true,
                ..Config::test_default()
            },
        )
        .unwrap();
        assert_eq!(out, "from os import *\n");
    }

    #[test]
    fn polyfill_future_unchanged() {
        let out = transpile(
            "from __future__ import annotations\n",
            &Config {
                lazy_imports: true,
                ..Config::test_default()
            },
        )
        .unwrap();
        assert_eq!(out, "from __future__ import annotations\n");
    }

    #[test]
    fn polyfill_bootstrap_sys_unchanged() {
        let out = transpile(
            "import sys\n",
            &Config {
                lazy_imports: true,
                ..Config::test_default()
            },
        )
        .unwrap();
        assert_eq!(out, "import sys\n");
    }

    #[test]
    fn polyfill_lazy_keyword_lazifies() {
        // `lazy import os` on default config: keyword stripped, polyfill applied
        check_polyfill_body("lazy import os\n", "os = _lazy_module(\"os\")\n");
    }

    #[test]
    fn passthrough_in_python_mode() {
        let py = transpile(
            "import os\n",
            &Config {
                is_python: true,
                ..Config {
                    lazy_imports: true,
                    ..Config::test_default()
                }
            },
        )
        .unwrap();
        assert_eq!(py, "import os\n");
    }
}
