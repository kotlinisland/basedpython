//! CLI argument definitions — forked from ty's `args.rs` with basedpython commands added.
//!
//! ty commands (`check`, `server`, `version`, `explain`, `generate-shell-completion`) are
//! kept 100% structurally identical to the upstream source so that `by` is a drop-in
//! replacement. basedpython-specific commands (`run`, `build`, `transpile`) are appended.

use std::path::PathBuf;

use clap::builder::Styles;
use clap::builder::styling::{AnsiColor, Effects};
use clap::{ArgAction, ArgMatches, Error, Parser};
use clap::error::ErrorKind;

const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default());

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    name = "by",
    about = "an extremely fast Python type checker, with basedpython support",
    styles = STYLES,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[expect(clippy::large_enum_variant)]
#[derive(Debug, clap::Subcommand)]
pub enum Command {
    // ── ty commands (identical to upstream) ─────────────────────────────────

    /// Check a project for type errors.
    Check(CheckCommand),

    /// Start the language server.
    Server,

    /// Display by's version.
    Version {
        #[arg(
            long,
            value_enum,
            default_value = "text",
            help = "The format in which to display the version information"
        )]
        output_format: HelpFormat,
    },

    /// Generate shell completion.
    #[clap(hide = true)]
    GenerateShellCompletion { shell: clap_complete_command::Shell },

    /// Explain rules and other parts of by.
    Explain {
        #[command(subcommand)]
        command: ExplainCommand,
    },

    // ── basedpython commands ─────────────────────────────────────────────────

    /// Transpile and run a module with `python -m <module>`.
    Run {
        /// module to run (e.g. `by run main` looks for main.by)
        module: String,
        /// minimum Python version the output must run on
        #[arg(long, value_name = "VERSION", default_value = "3.10")]
        min_version: String,
    },

    /// Transpile all .by files and write them to out/.
    Build {
        /// minimum Python version the output must run on
        #[arg(long, value_name = "VERSION", default_value = "3.10")]
        min_version: String,
    },

    /// Transpile a single file to stdout (reads stdin if no file given).
    Transpile {
        file: Option<PathBuf>,
        /// overwrite the source file with its Python output
        #[arg(short, long)]
        in_place: bool,
        /// convert Python source into basedpython idioms (instead of the default by → py direction)
        #[arg(long)]
        reverse: bool,
        /// minimum Python version the output must run on
        #[arg(long, value_name = "VERSION", default_value = "3.10")]
        min_version: String,
    },
}

// ── CheckCommand (ty-identical) ─────────────────────────────────────────────

