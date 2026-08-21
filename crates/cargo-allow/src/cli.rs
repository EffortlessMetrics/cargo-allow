use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use std::env;
use std::ffi::OsStr;
use std::path::PathBuf;

use crate::{
    add, adoption, audit, capabilities, changie, check, completions, diff, doctor, explain,
    extraction_parity_command, hooks, init, list, migrate, precommit_tool, propose, prune,
    reference, refresh, vocabulary, why, worklist,
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
    /// receipts) and `--output` files are never styled.
    ///
    /// Currently honored by `init`, `adopt`, `check`, `audit`, `list`, `explain`, `diff`,
    /// `why`, `doctor`, `propose`, `worklist`, `refresh`, `prune`, `add`,
    /// `migrate`, `tool`, and `vocabulary` human reports. Completion scripts
    /// and other machine-oriented output remain plain.
    ///
    /// Precedence: explicit flag > NO_COLOR > CLICOLOR_FORCE >
    /// CARGO_TERM_COLOR=never > terminal capability. CARGO_TERM_COLOR can
    /// only disable styling, never enable it: CI sets it for cargo's own
    /// logs, not for this tool's reports.
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto, global = true)]
    pub(crate) color: ColorChoice,
    /// Suppress non-essential output (claim boundary, matched inventory,
    /// non-matched advisory outcomes). Show only result + counts.
    #[arg(short = 'q', long, global = true)]
    pub(crate) quiet: bool,
    /// Write the versioned common command summary to a separate JSON file.
    ///
    /// Supports the source-exception `adopt`, `doctor`, `audit`, `check`, `diff`,
    /// `init`, `explain`, `why`, and `worklist` commands. Existing detailed human,
    /// JSON, Markdown, HTML, SARIF, and receipt artifacts remain unchanged.
    ///
    /// Deliberately *not* named `--summary-output`: `add`, `propose`, and
    /// `migrate` each own a per-command `--summary-output` with different
    /// semantics, and a global flag of that name shadows them.
    #[arg(long, global = true, value_name = "PATH")]
    pub(crate) command_summary_output: Option<PathBuf>,
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
    /// Recommend one bounded, read-only adoption step.
    Adopt(adoption::AdoptionArgs),
    /// Inventory exceptions and policy health.
    Audit(audit::ReportArgs),
    /// CI gate for the exception ledger.
    Check(check::CheckArgs),
    /// Show the source-tree sensor capability and claim matrix.
    Capabilities(capabilities::CapabilitiesArgs),
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
    /// Generate a deterministic command reference with checked support metadata.
    Reference(reference::ReferenceArgs),
    /// Preview checked local hook plans without changing the repository.
    Hooks(hooks::HooksArgs),
    /// Emit runtime parity evidence for extraction stages.
    ExtractionParity(extraction_parity_command::ParityArgs),
    /// Run the Rust-native Changie static sensor over one exact source subject.
    Changie(changie::ChangieArgs),
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
        if cli.command_summary_output.is_some() {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                "--command-summary-output requires the adopt, doctor, audit, check, diff, init, propose, add, refresh, prune, explain, why, or worklist subcommand",
            ));
        }
        CargoAllowCli::command().print_help().map_err(|e| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Artifact,
                format!("failed to print help: {e}"),
            )
        })?;
        println!();
        return Ok(());
    };
    configure_summary_output(cli.command_summary_output, &command)?;
    match command {
        CargoAllowCommand::Init(args) => init::cmd_init(&args),
        CargoAllowCommand::Adopt(args) => adoption::cmd_adopt(&args),
        CargoAllowCommand::Audit(args) => audit::cmd_audit(&args),
        CargoAllowCommand::Check(args) => check::cmd_check(&args),
        CargoAllowCommand::Capabilities(args) => capabilities::cmd_capabilities(&args),
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
        CargoAllowCommand::Reference(args) => reference::cmd_reference(&args),
        CargoAllowCommand::Hooks(args) => hooks::cmd_hooks(&args),
        CargoAllowCommand::ExtractionParity(args) => extraction_parity_command::cmd_parity(&args),

        CargoAllowCommand::Changie(args) => changie::cmd_changie(&args.clone()),
    }
}

