use allow_core::{CargoAllowError, CargoAllowResult};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use std::env;

use crate::{
    add, audit, check, diff, doctor, explain, init, list, migrate, propose, prune, refresh, why,
    worklist,
};

#[derive(Debug, Parser)]
#[command(
    name = "cargo-allow",
    about = "Source exception ledger for source trees",
    version
)]
pub(crate) struct CargoAllowCli {
    /// Accept cargo-style color preference before the subcommand.
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto)]
    pub(crate) color: ColorChoice,
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
    /// Convert compatible legacy policy files.
    Migrate(migrate::MigrateArgs),
    /// Record operator-approved advisory drift refresh for one allow entry.
    Refresh(refresh::RefreshArgs),
    /// Preview or remove stale allow entries.
    Prune(prune::PruneArgs),
    /// Validate local setup.
    Doctor(doctor::DoctorArgs),
}

pub(crate) fn run() -> CargoAllowResult<()> {
    let cli = CargoAllowCli::parse_from(normalized_args(env::args()));
    let _color = cli.color;
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
    }
}

pub(crate) fn normalized_args(args: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut args = args.into_iter().collect::<Vec<_>>();
    if let Some(index) = leading_cargo_allow_shim_index(&args) {
        args.remove(index);
    }
    args
}

/// Locate the cargo-plugin `allow` shim token (`cargo allow <subcommand>`).
///
/// Cargo invokes this binary as `cargo-allow` and inserts a literal `allow`
/// before the real subcommand. Strip that shim only when a known subcommand
/// follows (flags may appear in between). Bare `cargo-allow allow …` keeps
/// `allow` so a future `Allow` subcommand is not permanently reserved.
fn leading_cargo_allow_shim_index(args: &[String]) -> Option<usize> {
    for (index, arg) in args.iter().enumerate().skip(1) {
        if arg == "allow" {
            return if has_known_subcommand_after(args, index + 1) {
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

fn has_known_subcommand_after(args: &[String], start: usize) -> bool {
    for arg in args.iter().skip(start) {
        if is_known_subcommand(arg) {
            return true;
        }
        if arg.starts_with('-') {
            continue;
        }
        // A non-flag token that is not a known subcommand means `allow` is
        // itself the command (or an unknown command), not a cargo shim.
        return false;
    }
    false
}

fn is_known_subcommand(arg: &str) -> bool {
    CargoAllowCommand::SUBCOMMANDS.contains(&arg)
}

impl CargoAllowCommand {
    /// Installed subcommand names. Do not add `"allow"` here: that token is the
    /// cargo-plugin shim stripped by [`normalized_args`] when a real subcommand
    /// follows. A future `Allow` command can use the bare `allow` name because
    /// the shim no longer steals it without a following known subcommand.
    pub(crate) const SUBCOMMANDS: &[&str] = &[
        "init", "audit", "check", "diff", "list", "explain", "why", "add", "propose", "worklist",
        "migrate", "refresh", "prune", "doctor",
    ];
}
