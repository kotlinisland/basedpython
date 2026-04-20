use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

use basedpython::config::{Config, PythonVersion};
use clap::{Parser, Subcommand};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "by", about = "The basedpython build tool")]
struct Cli {
    /// Minimum Python version the output must run on
    #[arg(long, value_name = "VERSION", global = true, default_value = "3.10")]
    min_version: String,

    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Transpile and run a module with `python -m <module>`
    Run {
        /// Module to run (e.g. `by run main` looks for main.by)
        module: String,
    },
    /// Transpile all .by files and write them to out/
    Build,
    /// Transpile a single file to stdout (reads stdin if no file given)
    Transpile {
        file: Option<PathBuf>,
        /// Overwrite the source file with its Python output
        #[arg(short, long)]
        in_place: bool,
        /// Convert Python source into basedpython idioms (instead of the default by → py direction)
        #[arg(long)]
        reverse: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    let config = parse_version(&cli.min_version);
    match cli.command {
        Cmd::Run { module } => cmd_run(&module, &config),
        Cmd::Build => cmd_build(&config),
        Cmd::Transpile { file, in_place, reverse } => {
            cmd_transpile(file, in_place, reverse, &config);
        }
    }
}

fn parse_version(s: &str) -> Config {
    let version = PythonVersion::parse(s)
        .unwrap_or_else(|| die!("unknown Python version {s:?} — use e.g. 3.10, 3.11, 3.12"));
    Config { min_version: version }
}

// --- run -------------------------------------------------------------------

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

// --- build -----------------------------------------------------------------

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

// --- transpile -------------------------------------------------------------

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

// --- helpers ---------------------------------------------------------------

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
