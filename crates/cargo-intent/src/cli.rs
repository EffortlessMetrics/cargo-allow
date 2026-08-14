use cargo_intent::{
    IdentityFrameV1, IntentConfigV1, OutputFormat, ProcessExitFamilyV1, ProductIdentityV1,
    change_status_staged_precommit, emit_frame, exit_code_for_family, load_config,
};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "cargo-intent",
    about = "Durable authored intent and obligation compiler",
    version,
    propagate_version = true
)]
pub struct CargoIntentCli {
    /// Repository root for intent evaluation.
    #[arg(long, default_value = ".")]
    pub root: PathBuf,
    /// Optional explicit config path (default: .allow/intent.toml).
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Human or JSON output.
    #[arg(long, value_enum, default_value_t = FormatArg::Human)]
    pub format: FormatArg,
    #[command(subcommand)]
    pub command: Option<CargoIntentCommand>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FormatArg {
    Human,
    Json,
}

impl From<FormatArg> for OutputFormat {
    fn from(value: FormatArg) -> Self {
        match value {
            FormatArg::Human => Self::Human,
            FormatArg::Json => Self::Json,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum CargoIntentCommand {
    /// Identity and capability surface.
    Identity,
    /// Change-oriented intent commands.
    Change(ChangeCli),
    /// Compile the governance authority into a validation receipt (#2942 step 4).
    Governance(GovernanceArgs),
}

#[derive(Debug, Parser)]
pub struct GovernanceArgs {
    /// Write the receipt JSON to this path in addition to stdout.
    #[arg(long)]
    pub receipt: Option<PathBuf>,
}

#[derive(Debug, Parser)]
pub struct ChangeCli {
    #[command(subcommand)]
    pub command: ChangeCommand,
}

#[derive(Debug, Subcommand)]
pub enum ChangeCommand {
    /// Staged change status for a lifecycle phase.
    Status(StatusArgs),
}

#[derive(Debug, Parser)]
pub struct StatusArgs {
    /// Read staged index posture instead of committed head only.
    #[arg(long)]
    pub staged: bool,
    /// Lifecycle phase selector.
    #[arg(long, value_enum)]
    pub phase: Option<PhaseArg>,
    /// Emit a provider-neutral `repo.analysis-receipt.v1` envelope in JSON mode.
    #[arg(long)]
    pub analysis_receipt: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PhaseArg {
    Precommit,
}

pub fn run() -> Result<ProcessExitFamilyV1, String> {
    let cli = CargoIntentCli::parse();
    let config = load_config(&cli.root, cli.config.as_deref())?;
    let output_format = OutputFormat::from(cli.format);
    match cli.command {
        None => {
            print_help()?;
            Ok(ProcessExitFamilyV1::Success)
        }
        Some(CargoIntentCommand::Identity) => {
            cmd_identity(&config, output_format)?;
            Ok(ProcessExitFamilyV1::Success)
        }
        Some(CargoIntentCommand::Change(change)) => match change.command {
            ChangeCommand::Status(args) => {
                validate_change_status_args(&args, output_format)?;
                change_status_staged_precommit(
                    &cli.root,
                    &config,
                    output_format,
                    args.analysis_receipt,
                )
            }
        },
        Some(CargoIntentCommand::Governance(args)) => {
            cmd_governance(&cli.root, &args, output_format)
        }
    }
}

fn cmd_governance(
    root: &std::path::Path,
    args: &GovernanceArgs,
    format: OutputFormat,
) -> Result<ProcessExitFamilyV1, String> {
    use cargo_intent::{GOVERNANCE_RECEIPT_SCHEMA_ID, compile_governance_receipt_at};

    let receipt = compile_governance_receipt_at(root)?;
    if let Some(path) = &args.receipt {
        let json = serde_json::to_string_pretty(&receipt)
            .map_err(|err| format!("serialize receipt: {err}"))?;
        std::fs::write(path, format!("{json}\n"))
            .map_err(|err| format!("write receipt {}: {err}", path.display()))?;
    }
    let rendered = match format {
        OutputFormat::Json => serde_json::to_string_pretty(&receipt)
            .map_err(|err| format!("serialize receipt: {err}"))?,
        OutputFormat::Human => format!(
            "governance receipt {} ({} blocking findings, {} candidate rows)\nschema: {GOVERNANCE_RECEIPT_SCHEMA_ID}\ndigest: {}\nclaim boundary: {}\n",
            receipt.result,
            receipt.blocking_finding_count,
            receipt.candidate_package_rows.len(),
            receipt.receipt_digest,
            receipt.claim_boundary
        ),
    };
    print!("{rendered}");
    Ok(if receipt.result == "passed" {
        ProcessExitFamilyV1::Success
    } else {
        ProcessExitFamilyV1::Blocking
    })
}

fn validate_change_status_args(args: &StatusArgs, format: OutputFormat) -> Result<(), String> {
    if !args.staged {
        return Err("change status requires --staged".to_string());
    }
    match args.phase {
        Some(PhaseArg::Precommit) => {}
        None => return Err("change status requires --phase precommit".to_string()),
    }
    // Analysis receipts are one exact JSON contract.
    // A human frame is never a compatible fallback.
    if args.analysis_receipt && format != OutputFormat::Json {
        return Err(
            "--analysis-receipt requires --format json; refusing to emit a different output contract"
                .to_string(),
        );
    }
    Ok(())
}

fn cmd_identity(config: &IntentConfigV1, format: OutputFormat) -> Result<(), String> {
    let identity = ProductIdentityV1::current(env!("CARGO_PKG_VERSION"));
    let frame = IdentityFrameV1::from_identity(&identity);
    let _profile = config.profile;
    let rendered = emit_frame(&frame, format)?;
    print!("{rendered}");
    Ok(())
}

fn print_help() -> Result<(), String> {
    use clap::CommandFactory;
    CargoIntentCli::command()
        .print_help()
        .map_err(|err| format!("print help: {err}"))?;
    println!();
    Ok(())
}

pub fn main_exit_code(result: Result<ProcessExitFamilyV1, String>) -> i32 {
    match result {
        Ok(family) => exit_code_for_family(family),
        Err(message) => {
            eprintln!("error: {message}");
            exit_code_for_family(ProcessExitFamilyV1::Usage)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_args() -> StatusArgs {
        StatusArgs {
            staged: true,
            phase: Some(PhaseArg::Precommit),
            analysis_receipt: false,
        }
    }

    #[test]
    fn change_status_requires_staged_and_phase() {
        let args = StatusArgs {
            staged: false,
            ..valid_args()
        };
        assert!(validate_change_status_args(&args, OutputFormat::Json).is_err());
        let args = StatusArgs {
            phase: None,
            ..valid_args()
        };
        assert!(validate_change_status_args(&args, OutputFormat::Json).is_err());
        assert!(validate_change_status_args(&valid_args(), OutputFormat::Human).is_ok());
    }

    #[test]
    fn analysis_receipt_requires_json_output() {
        let args = StatusArgs {
            analysis_receipt: true,
            ..valid_args()
        };
        assert!(validate_change_status_args(&args, OutputFormat::Human).is_err());
        assert!(validate_change_status_args(&args, OutputFormat::Json).is_ok());
    }
}
