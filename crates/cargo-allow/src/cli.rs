use allow_core::{CargoAllowError, CargoAllowResult};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use std::env;
use std::ffi::OsStr;

use crate::{
    add, audit, check, completions, diff, doctor, explain, init, list, migrate, precommit_tool,
    propose, prune, refresh, vocabulary, why, worklist,
};

#[derive(Debug, Parser)]
#[command(
    name = "cargo-allow",
    about = "Source-tree exception ledger and policy scanner for Rust repositories",
    version,
    propagate_version = true
)]
pub(crate) struct CargoAllowCli {
    /// Control terminal styling of human output.
    ///
    /// `auto` styles only an interactive terminal. `always` forces styling
    /// on stdout; `never` disables it. Machine formats (JSON, SARIF,
    /// receipts) and `--output` files are never styled. Precedence:
    /// explicit flag > NO_COLOR > CLICOLOR_FORCE > CARGO_TERM_COLOR >
    /// terminal capability.
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto, global = true)]
    pub(crate) color: ColorChoice,
    /// Suppress non-essential output (claim boundary, matched inventory,
    /// non-matched advisory outcomes). Show only result + counts.
    #[arg(short = 'q', long, global = true)]
    pub(crate) quiet: bool,
    #[command(subcommand)]
    pub(crate) command: Option<CargoAllowCommand>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ColorChoice {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Subcommand)]
pub(crate) enum CargoAllowCommand {
    /// Create policy/allow.toml.
    Init(init::InitArgs),
    /// Inventory exceptions and policy health.
    Audit(audit::ReportArgs),
    /// CI gate for the exception ledger.
    Check(check::CheckArgs),
    /// PR-oriented report with git changed files.
    Diff(diff::DiffArgs),
    /// List allow entries.
    List(list::ListArgs),
    /// Explain one allow entry.
    Explain(explain::ExplainArgs),
    /// Explain why a finding at a path/line is unreceipted.
    Why(why::WhyArgs),
    /// Generate an allow entry from a current finding.
    Add(add::AddArgs),
    /// Generate temporary baseline_debt entries.
    Propose(propose::ProposeArgs),
    /// Emit actionable work items for humans or agents.
    Worklist(worklist::WorklistArgs),
    /// Convert compatible legacy policy files and xtask/ripr bespoke ledgers.
    Migrate(migrate::MigrateArgs),
    /// Record operator-approved advisory drift refresh for one allow entry.
    Refresh(refresh::RefreshArgs),
    /// Preview or remove stale allow entries.
    Prune(prune::PruneArgs),
    /// Validate local setup.
    Doctor(doctor::DoctorArgs),
    /// List finding kinds, evidence prefixes, and match statuses.
    Vocabulary(vocabulary::VocabularyArgs),
    /// Inspect the selected cargo-allow tool identity and capabilities.
    Tool(precommit_tool::ToolArgs),
    /// Generate a shell completion script.
    Completions(completions::CompletionsArgs),
}

/// Resolve the process output style from the flag, the environment, and
/// terminal capability (#2572).
///
/// This is the only place the colour environment is read. `IsTerminal` comes
/// from std, so no dependency is needed for capability detection.
fn resolve_output_style(choice: ColorChoice) -> allow_report::Style {
    use std::io::IsTerminal;

    let no_color = env::var("NO_COLOR").ok();
    let clicolor_force = env::var("CLICOLOR_FORCE").ok();
    let cargo_term_color = env::var("CARGO_TERM_COLOR").ok();

    let (style, _reason) = allow_report::resolve_style(
        match choice {
            ColorChoice::Auto => allow_report::ColorChoice::Auto,
            ColorChoice::Always => allow_report::ColorChoice::Always,
            ColorChoice::Never => allow_report::ColorChoice::Never,
        },
        allow_report::StyleEnv {
            no_color: no_color.as_deref(),
            clicolor_force: clicolor_force.as_deref(),
            cargo_term_color: cargo_term_color.as_deref(),
            stdout_is_terminal: std::io::stdout().is_terminal(),
        },
    );
    style
}

