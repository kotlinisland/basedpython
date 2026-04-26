mod reverse_transforms;
mod transforms;
pub mod config;
pub mod source_map;
pub mod symbol_table;

pub use config::Config;
pub use source_map::SourceMap;

use ruff_python_ast::visitor::Visitor;
use ruff_python_parser::parse_module;
use ruff_text_size::{TextRange, TextSize};

use symbol_table::SymbolTable;

pub fn transpile(source: &str, config: &Config) -> Result<String, String> {
    transpile_with_map(source, config).map(|(s, _)| s)
}

pub fn transpile_with_map(source: &str, config: &Config) -> Result<(String, SourceMap), String> {
    let parsed = parse_module(source).map_err(|e| e.to_string())?;
    let symbols = SymbolTable::build(source, parsed.suite());

    let mut edits: Vec<(TextRange, String)> = Vec::new();

    // --- transforms ---
    let mut subscript = transforms::subscript::SubscriptNormalizer::new(source, &symbols);
    let mut mut_defaults = transforms::mutable_defaults::MutableDefaultFixer::new(source);
    let mut typing_redirect =
        transforms::typing_redirect::TypingRedirect::new(config.clone());
    let mut generics = transforms::generics::GenericPolyfill::new(source, &symbols, config.clone());
    let mut compat =
        transforms::compat::CompatRewriteWithSource::new(source, config.clone());
    let mut tuple_types = transforms::annotation::TupleLiteralType::new(source);
    let mut literal_types = transforms::literal_types::LiteralType::new(source, &symbols);
    let mut auto_quote = transforms::auto_quote::AutoQuote::new(source);
    let mut intersection = transforms::intersection::IntersectionType::new(source);
    let mut callable = transforms::callable::CallableSyntax::new(source);
    let mut unpack = transforms::unpack::UnpackSyntax::new(source, config.clone());
    let mut empty_decls = transforms::empty_declarations::EmptyDeclarations::new();
    let mut modifiers = transforms::modifiers::Modifiers::new(source);
    let mut overload = transforms::overload::Overload::new(source);
    let mut coalesce = transforms::coalesce::NoneCoalesce::new(source);
    let mut none_chain = transforms::none_chain::NoneChain::new(source, &symbols);
    let mut dedent_string = transforms::dedent_string::DedentString::new(source);
    let mut typed_lambda = transforms::typed_lambda::TypedLambda::new(source);

    for stmt in parsed.suite() {
        subscript.visit_stmt(stmt);
        mut_defaults.visit_stmt(stmt);
        typing_redirect.visit_stmt(stmt);
        generics.visit_stmt(stmt);
        compat.visit_stmt(stmt);
        tuple_types.visit_stmt(stmt);
        literal_types.visit_stmt(stmt);
        auto_quote.visit_stmt(stmt);
        intersection.visit_stmt(stmt);
        callable.visit_stmt(stmt);
        unpack.visit_stmt(stmt);
        empty_decls.visit_stmt(stmt);
        modifiers.visit_stmt(stmt);
        coalesce.visit_stmt(stmt);
        none_chain.visit_stmt(stmt);
        dedent_string.visit_stmt(stmt);
        typed_lambda.visit_stmt(stmt);
    }
    // Overload uses visit_body to see sibling statements; entry via the module suite.
    use ruff_python_ast::visitor::Visitor as _;
    overload.visit_body(parsed.suite());

    edits.extend(subscript.into_edits());
    edits.extend(mut_defaults.edits);
    edits.extend(typing_redirect.edits);
    edits.extend(generics.edits);
    edits.extend(compat.edits);
    edits.extend(tuple_types.edits);
    let literal_needs_import = literal_types.needs_literal_import;
    edits.extend(literal_types.edits);
    edits.extend(auto_quote.edits);
    edits.extend(intersection.edits);
    edits.extend(callable.edits);
    edits.extend(unpack.edits);
    let exports = std::mem::take(&mut modifiers.exports);
    let private_renames = std::mem::take(&mut modifiers.private_renames);
    edits.extend(modifiers.edits);
    edits.extend(empty_decls.edits);

    // rename call sites that reference private-renamed module-level symbols
    if !private_renames.is_empty() {
        let mut name_renamer = transforms::modifiers::NameRenamer::new(&private_renames);
        for stmt in parsed.suite() {
            name_renamer.visit_stmt(stmt);
        }
        edits.extend(name_renamer.edits);
    }
    edits.extend(overload.edits);
    edits.extend(coalesce.edits);
    edits.extend(none_chain.edits);
    edits.extend(dedent_string.edits);
    edits.extend(typed_lambda.edits);

    let kept_edits = dedup_edits(edits);
    let result = apply_deduped(source, &kept_edits);

    // Append auto-generated `__all__` when `export`/`public` was used.
    let result = if exports.is_empty() {
        result
    } else {
        let entries = exports
            .iter()
            .map(|n| format!("\"{n}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let separator = if result.ends_with('\n') { "" } else { "\n" };
        format!("{result}{separator}__all__ = [{entries}]\n")
    };

    // --- prepend generated preamble ---
    let mut preamble = String::new();

    // Save flags before consuming generics.needed_imports.
    let generics_emits_unpack = generics.needed_imports.unpack;

    // Generic polyfill imports come first.
    if !generics.needed_imports.is_empty() {
        for line in generics.needed_imports.into_lines() {
            preamble.push_str(&line);
            preamble.push('\n');
        }
    }

    if literal_needs_import
        && !transforms::literal_types::literal_already_imported(&symbols)
    {
        preamble.push_str("from typing import Literal\n");
    }

    if intersection.needs_import {
        preamble.push_str("from ty_extensions import Intersection\n");
    }

    // Only emit the Unpack import when generics.rs didn't already include it.
    if unpack.needs_import && !generics_emits_unpack {
        preamble.push_str("from typing import Unpack\n");
    }

    // Modifier keyword imports.
    {
        let mut typing_imports: Vec<&'static str> = Vec::new();
        if callable.needs_import {
            typing_imports.push("Callable");
        }
        if modifiers.needs_final {
            typing_imports.push("final");
        }
        if modifiers.needs_final_annotation {
            typing_imports.push("Final");
        }
        if modifiers.needs_classvar {
            typing_imports.push("ClassVar");
        }
        if modifiers.needs_newtype {
            typing_imports.push("NewType");
        }
        if modifiers.needs_override {
            typing_imports.push("override");
        }
        if overload.needs_overload {
            typing_imports.push("overload");
        }
        if !typing_imports.is_empty() {
            typing_imports.sort_unstable();
            preamble.push_str(&format!("from typing import {}\n", typing_imports.join(", ")));
        }
    }
    if modifiers.needs_abstractmethod {
        preamble.push_str("from abc import abstractmethod\n");
    }
    if modifiers.needs_dataclass {
        preamble.push_str("from dataclasses import dataclass\n");
    }
    if modifiers.needs_enum {
        preamble.push_str("from enum import Enum\n");
    }
    if modifiers.needs_protocol {
        preamble.push_str("from typing import Protocol\n");
    }

    if mut_defaults.needs_sentinel {
        preamble.push_str("_MISSING = object()\n");
    }

    let output = if preamble.is_empty() {
        result
    } else {
        format!("{preamble}{result}")
    };

    let map = SourceMap::build(source, &kept_edits, &preamble);

    Ok((output, map))
}

/// Rewrite standard Python source into idiomatic basedpython.
///
/// Counterpart to [`transpile`]: detects polyfill output patterns and
/// rewrites them to the basedpython surface form. Used for ecosystem
/// round-trip testing — `transpile(reverse_transpile(py))` should produce
/// AST-equivalent code to `transpile(py)`.
pub fn reverse_transpile(source: &str, _config: &Config) -> Result<String, String> {
    let parsed = parse_module(source).map_err(|e| e.to_string())?;
    let symbols = SymbolTable::build(source, parsed.suite());

    let mut edits: Vec<(TextRange, String)> = Vec::new();

    let mut empty_class = reverse_transforms::empty_class::EmptyClass::new();
    let mut literal_types = reverse_transforms::literal_types::LiteralReverse::new(source, &symbols);
    let mut subscript = reverse_transforms::subscript::SubscriptReverse::new(source, &symbols);

    for stmt in parsed.suite() {
        empty_class.visit_stmt(stmt);
        literal_types.visit_stmt(stmt);
        subscript.visit_stmt(stmt);
    }

    edits.extend(empty_class.edits);
    edits.extend(literal_types.edits);
    edits.extend(subscript.edits);

    Ok(apply_edits(source, edits))
}

/// sort and deduplicate edits: ascending by start, drop subsumed ranges
fn dedup_edits(mut edits: Vec<(TextRange, String)>) -> Vec<(TextRange, String)> {
    edits.sort_by_key(|e| (e.0.start(), std::cmp::Reverse(e.0.end())));
    let mut kept: Vec<(TextRange, String)> = Vec::new();
    let mut max_end = TextSize::from(0);
    for (range, text) in edits {
        if range.start() >= max_end {
            if range.end() > max_end {
                max_end = range.end();
            }
            kept.push((range, text));
        }
    }
    kept
}

fn apply_deduped(source: &str, deduped: &[(TextRange, String)]) -> String {
    let mut result = source.to_string();
    for (range, new_text) in deduped.iter().rev() {
        let start = usize::from(range.start());
        let end = usize::from(range.end());
        result.replace_range(start..end, new_text);
    }
    result
}

fn apply_edits(source: &str, edits: Vec<(TextRange, String)>) -> String {
    let kept = dedup_edits(edits);
    apply_deduped(source, &kept)
}
