use cargo_proof::{
    IdentityFrameV1, OutputFormat, ProcessExitFamilyV1, ProductIdentityV1, dry_run_from_plan_path,
    emit_frame, exit_code_for_family, exit_family_for_result_class, load_config,
    plan_from_obligation_path, render_dry_run_frame, render_plan_frame,
};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "cargo-proof",
    about = "Exact-snapshot evidence orchestration",
    version,
    propagate_version = true
)]
pub struct CargoProofCli {
    /// Repository root for proof orchestration.
    #[arg(long, default_value = ".")]
    pub root: PathBuf,
    /// Optional explicit config path (default: .allow/proof.toml).
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Human or JSON output.
    #[arg(long, value_enum, default_value_t = FormatArg::Human)]
    pub format: FormatArg,
    #[command(subcommand)]
    pub command: Option<CargoProofCommand>,
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
pub enum CargoProofCommand {
    /// Identity and capability surface.
    Identity,
    /// Plan proof execution from an intent obligation plan JSON file.
    Plan(PlanArgs),
    /// Dry-run a proof plan TOML file (structured argv only).
    DryRun(DryRunArgs),
}

#[derive(Debug, Parser)]
pub struct PlanArgs {
    /// Path to an `intent.obligation-plan.v1` JSON file.
    #[arg(long)]
    pub obligation_plan: PathBuf,
}

#[derive(Debug, Parser)]
pub struct DryRunArgs {
    /// Path to a `proof.plan.v1` TOML file.
    #[arg(long)]
    pub proof_plan: PathBuf,
}

pub fn run() -> Result<ProcessExitFamilyV1, String> {
    let cli = CargoProofCli::parse();
    let config = load_config(&cli.root, cli.config.as_deref())?;
    let output_format = OutputFormat::from(cli.format);
    let _profile = config.profile;
    match cli.command {
        None => {
            print_help()?;
            Ok(ProcessExitFamilyV1::Success)
        }
        Some(CargoProofCommand::Identity) => {
            cmd_identity(output_format)?;
            Ok(ProcessExitFamilyV1::Success)
        }
        Some(CargoProofCommand::Plan(args)) => {
            let outcome = match plan_from_obligation_path(&args.obligation_plan) {
                Ok(outcome) => outcome,
                Err(plan_error) => {
                    // Map the exit family from the proof-corpus result
                    // class instead of treating every plan failure as
                    // usage (#3598 exit-family follow-up).
                    eprintln!("error: {}", plan_error.message);
                    return Ok(exit_family_for_result_class(
                        plan_error.result_state.as_str(),
                    ));
                }
            };
            let rendered = render_plan_frame(&outcome, output_format)?;
            print!("{rendered}");
            Ok(ProcessExitFamilyV1::Success)
        }
        Some(CargoProofCommand::DryRun(args)) => {
            let report = dry_run_from_plan_path(&args.proof_plan)?;
            let rendered = render_dry_run_frame(&report, output_format)?;
            print!("{rendered}");
            Ok(ProcessExitFamilyV1::Success)
        }
    }
}

fn cmd_identity(format: OutputFormat) -> Result<(), String> {
    let identity = ProductIdentityV1::current(env!("CARGO_PKG_VERSION"));
    let frame = IdentityFrameV1::from_identity(&identity);
    let rendered = emit_frame(&frame, format)?;
    print!("{rendered}");
    Ok(())
}

fn print_help() -> Result<(), String> {
    use clap::CommandFactory;
    CargoProofCli::command()
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