pub(crate) fn run() -> CargoAllowResult<()> {
    // Preserve OS-native argv all the way into clap. `std::env::args()` panics
    // when any argument is not valid UTF-8, which rejected otherwise supported
    // Unix repository roots before staged delegation could pass the path to
    // `Command::arg` as an `OsStr` (#2883).
    let cli = CargoAllowCli::parse_from(normalized_args(env::args_os()));
    // One shared style decision for the whole process (#2572). Renderers do
    // not read the environment themselves; `print_report` applies this only to
    // human output going to stdout.
    crate::reporting::set_output_style(resolve_output_style(cli.color));
    if cli.quiet {
        // Report renderers check this env var to suppress non-essential output
        // (claim boundary, matched inventory, advisory outcomes). #2785.
        // Safety: set_var is safe here because we're single-threaded before
        // any scan/match work begins.
        unsafe {
            std::env::set_var("CARGO_ALLOW_QUIET", "1");
        }
    }
    let Some(command) = cli.command else {
        CargoAllowCli::command()
            .print_help()
            .map_err(|e| CargoAllowError::new(format!("failed to print help: {e}")))?;
        println!();
        return Ok(());
    };
    match command {
        CargoAllowCommand::Init(args) => init::cmd_init(&args),
        CargoAllowCommand::Audit(args) => audit::cmd_audit(&args),
        CargoAllowCommand::Check(args) => check::cmd_check(&args),
        CargoAllowCommand::Diff(args) => diff::cmd_diff(&args),
        CargoAllowCommand::List(args) => list::cmd_list(&args),
        CargoAllowCommand::Explain(args) => explain::cmd_explain(&args),
        CargoAllowCommand::Why(args) => why::cmd_why(&args),
        CargoAllowCommand::Add(args) => add::cmd_add(&args),
        CargoAllowCommand::Propose(args) => propose::cmd_propose(&args),
        CargoAllowCommand::Worklist(args) => worklist::cmd_worklist(&args),
        CargoAllowCommand::Migrate(args) => migrate::cmd_migrate(&args),
        CargoAllowCommand::Refresh(args) => refresh::cmd_refresh(&args),
        CargoAllowCommand::Prune(args) => prune::cmd_prune(&args),
        CargoAllowCommand::Doctor(args) => doctor::cmd_doctor(&args),
        CargoAllowCommand::Vocabulary(args) => vocabulary::cmd_vocabulary(&args),
        CargoAllowCommand::Tool(args) => precommit_tool::cmd_tool(&args),
        CargoAllowCommand::Completions(args) => completions::cmd_completions(&args),
    }
}

pub(crate) fn normalized_args<I, S>(args: I) -> Vec<S>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut args = args.into_iter().collect::<Vec<_>>();
    if let Some(index) = leading_cargo_allow_shim_index(&args) {
        args.remove(index);
    }
    args
}

/// Locate the cargo-plugin `allow` shim token (`cargo allow …`).
///
/// Cargo invokes this binary as `cargo-allow` and inserts a literal `allow`
/// before the real subcommand or root flags. Strip that shim when:
/// - a known subcommand follows (flags may appear in between), or
/// - only root flags follow (for example `cargo allow --version`).
///
/// Bare `cargo-allow allow` with no further tokens keeps `allow` so a future
/// `Allow` subcommand is not permanently reserved. Unknown non-flag tokens
/// after `allow` also keep it (for example `cargo-allow allow future-cmd`).
fn leading_cargo_allow_shim_index<S>(args: &[S]) -> Option<usize>
where
    S: AsRef<OsStr>,
{
    for (index, arg) in args.iter().enumerate().skip(1) {
        let arg = arg.as_ref();
        if arg == OsStr::new("allow") {
            return if should_strip_cargo_allow_shim(args, index + 1) {
                Some(index)
            } else {
                None
            };
        }
        if is_known_subcommand(arg) {
            return None;
        }
    }
    None
}

fn should_strip_cargo_allow_shim<S>(args: &[S], start: usize) -> bool
where
    S: AsRef<OsStr>,
{
    let mut saw_token = false;
    for arg in args.iter().skip(start) {
        let arg = arg.as_ref();
        saw_token = true;
        if is_known_subcommand(arg) {
            return true;
        }
        if arg.as_encoded_bytes().first() == Some(&b'-') {
            continue;
        }
        // A non-flag token that is not a known subcommand means `allow` is
        // itself the command (or an unknown command), not a cargo shim.
        return false;
    }
    // Only flags after `allow` (for example `--version`) are cargo-plugin root
    // options. Empty tail keeps bare `allow` free for a future subcommand.
    saw_token
}

fn is_known_subcommand(arg: &OsStr) -> bool {
    arg.to_str()
        .is_some_and(|arg| CargoAllowCommand::SUBCOMMANDS.contains(&arg))
}

impl CargoAllowCommand {
    /// Installed subcommand names. Do not add `"allow"` here: that token is the
    /// cargo-plugin shim stripped by [`normalized_args`] when a real subcommand
    /// follows. A future `Allow` command can use the bare `allow` name because
    /// the shim no longer steals it without a following known subcommand.
    pub(crate) const SUBCOMMANDS: &[&str] = &[
        "init",
        "audit",
        "check",
        "diff",
        "list",
        "explain",
        "why",
        "add",
        "propose",
        "worklist",
        "migrate",
        "refresh",
        "prune",
        "doctor",
        "vocabulary",
        "tool",
    ];
}
