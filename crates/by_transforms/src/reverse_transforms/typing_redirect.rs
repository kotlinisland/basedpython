//! reverse of `crate::transforms::typing_redirect`:
//!   `from typing_extensions import X` → `from typing import X`
//!   for names that belong in stdlib `typing`
//!
//! only reverses names in the known `typing_added_in` table.
//! names not in the table remain in `typing_extensions`

use ruff_diagnostics::{Edit, Fix};
use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{Stmt, StmtImportFrom};
use ruff_text_size::Ranged;

pub(crate) struct TypingRedirectReverse {
    pub(crate) edits: Vec<Fix>,
}

impl TypingRedirectReverse {
    pub(crate) fn new() -> Self {
        Self { edits: Vec::new() }
    }

    fn is_typing_name(name: &str) -> bool {
        matches!(
            name,
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
                | "clear_overloads"
                | "override"
                | "TypeAliasType"
                | "TypeIs"
                | "ReadOnly"
                | "get_protocol_members"
                | "is_protocol"
                | "NoDefault"
        )
    }

    fn process_import(&mut self, node: &StmtImportFrom) {
        let Some(module) = &node.module else { return };
        if module.id.as_str() != "typing_extensions" {
            return;
        }

        let mut keep_ext: Vec<String> = Vec::new();
        let mut move_typing: Vec<String> = Vec::new();

        for alias in &node.names {
            let name = alias.name.id.as_str();
            let formatted = match &alias.asname {
                Some(asname) => format!("{name} as {}", asname.id.as_str()),
                None => name.to_owned(),
            };
            if Self::is_typing_name(name) {
                move_typing.push(formatted);
            } else {
                keep_ext.push(formatted);
            }
        }

        if move_typing.is_empty() {
            return;
        }

        let mut parts: Vec<String> = Vec::new();
        parts.push(format!("from typing import {}", move_typing.join(", ")));
        if !keep_ext.is_empty() {
            parts.push(format!(
                "from typing_extensions import {}",
                keep_ext.join(", ")
            ));
        }

        self.edits.push(Fix::safe_edit(Edit::range_replacement(
            parts.join("\n"),
            node.range(),
        )));
    }
}

impl<'ast> Visitor<'ast> for TypingRedirectReverse {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        if let Stmt::ImportFrom(node) = stmt {
            self.process_import(node);
        }
        walk_stmt(self, stmt);
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
    fn self_to_typing() {
        check(
            "from typing_extensions import Self\n",
            "from typing import Self\n",
        );
    }

    #[test]
    fn mixed_ext_and_typing() {
        check(
            "from typing_extensions import Self, Annotated\n",
            indoc! {"
                from typing import Self
                from typing_extensions import Annotated
            "},
        );
    }

    #[test]
    fn non_typing_name_unchanged() {
        check(
            "from typing_extensions import Annotated\n",
            "from typing_extensions import Annotated\n",
        );
    }

    #[test]
    fn multiple_typing_names() {
        check(
            "from typing_extensions import Never, LiteralString, Unpack\n",
            "from typing import Never, LiteralString, Unpack\n",
        );
    }

    #[test]
    fn plain_typing_unchanged() {
        check(
            "from typing import TypeVar\n",
            "from typing import TypeVar\n",
        );
    }
}
