mod args;

use std::ffi::OsString;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

use args::{CheckCommand, Cli, Command as Cmd, ExplainCommand, HelpFormat};
use basedpython::config::{Config, PythonVersion};
use clap::{CommandFactory, Parser};
use walkdir::WalkDir;

#[allow(unused_imports)]
use std::path::Path as _;

fn main() {
    let cli = Cli::parse();
    match cli.command {
        // ── ty commands (run natively via ty::run_from_args) ─────────────────
        Cmd::Check(check) => cmd_check(check),
        Cmd::Server => cmd_ty_args(&["server"]),
        Cmd::Version { output_format } => cmd_version(output_format),
        Cmd::GenerateShellCompletion { shell } => {
            shell.generate(&mut Cli::command(), &mut io::stdout());
        }
        Cmd::Explain { command } => match command {
            ExplainCommand::Rule { rule, output_format } => {
                let mut ty_args: Vec<OsString> =
                    vec!["by".into(), "explain".into(), "rule".into()];
                match output_format {
                    HelpFormat::Json => ty_args.extend(["--output-format".into(), "json".into()]),
                    HelpFormat::Text => {}
                }
                if let Some(r) = rule {
                    ty_args.push(r.into());
                }
                run_ty(ty_args);
            }
        },

        // ── basedpython commands ─────────────────────────────────────────────
        Cmd::Run { module, min_version } => {
            cmd_run(&module, &parse_version(&min_version));
        }
        Cmd::Build { min_version } => cmd_build(&parse_version(&min_version)),
        Cmd::Transpile { file, in_place, reverse, min_version } => {
            cmd_transpile(file, in_place, reverse, &parse_version(&min_version));
        }
    }
}

fn parse_version(s: &str) -> Config {
    let version = PythonVersion::parse(s)
        .unwrap_or_else(|| die!("unknown Python version {s:?} — use e.g. 3.12"));
    Config { min_version: version }
}

// ── run ──────────────────────────────────────────────────────────────────────

fn cmd_run(module: &str, config: &Config) {
    let cwd = std::env::current_dir().unwrap_or_else(|e| die!("cwd: {e}"));
    let tmp = tempfile::TempDir::new().unwrap_or_else(|e| die!("tempdir: {e}"));

    for bpy in bpy_files(&cwd) {
        let py = tmp
            .path()
            .join(bpy.strip_prefix(&cwd).unwrap())
            .with_extension("py");
        fs::create_dir_all(py.parent().unwrap()).unwrap();
        fs::write(&py, transpile_path(&bpy, config)).unwrap();
    }

    let python = std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_owned());
    let status = Command::new(&python)
        .arg("-m")
        .arg(module)
        .current_dir(tmp.path())
        .status()
        .unwrap_or_else(|e| die!("{python}: {e}"));

    std::process::exit(status.code().unwrap_or(1));
}

// ── build ────────────────────────────────────────────────────────────────────

fn cmd_build(config: &Config) {
    let cwd = std::env::current_dir().unwrap_or_else(|e| die!("cwd: {e}"));
    let out = cwd.join("out");
    let files = bpy_files(&cwd);

    if files.is_empty() {
        eprintln!("no .by files found");
        return;
    }

    for bpy in &files {
        let py = out
            .join(bpy.strip_prefix(&cwd).unwrap())
            .with_extension("py");
        fs::create_dir_all(py.parent().unwrap()).unwrap();
        fs::write(&py, transpile_path(bpy, config)).unwrap();
        eprintln!("{} -> {}", bpy.display(), py.display());
    }

    eprintln!("\nbuild complete ({} files)", files.len());
}

// ── transpile ────────────────────────────────────────────────────────────────

fn cmd_transpile(file: Option<PathBuf>, in_place: bool, reverse: bool, config: &Config) {
    let (source, path) = match &file {
        Some(p) => (
            fs::read_to_string(p).unwrap_or_else(|e| die!("{e}")),
            Some(p.as_path()),
        ),
        None => {
            let mut s = String::new();
            io::stdin()
                .read_to_string(&mut s)
                .unwrap_or_else(|e| die!("{e}"));
            (s, None)
        }
    };

    let output = if reverse {
        basedpython::reverse_transpile(&source, config).unwrap_or_else(|e| die!("{e}"))
    } else {
        basedpython::transpile(&source, config).unwrap_or_else(|e| die!("{e}"))
    };

    if in_place {
        let p = path.unwrap_or_else(|| die!("--in-place requires a file argument"));
        fs::write(p, &output).unwrap_or_else(|e| die!("{e}"));
    } else {
        print!("{output}");
    }
}

