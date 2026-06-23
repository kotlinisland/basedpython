//! checker/runtime divergence harness.
//!
//! every `.by` code block in the basedpython mdtests that the checker accepts
//! (no `# error:` assertions) must also transpile and *execute* cleanly: the
//! mdtest framework verifies ty's diagnostics, this test verifies the runtime
//! half of the contract. divergences of the form "checks clean but crashes at
//! runtime" (enum constants becoming members, transform composition leaks,
//! unsound lowerings) are exactly the bug class this catches.
//!
//! blocks carrying expected diagnostics are skipped — their runtime behaviour
//! is intentionally unspecified.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn mdtest_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../ty_python_semantic/resources/mdtest")
}

/// A CPython 3.13 interpreter, provisioned through uv so the harness runs the
/// same interpreter everywhere instead of riding on whatever `python3` the host
/// happens to ship. The transpiler emits modern syntax — PEP 695 generics, PEP
/// 696 type-parameter defaults (`class C[T = int]`), PEP 646 unpacking — whose
/// runtime floor is 3.13; CI runners range from 3.10 upward, so a checker-clean
/// block can fail to even parse on an older interpreter. Returns `None` (the
/// test then skips) when uv or the interpreter can't be obtained.
fn python() -> Option<String> {
    if let Ok(p) = std::env::var("PYTHON") {
        return Some(p);
    }
    let find = || {
        let out = Command::new("uv")
            .args(["python", "find", "3.13"])
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
    };
    if let Some(path) = find() {
        return Some(path);
    }
    // not discoverable yet — let uv download a managed build, then locate it
    Command::new("uv")
        .args(["python", "install", "3.13"])
        .output()
        .ok()?;
    find()
}

/// `major.minor` of the interpreter the blocks will execute on, so the
/// transpile targets what it actually supports.
fn python_version(python: &str) -> Option<String> {
    let output = Command::new(python)
        .arg("-c")
        .arg("import sys; print(f'{sys.version_info[0]}.{sys.version_info[1]}')")
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Extract the `by` fenced code blocks of a markdown file, in order, with a flag
/// for blocks living in a multi-file section (one that declares companion
/// modules via a `` `name.py`: `` marker) — those import section-local modules
/// and cannot run standalone.
fn by_blocks(markdown: &str) -> Vec<(String, bool)> {
    let mut blocks: Vec<(String, usize)> = Vec::new();
    let mut multi_file_sections: Vec<usize> = Vec::new();
    let mut section = 0usize;
    let mut current: Option<String> = None;
    for line in markdown.lines() {
        if current.is_none() && line.starts_with('#') {
            section += 1;
        }
        // a companion-module marker: `` `pylib.py`: `` ahead of its fence
        if current.is_none()
            && line.trim().starts_with('`')
            && (line.trim().ends_with(".py`:")
                || line.trim().ends_with(".by`:")
                || line.trim().ends_with(".byi`:"))
        {
            multi_file_sections.push(section);
        }
        match &mut current {
            None if line.trim() == "```by" => current = Some(String::new()),
            None => {}
            Some(block) => {
                if line.trim() == "```" {
                    blocks.push((current.take().expect("block in progress"), section));
                } else {
                    block.push_str(line);
                    block.push('\n');
                }
            }
        }
    }
    blocks
        .into_iter()
        .map(|(b, s)| (b, multi_file_sections.contains(&s)))
        .collect()
}

fn transpile(source: &str, min_version: &str) -> Result<String, String> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_by"))
        .args(["transpile", "--min-version", min_version])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn by");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

/// Stub `reveal_type` (an mdtest debugging device with no runtime binding)
/// after the `__future__` import, which must stay first.
fn with_reveal_stub(transpiled: &str) -> String {
    const STUB: &str = "def reveal_type(x, *a, **k):\n    return x\n";
    match transpiled.strip_prefix("from __future__ import annotations\n") {
        Some(rest) => format!("from __future__ import annotations\n{STUB}{rest}"),
        None => format!("{STUB}{transpiled}"),
    }
}

#[test]
#[expect(clippy::print_stderr, reason = "skip diagnostic when python is unavailable")]
fn clean_mdtest_blocks_run() {
    let Some(python) = python() else {
        eprintln!("skipping: uv could not provide a python 3.13 interpreter");
        return;
    };
    let Some(version) = python_version(&python) else {
        eprintln!("skipping: `{python}` not runnable");
        return;
    };

    // third-party runtime deps are environment-dependent; skip blocks that
    // need one the interpreter doesn't have
    let has_typing_extensions = Command::new(&python)
        .args(["-c", "import typing_extensions"])
        .output()
        .is_ok_and(|o| o.status.success());

    let mut failures: Vec<String> = Vec::new();
    let mut total = 0usize;
    let dir = mdtest_dir();
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("mdtest dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("basedpython_"))
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no basedpython mdtests found in {dir:?}");

    let tmp = tempfile::tempdir().expect("tempdir");
    for file in files {
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        let markdown = fs::read_to_string(&file).expect("read mdtest");
        for (i, (block, multi_file)) in by_blocks(&markdown).iter().enumerate() {
            // a block with expected diagnostics is allowed to misbehave at
            // runtime (only checker-clean blocks carry the contract), and a
            // block in a multi-file section imports section-local modules this
            // harness doesn't materialize
            if block.contains("# error:") || *multi_file {
                continue;
            }
            total += 1;
            let transpiled = match transpile(block, &version) {
                Ok(t) => t,
                Err(e) => {
                    failures.push(format!(
                        "{name} block {i}: transpile failed:\n{e}\n--- block ---\n{block}"
                    ));
                    continue;
                }
            };
            if !has_typing_extensions && transpiled.contains("typing_extensions") {
                continue;
            }
            let py = tmp.path().join(format!(
                "{}_{i}.py",
                name.trim_end_matches(".md").replace('-', "_")
            ));
            fs::write(&py, with_reveal_stub(&transpiled)).unwrap();
            let run = Command::new(&python)
                .arg(&py)
                .output()
                .expect("failed to run python");
            if !run.status.success() {
                failures.push(format!(
                    "{name} block {i}: checker-clean block crashed at runtime:\n{}\n--- block ---\n{block}",
                    String::from_utf8_lossy(&run.stderr)
                ));
            }
        }
    }

    assert!(total > 0, "no checker-clean by blocks found");
    assert!(
        failures.is_empty(),
        "{} of {} checker-clean blocks diverge at runtime:\n\n{}",
        failures.len(),
        total,
        failures.join("\n\n")
    );
}
