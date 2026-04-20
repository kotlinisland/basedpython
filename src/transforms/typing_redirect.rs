use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{Stmt, StmtImportFrom};
use ruff_text_size::Ranged;

use crate::config::{Config, PythonVersion};

/// Rewrites `from typing import X` (and `from warnings import deprecated`) to
/// `from typing_extensions import X` for names not yet in the stdlib at the
/// configured minimum Python version.
pub struct TypingRedirect {
    config: Config,
    pub edits: Vec<(ruff_text_size::TextRange, String)>,
}

impl TypingRedirect {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            edits: Vec::new(),
        }
    }

    /// Returns the Python version when `name` was added to `typing`.
    /// Returns `None` if it has been in `typing` since before 3.10.
    fn typing_added_in(name: &str) -> Option<PythonVersion> {
        use PythonVersion::*;
        match name {
            // 3.11
            "Never"
            | "assert_never"
            | "LiteralString"
            | "Required"
            | "NotRequired"
            | "Self"
            | "Unpack"
            | "TypeVarTuple"
            | "dataclass_transform"
            | "reveal_type"
            | "assert_type"
            | "get_overloads"
            | "clear_overloads" => Some(V311),
            // 3.12
            "override" | "TypeAliasType" => Some(V312),
            // 3.13
            "TypeIs" | "ReadOnly" | "get_protocol_members" | "is_protocol" | "NoDefault" => {
                Some(V313)
            }
            _ => None,
        }
    }

    /// Returns the Python version when `name` was added to `warnings`.
    fn warnings_added_in(name: &str) -> Option<PythonVersion> {
        match name {
            "deprecated" => Some(PythonVersion::V313),
            _ => None,
        }
    }

    fn process_import(&mut self, node: &StmtImportFrom) {
        // Skip relative imports and star imports.
        if node.level > 0 {
            return;
        }

        let Some(module) = &node.module else {
            return;
        };
        let module_str = module.id.as_str();

        let added_in: fn(&str) -> Option<PythonVersion> = match module_str {
            "typing" => Self::typing_added_in,
            "warnings" => Self::warnings_added_in,
            _ => return,
        };

        let mut keep: Vec<String> = Vec::new();
        let mut redirect: Vec<String> = Vec::new();

        for alias in &node.names {
            let name = alias.name.id.as_str();
            let formatted = match &alias.asname {
                Some(asname) => format!("{name} as {}", asname.id.as_str()),
                None => name.to_owned(),
            };

            if let Some(added) = added_in(name) {
                if added > self.config.min_version {
                    redirect.push(formatted);
                } else {
                    keep.push(formatted);
                }
            } else {
                keep.push(formatted);
            }
        }

        if redirect.is_empty() {
            return;
        }

        let mut parts: Vec<String> = Vec::new();
        if !keep.is_empty() {
            parts.push(format!("from {module_str} import {}", keep.join(", ")));
        }
        parts.push(format!(
            "from typing_extensions import {}",
            redirect.join(", ")
        ));

        self.edits.push((node.range(), parts.join("\n")));
    }
}

impl<'ast> Visitor<'ast> for TypingRedirect {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        if let Stmt::ImportFrom(node) = stmt {
            self.process_import(node);
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
    fn redirects_self_on_310() {
        check(
            "from typing import Self\n",
            "from typing_extensions import Self\n",
        );
    }

    #[test]
    fn keeps_typevar_redirects_self() {
        check(
            "from typing import TypeVar, Self\n",
            indoc! {"
                from typing import TypeVar
                from typing_extensions import Self
            "},
        );
    }

    #[test]
    fn no_redirect_when_already_available() {
        check(
            "from typing import TypeVar, Optional\n",
            "from typing import TypeVar, Optional\n",
        );
    }

    #[test]
    fn redirects_warnings_deprecated() {
        check(
            "from warnings import deprecated\n",
            "from typing_extensions import deprecated\n",
        );
    }

    #[test]
    fn redirects_311_names() {
        check(
            "from typing import Never, LiteralString, Unpack\n",
            "from typing_extensions import Never, LiteralString, Unpack\n",
        );
    }

    #[test]
    fn redirects_312_override() {
        check(
            "from typing import override\n",
            "from typing_extensions import override\n",
        );
    }

    #[test]
    fn no_redirect_on_matching_version() {
        let config = Config {
            min_version: crate::config::PythonVersion::V311,
        };
        assert_eq!(
            transpile("from typing import Self\n", &config).unwrap(),
            "from typing import Self\n",
        );
    }
}