fn configure_summary_output(
    summary_output: Option<PathBuf>,
    command: &CargoAllowCommand,
) -> CargoAllowResult<()> {
    let Some(path) = summary_output else {
        return Ok(());
    };
    use crate::core_command_router::ConflictBase::{SourceTreeRoot, WorkingDirectory};
    // `--config` is discovered under the source-tree root for every command, and
    // `adopt` resolves `--output` under the root as well. The rest write through
    // `emit_text`/`write_file`, which are relative to the working directory.
    let conflicts: Vec<_> = match command {
        CargoAllowCommand::Audit(args) if args.profile.is_none() => vec![
            (WorkingDirectory, args.output.clone()),
            (SourceTreeRoot, args.config.clone()),
        ],
        CargoAllowCommand::Check(args) if args.profile.is_none() && !args.staged_identity_only => {
            vec![
                (WorkingDirectory, args.output.clone()),
                (WorkingDirectory, args.receipt.clone()),
                (SourceTreeRoot, args.config.clone()),
            ]
        }
        CargoAllowCommand::Adopt(args) => vec![
            (SourceTreeRoot, args.output.clone()),
            (SourceTreeRoot, args.config.clone()),
        ],
        CargoAllowCommand::Doctor(args) if args.profile.is_none() => vec![
            (WorkingDirectory, args.output.clone()),
            (SourceTreeRoot, args.config.clone()),
            (WorkingDirectory, args.support_bundle.clone()),
        ],
        CargoAllowCommand::Explain(args) if args.profile.is_none() => vec![
            (WorkingDirectory, args.output.clone()),
            (SourceTreeRoot, args.config.clone()),
        ],
        // `why --plan` writes a candidate add-finding plan, so the summary
        // sidecar must not be allowed to overwrite it.
        CargoAllowCommand::Why(args) => vec![
            (WorkingDirectory, args.output.clone()),
            (SourceTreeRoot, args.config.clone()),
            (WorkingDirectory, args.plan.clone()),
        ],
        CargoAllowCommand::Worklist(args) if args.profile.is_none() => vec![
            (WorkingDirectory, args.output.clone()),
            (SourceTreeRoot, args.config.clone()),
        ],
        CargoAllowCommand::Diff(args) => vec![
            (WorkingDirectory, args.output.clone()),
            (WorkingDirectory, args.receipt.clone()),
            (SourceTreeRoot, args.config.clone()),
        ],
        CargoAllowCommand::Init(args) if args.profile.is_none() => {
            vec![(SourceTreeRoot, Some(args.config.clone()))]
        }
        CargoAllowCommand::Propose(args) => vec![
            (WorkingDirectory, args.write.clone()),
            (WorkingDirectory, args.summary_output.clone()),
            (SourceTreeRoot, args.config.clone()),
        ],
        CargoAllowCommand::Add(args) => vec![
            (WorkingDirectory, args.write.clone()),
            (WorkingDirectory, args.from_plan.clone()),
            (WorkingDirectory, args.summary_output.clone()),
            (SourceTreeRoot, args.config.clone()),
        ],
        CargoAllowCommand::Refresh(args) => vec![
            (WorkingDirectory, args.output.clone()),
            (SourceTreeRoot, args.config.clone()),
        ],
        CargoAllowCommand::Prune(args) => vec![
            (WorkingDirectory, args.output.clone()),
            (SourceTreeRoot, args.config.clone()),
        ],
        CargoAllowCommand::Migrate(args) => vec![
            (SourceTreeRoot, Some(args.out.clone())),
            (WorkingDirectory, args.summary_output.clone()),
        ],
        _ => {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                "--command-summary-output currently supports the source-exception adopt, doctor, audit, check, diff, init, propose, add, refresh, prune, migrate, explain, why, and worklist commands only",
            ));
        }
    }
    .into_iter()
    .filter_map(|(base, path)| path.map(|path| (base, path)))
    .collect();
    crate::core_command_router::configure_summary_output(
        crate::core_command_router::SummaryOutputConfig::new(path, conflicts),
    )
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
        "adopt",
        "audit",
        "check",
        "capabilities",
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
        "completions",
        "reference",
        "hooks",
        "extraction-parity",
        "changie",
    ];
}