#[derive(Debug, Parser)]
#[expect(clippy::struct_excessive_bools)]
pub struct CheckCommand {
    /// List of files or directories to check.
    #[clap(
        help = "List of files or directories to check [default: the project root]",
        value_name = "PATH"
    )]
    pub paths: Vec<PathBuf>,

    /// Apply fixes to resolve errors.
    #[arg(long)]
    pub fix: bool,

    /// Adds `ty: ignore` comments to suppress all rule diagnostics.
    #[arg(long, conflicts_with("fix"))]
    pub add_ignore: bool,

    /// Run the command within the given project directory.
    ///
    /// All `pyproject.toml` files will be discovered by walking up the directory tree from the
    /// given project directory, as will the project's virtual environment (`.venv`) unless the
    /// `venv-path` option is set.
    ///
    /// Other command-line arguments (such as relative paths) will be resolved relative to the
    /// current working directory.
    #[arg(long, value_name = "PROJECT")]
    pub project: Option<PathBuf>,

    /// Path to your project's Python environment or interpreter.
    ///
    /// ty uses your Python environment to resolve third-party imports in your code.
    ///
    /// This can be a path to:
    ///
    /// - A Python interpreter, e.g. `.venv/bin/python3`
    /// - A virtual environment directory, e.g. `.venv`
    /// - A system Python [`sys.prefix`] directory, e.g. `/usr`
    ///
    /// If you're using a project management tool such as uv or you have an activated Conda or
    /// virtual environment, you should not generally need to specify this option.
    ///
    /// [`sys.prefix`]: https://docs.python.org/3/library/sys.html#sys.prefix
    #[arg(long, value_name = "PATH", alias = "venv")]
    pub python: Option<PathBuf>,

    /// Custom directory to use for stdlib typeshed stubs.
    #[arg(long, value_name = "PATH", alias = "custom-typeshed-dir")]
    pub typeshed: Option<PathBuf>,

    /// Additional path to use as a module-resolution source (can be passed multiple times).
    ///
    /// This is an advanced option that should usually only be used for first-party or third-party
    /// modules that are not installed into your Python environment in a conventional way.
    /// Use `--python` to point ty to your Python environment if it is in an unusual location.
    #[arg(long, value_name = "PATH")]
    pub extra_search_path: Option<Vec<PathBuf>>,

    /// Python version to assume when resolving types.
    ///
    /// The Python version affects allowed syntax, type definitions of the standard library, and
    /// type definitions of first- and third-party modules that are conditional on the Python
    /// version.
    ///
    /// If a version is not specified on the command line or in a configuration file,
    /// ty will try the following techniques in order of preference to determine a value:
    /// 1. Check for the `project.requires-python` setting in a `pyproject.toml` file
    ///    and use the minimum version from the specified range
    /// 2. Check for an activated or configured Python environment
    ///    and attempt to infer the Python version of that environment
    /// 3. Fall back to the latest stable Python version supported by ty (see `ty check --help` output)
    #[arg(long, value_name = "VERSION", alias = "target-version", value_enum)]
    pub python_version: Option<TyPythonVersion>,

    /// Target platform to assume when resolving types.
    ///
    /// This is used to specialize the type of `sys.platform` and will affect the visibility
    /// of platform-specific functions and attributes. If the value is set to `all`, no
    /// assumptions are made about the target platform. If unspecified, the current system's
    /// platform will be used.
    #[arg(long, value_name = "PLATFORM", alias = "platform")]
    pub python_platform: Option<String>,

    #[clap(flatten)]
    pub verbosity: Verbosity,

    #[clap(flatten)]
    pub rules: RulesArg,

    #[clap(flatten)]
    pub config: ConfigsArg,

    /// The path to a `ty.toml` file to use for configuration.
    ///
    /// While ty configuration can be included in a `pyproject.toml` file, it is not allowed in
    /// this context.
    #[arg(long, env = "TY_CONFIG_FILE", value_name = "PATH")]
    pub config_file: Option<PathBuf>,

    /// The format to use for printing diagnostic messages.
    #[arg(long, env = "TY_OUTPUT_FORMAT")]
    pub output_format: Option<OutputFormat>,

    /// Use exit code 1 if there are any warning-level diagnostics.
    #[arg(long, conflicts_with = "exit_zero", default_missing_value = "true", num_args = 0..1)]
    pub error_on_warning: Option<bool>,

    /// Always use exit code 0, even when there are error-level diagnostics.
    #[arg(long)]
    pub exit_zero: bool,

    /// Watch files for changes and recheck files related to the changed files.
    #[arg(long, short = 'W')]
    pub watch: bool,

    /// Respect file exclusions via `.gitignore` and other standard ignore files.
    /// Use `--no-respect-ignore-files` to disable.
    #[arg(
        long,
        overrides_with("no_respect_ignore_files"),
        help_heading = "File selection",
        default_missing_value = "true",
        num_args = 0..1
    )]
    pub respect_ignore_files: Option<bool>,
    #[clap(long, overrides_with("respect_ignore_files"), hide = true)]
    pub no_respect_ignore_files: bool,

    /// Enforce exclusions, even for paths passed to ty directly on the command-line.
    /// Use `--no-force-exclude` to disable.
    #[arg(
        long,
        overrides_with("no_force_exclude"),
        help_heading = "File selection"
    )]
    pub force_exclude: bool,
    #[clap(long, overrides_with("force_exclude"), hide = true)]
    pub no_force_exclude: bool,

    /// Glob patterns for files to exclude from type checking.
    ///
    /// Uses gitignore-style syntax to exclude files and directories from type checking.
    /// Supports patterns like `tests/`, `*.tmp`, `**/__pycache__/**`.
    #[arg(long, help_heading = "File selection")]
    pub exclude: Option<Vec<String>>,

    /// Control when colored output is used.
    #[arg(
        long,
        value_name = "WHEN",
        help_heading = "Global options",
        display_order = 1000
    )]
    pub color: Option<TerminalColor>,

    /// Hide all progress outputs.
    ///
    /// For example, spinners or progress bars.
    #[arg(
        global = true,
        long,
        value_parser = clap::builder::BoolishValueParser::new(),
        help_heading = "Global options"
    )]
    pub no_progress: bool,
}

