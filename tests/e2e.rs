use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

fn transpile(source: &str) -> String {
    run_transpile(source, &[])
}

fn reverse_transpile(source: &str) -> String {
    run_transpile(source, &["--reverse"])
}

fn run_transpile(source: &str, extra_args: &[&str]) -> String {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_by"));
    cmd.arg("transpile");
    cmd.args(extra_args);
    let mut child = cmd
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
    assert!(
        output.status.success(),
        "by exited with error:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn run_executes_module() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join("main.by"), "print('hello from by run')\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_by"))
        .args(["run", "main"])
        .current_dir(dir.path())
        .output()
        .expect("failed to spawn by");

    assert!(
        output.status.success(),
        "by run failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "hello from by run"
    );
}

#[test]
fn run_applies_transforms() {
    // Exercises subscript normalization end-to-end: the .by source uses a
    // tuple subscript, which basedpython rewrites so __getitem__ receives a
    // 1-tuple. The class unpacks accordingly and prints the values.
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(
        dir.path().join("main.by"),
        "\
class Grid:
    def __getitem__(self, key):
        (row, col), = key
        print(row, col)

Grid()[(1, 2)]
",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_by"))
        .args(["run", "main"])
        .current_dir(dir.path())
        .output()
        .expect("failed to spawn by");

    assert!(
        output.status.success(),
        "by run failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "1 2"
    );
}

#[test]
fn parenthesized_tuple_subscript() {
    assert_eq!(transpile("x = {}\nx[(a, b)]\n"), "x = {}\nx[(a, b),]\n");
}

#[test]
fn bare_tuple_subscript() {
    assert_eq!(transpile("x = {}\nx[a, b]\n"), "x = {}\nx[(a, b),]\n");
}

#[test]
fn scalar_subscript_unchanged() {
    assert_eq!(transpile("x = {}\nx[a]\n"), "x = {}\nx[a]\n");
}

#[test]
fn subscript_in_function() {
    let src = "d = {}\ndef foo():\n    return d[(x, y)]\n";
    let out = "d = {}\ndef foo():\n    return d[(x, y),]\n";
    assert_eq!(transpile(src), out);
}

#[test]
fn multiple_subscripts_in_one_file() {
    let src = "a = {}\nb = {}\nc = {}\na[(1, 2)]\nb[(3, 4)]\nc[x]\n";
    let out = "a = {}\nb = {}\nc = {}\na[(1, 2),]\nb[(3, 4),]\nc[x]\n";
    assert_eq!(transpile(src), out);
}

#[test]
fn comments_and_unrelated_code_preserved() {
    let src = "# a comment\nx = 1\ny = {}\ny[(a, b)]\n";
    let out = "# a comment\nx = 1\ny = {}\ny[(a, b),]\n";
    assert_eq!(transpile(src), out);
}

#[test]
fn reverse_empty_class() {
    assert_eq!(reverse_transpile("class A: ...\n"), "class A\n");
}

#[test]
fn export_generates_dunder_all() {
    let src = "export def api(): ...\nprivate def helper(): ...\ndef internal(): ...\n";
    let out = "def api(): ...\ndef _helper(): ...\ndef internal(): ...\n__all__ = [\"api\"]\n";
    assert_eq!(transpile(src), out);
}

#[test]
fn reverse_literal_union() {
    assert_eq!(
        reverse_transpile("a: Literal[1, 2]\n"),
        "a: 1 | 2\n",
    );
}

#[test]
fn reverse_paren_tuple_in_type_subscript() {
    assert_eq!(
        reverse_transpile("a: dict[(int, str)]\n"),
        "a: dict[int, str]\n",
    );
}
