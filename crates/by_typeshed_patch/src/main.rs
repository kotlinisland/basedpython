//! binary entry: walks the basedpython typeshed and applies every registered
//! patch. invoked by `scripts/sync_typeshed_by.sh` after reverse-transpile
//! and before `ruff --fix`
//!
//! usage:
//!   `by_typeshed_patch` `<typeshed-stdlib-dir>`

#![allow(clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{env, fs};

use anyhow::{Context, Result, bail};
use ruff_python_ast::PySourceType;
use ruff_python_parser::parse_unchecked_source;
use walkdir::WalkDir;

use by_typeshed_patch::{Patch, all_patches, apply_edits};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let [_, dir] = args.as_slice() else {
        bail!("usage: by_typeshed_patch <typeshed-stdlib-dir>");
    };
    let root = PathBuf::from(dir);
    if !root.is_dir() {
        bail!("not a directory: {}", root.display());
    }

    let patches = all_patches();
    if patches.is_empty() {
        eprintln!("no patches registered; nothing to do");
        return Ok(());
    }

    let mut patched = 0_usize;
    let mut visited = 0_usize;
    for entry in WalkDir::new(&root).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "byi") {
            continue;
        }
        visited += 1;
        let rel = path.strip_prefix(&root).unwrap_or(path);
        if apply_patches_to_file(path, rel, &patches)
            .with_context(|| format!("applying patches to {}", path.display()))?
        {
            patched += 1;
        }
    }
    eprintln!("visited {visited} file(s); patched {patched}");
    Ok(())
}

fn apply_patches_to_file(path: &Path, rel: &Path, patches: &[Box<dyn Patch>]) -> Result<bool> {
    let source = fs::read_to_string(path).with_context(|| format!("{}", path.display()))?;
    // soft errors retained on `parsed.errors()`; we don't gate on them — the
    // basedpython parser accepts patterns (decorator + modifier chain) that
    // the standalone parser still flags. real parse failures are caught by
    // `cargo nextest run` downstream
    let parsed = parse_unchecked_source(&source, PySourceType::BasedPythonStub);

    let mut edits = Vec::new();
    for patch in patches {
        edits.extend(patch.rewrite(rel, &parsed, &source));
    }
    if edits.is_empty() {
        return Ok(false);
    }
    let new_source = apply_edits(&source, edits);
    fs::write(path, new_source).with_context(|| format!("{}", path.display()))?;
    Ok(true)
}