// ── Verbosity (ty-identical) ─────────────────────────────────────────────────

#[derive(clap::Args, Debug, Clone, Default)]
#[command(about = None, long_about = None)]
pub struct Verbosity {
    #[arg(
        long,
        short = 'v',
        help = "Use verbose output (or `-vv` and `-vvv` for more verbose output)",
        action = clap::ArgAction::Count,
        global = true,
        overrides_with = "quiet",
    )]
    pub verbose: u8,

    #[arg(
        long,
        short,
        help = "Use quiet output (or `-qq` for silent output)",
        action = clap::ArgAction::Count,
        global = true,
        overrides_with = "verbose",
    )]
    pub quiet: u8,
}

// ── RulesArg (ty-identical, preserves interleaving order) ────────────────────

/// severity level used for `--error`, `--warn`, `--ignore` reconstruction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleLevel {
    Error,
    Warn,
    Ignore,
}

impl RuleLevel {
    pub const fn as_flag(self) -> &'static str {
        match self {
            Self::Error => "--error",
            Self::Warn => "--warn",
            Self::Ignore => "--ignore",
        }
    }
}

#[derive(Debug)]
pub struct RulesArg(pub Vec<(String, RuleLevel)>);

impl RulesArg {}

impl clap::FromArgMatches for RulesArg {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, Error> {
        let mut rules = Vec::new();

        for (level, arg_id) in [
            (RuleLevel::Ignore, "ignore"),
            (RuleLevel::Warn, "warn"),
            (RuleLevel::Error, "error"),
        ] {
            let indices = matches.indices_of(arg_id).into_iter().flatten();
            let values = matches.get_many::<String>(arg_id).into_iter().flatten();
            rules.extend(indices.zip(values).map(|(i, v)| (i, v, level)));
        }

        rules.sort_by_key(|(i, _, _)| *i);
        Ok(Self(
            rules.into_iter().map(|(_, v, l)| (v.to_owned(), l)).collect(),
        ))
    }

    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), Error> {
        self.0 = Self::from_arg_matches(matches)?.0;
        Ok(())
    }
}

impl clap::Args for RulesArg {
    fn augment_args(cmd: clap::Command) -> clap::Command {
        const HEADING: &str = "Enabling / disabling rules";
        cmd.arg(
            clap::Arg::new("error")
                .long("error")
                .action(ArgAction::Append)
                .help("Treat the given rule as having severity 'error'. Can be specified multiple times. Use 'all' to apply to all rules.")
                .value_name("RULE")
                .help_heading(HEADING),
        )
        .arg(
            clap::Arg::new("warn")
                .long("warn")
                .action(ArgAction::Append)
                .help("Treat the given rule as having severity 'warn'. Can be specified multiple times. Use 'all' to apply to all rules.")
                .value_name("RULE")
                .help_heading(HEADING),
        )
        .arg(
            clap::Arg::new("ignore")
                .long("ignore")
                .action(ArgAction::Append)
                .help("Disables the rule. Can be specified multiple times. Use 'all' to apply to all rules.")
                .value_name("RULE")
                .help_heading(HEADING),
        )
    }

    fn augment_args_for_update(cmd: clap::Command) -> clap::Command {
        Self::augment_args(cmd)
    }
}

// ── ConfigsArg (ty-identical interface, simplified storage) ──────────────────

#[derive(Debug, Clone)]
pub struct ConfigsArg(pub Vec<String>);