// ── check ────────────────────────────────────────────────────────────────────

fn cmd_check(check: CheckCommand) {
    let cwd = std::env::current_dir().unwrap_or_else(|e| die!("cwd: {e}"));
    let ty_args = check_to_ty_args(&check, &cwd);
    run_ty(ty_args);
}

fn check_to_ty_args(cmd: &CheckCommand, _cwd: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["by".into(), "check".into()];

    if cmd.fix {
        args.push("--fix".into());
    }
    if cmd.add_ignore {
        args.push("--add-ignore".into());
    }
    if let Some(p) = &cmd.project {
        args.extend(["--project".into(), p.into()]);
    }
    if let Some(p) = &cmd.python {
        args.extend(["--python".into(), p.into()]);
    }
    if let Some(p) = &cmd.typeshed {
        args.extend(["--typeshed".into(), p.into()]);
    }
    if let Some(paths) = &cmd.extra_search_path {
        for p in paths {
            args.extend(["--extra-search-path".into(), p.into()]);
        }
    }
    if let Some(v) = &cmd.python_version {
        args.extend(["--python-version".into(), v.as_str().into()]);
    }
    if let Some(p) = &cmd.python_platform {
        args.extend(["--python-platform".into(), p.into()]);
    }
    for _ in 0..cmd.verbosity.verbose {
        args.push("-v".into());
    }
    for _ in 0..cmd.verbosity.quiet {
        args.push("-q".into());
    }
    for (rule, level) in &cmd.rules.0 {
        args.extend([level.as_flag().into(), rule.into()]);
    }
    for cfg in &cmd.config.0 {
        args.extend(["-c".into(), cfg.into()]);
    }
    if let Some(p) = &cmd.config_file {
        args.extend(["--config-file".into(), p.into()]);
    }
    if let Some(fmt) = &cmd.output_format {
        args.extend(["--output-format".into(), fmt.as_str().into()]);
    }
    if let Some(v) = cmd.error_on_warning {
        args.push(if v { "--error-on-warning" } else { "--error-on-warning=false" }.into());
    }
    if cmd.exit_zero {
        args.push("--exit-zero".into());
    }
    if cmd.watch {
        args.push("--watch".into());
    }
    match (cmd.force_exclude, cmd.no_force_exclude) {
        (true, false) => args.push("--force-exclude".into()),
        (false, true) => args.push("--no-force-exclude".into()),
        _ => {}
    }
    match (cmd.respect_ignore_files, cmd.no_respect_ignore_files) {
        (Some(true), false) => args.push("--respect-ignore-files".into()),
        (_, true) | (Some(false), false) => args.push("--no-respect-ignore-files".into()),
        _ => {}
    }
    if let Some(patterns) = &cmd.exclude {
        for p in patterns {
            args.extend(["--exclude".into(), p.into()]);
        }
    }
    if let Some(c) = &cmd.color {
        args.extend(["--color".into(), c.as_str().into()]);
    }
    if cmd.no_progress {
        args.push("--no-progress".into());
    }

    // pass paths through directly — ty now understands .by natively
    for path in &cmd.paths {
        args.push(path.into());
    }

    args
}

// ── version ──────────────────────────────────────────────────────────────────

fn cmd_version(output_format: HelpFormat) {
    let version = env!("CARGO_PKG_VERSION");
    match output_format {
        HelpFormat::Text => println!("by {version}"),
        HelpFormat::Json => println!("{{\"version\":\"{version}\"}}"),
    }
}

// ── ty passthrough (in-process, no subprocess) ────────────────────────────────

fn cmd_ty_args(subcmd_args: &[&str]) {
    let mut args: Vec<OsString> = vec!["by".into()];
    args.extend(subcmd_args.iter().map(|s| OsString::from(*s)));
    run_ty(args);
}

fn run_ty(args: Vec<OsString>) {
    match ty::run_from_args(args) {
        Ok(status) => std::process::exit(status as i32),
        Err(e) => die!("{e}"),
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn bpy_files(root: &Path) -> Vec<PathBuf> {
    let out = root.join("out");
    WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            !p.starts_with(&out) && p.extension().is_some_and(|x| x == "by")
        })
        .map(|e| e.into_path())
        .collect()
}

fn transpile_path(path: &Path, config: &Config) -> String {
    let src = fs::read_to_string(path).unwrap_or_else(|e| die!("{}: {e}", path.display()));
    basedpython::transpile(&src, config).unwrap_or_else(|e| die!("{}: {e}", path.display()))
}

macro_rules! die {
    ($($t:tt)*) => {{
        eprintln!("error: {}", format!($($t)*));
        std::process::exit(1)
    }};
}
use die;
