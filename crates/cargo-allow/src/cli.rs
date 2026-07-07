use allow_core::{CargoAllowError, CargoAllowResult};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use std::env;

use crate::{
    add, audit, check, diff, doctor, explain, init, list, migrate, propose, prune, refresh,
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
    if let Some(index) = leading_cargo_allow_token_index(&args) {
        args.remove(index);
    }
    args
}

fn leading_cargo_allow_token_index(args: &[String]) -> Option<usize> {
    for (index, arg) in args.iter().enumerate().skip(1) {
        if arg == "allow" {
            return Some(index);
        }
        if CargoAllowCommand::SUBCOMMANDS.contains(&arg.as_str()) {
            return None;
        }
    }
    None
}

impl CargoAllowCommand {
    const SUBCOMMANDS: &[&str] = &[
        "init", "audit", "check", "diff", "list", "explain", "add", "propose", "worklist",
        "migrate", "refresh", "prune", "doctor",
    ];
}