impl clap::FromArgMatches for ConfigsArg {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, Error> {
        let values: Vec<String> = matches
            .get_many::<String>("config")
            .into_iter()
            .flatten()
            .map(|s| {
                // validate it looks like TOML key=value
                if !s.contains('=') {
                    return Err(Error::raw(
                        ErrorKind::InvalidValue,
                        format!("config option must be a TOML key=value pair, got: {s}"),
                    ));
                }
                Ok(s.to_owned())
            })
            .collect::<Result<_, _>>()?;
        Ok(Self(values))
    }

    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), Error> {
        self.0 = Self::from_arg_matches(matches)?.0;
        Ok(())
    }
}

impl clap::Args for ConfigsArg {
    fn augment_args(cmd: clap::Command) -> clap::Command {
        cmd.arg(
            clap::Arg::new("config")
                .short('c')
                .long("config")
                .value_name("CONFIG_OPTION")
                .help("A TOML `<KEY> = <VALUE>` pair overriding a specific configuration option.")
                .long_help(
                    "
A TOML `<KEY> = <VALUE>` pair (such as you might find in a `ty.toml` configuration file)
overriding a specific configuration option.

Overrides of individual settings using this option always take precedence
over all configuration files.",
                )
                .action(ArgAction::Append),
        )
    }

    fn augment_args_for_update(cmd: clap::Command) -> clap::Command {
        Self::augment_args(cmd)
    }
}

// ── Enums (ty-identical) ─────────────────────────────────────────────────────

/// Enumeration of the Python versions accepted by ty.
#[derive(Copy, Clone, Hash, Debug, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum)]
pub enum TyPythonVersion {
    #[value(name = "3.7")]
    Py37,
    #[value(name = "3.8")]
    Py38,
    #[value(name = "3.9")]
    Py39,
    #[value(name = "3.10")]
    Py310,
    #[value(name = "3.11")]
    Py311,
    #[value(name = "3.12")]
    Py312,
    #[value(name = "3.13")]
    Py313,
    #[value(name = "3.14")]
    Py314,
    #[value(name = "3.15")]
    Py315,
}

impl TyPythonVersion {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Py37 => "3.7",
            Self::Py38 => "3.8",
            Self::Py39 => "3.9",
            Self::Py310 => "3.10",
            Self::Py311 => "3.11",
            Self::Py312 => "3.12",
            Self::Py313 => "3.13",
            Self::Py314 => "3.14",
            Self::Py315 => "3.15",
        }
    }
}

/// The diagnostic output format.
#[derive(Copy, Clone, Hash, Debug, PartialEq, Eq, PartialOrd, Ord, Default, clap::ValueEnum)]
pub enum OutputFormat {
    /// Print diagnostics verbosely, with context and helpful hints (default).
    #[default]
    #[value(name = "full")]
    Full,
    /// Print diagnostics concisely, one per line.
    #[value(name = "concise")]
    Concise,
    /// Print diagnostics in the JSON format expected by GitLab Code Quality reports.
    #[value(name = "gitlab")]
    Gitlab,
    /// Print diagnostics in the format used by GitHub Actions workflow error annotations.
    #[value(name = "github")]
    Github,
    /// Print diagnostics as a JUnit-style XML report.
    #[value(name = "junit")]
    Junit,
}

impl OutputFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Concise => "concise",
            Self::Gitlab => "gitlab",
            Self::Github => "github",
            Self::Junit => "junit",
        }
    }
}

/// Control when colored output is used.
#[derive(Copy, Clone, Hash, Debug, PartialEq, Eq, PartialOrd, Ord, Default, clap::ValueEnum)]
pub enum TerminalColor {
    /// Display colors if the output goes to an interactive terminal.
    #[default]
    Auto,
    /// Always display colors.
    Always,
    /// Never display colors.
    Never,
}

impl TerminalColor {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Always => "always",
            Self::Never => "never",
        }
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum HelpFormat {
    Text,
    Json,
}

#[derive(Debug, clap::Subcommand)]
pub enum ExplainCommand {
    /// Explain a rule (or all rules).
    Rule {
        /// Rule to explain
        ///
        /// Defaults to all rules if omitted.
        #[arg(hide_possible_values = true)]
        rule: Option<String>,

        /// Output format
        #[arg(long, value_enum, default_value = "text")]
        output_format: HelpFormat,
    },
}
