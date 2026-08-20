use cargo_proof::{
    IdentityFrameV1, OutputFormat, ProcessExitFamilyV1, ProductIdentityV1, dry_run_from_plan_path,
    emit_frame, exit_code_for_family, exit_family_for_result_class, load_config,
    plan_from_obligation_path, plan_v2_from_paths, receipt_validation_satisfies_plan,
    render_captured_receipt_status, render_captured_receipt_validation, render_dry_run_frame,
    render_plan_v2_frame,
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
    /// Read-only deterministic selected-provider and capability projection.
    Providers,
    /// Plan proof execution from an intent obligation plan JSON file.
    Plan(PlanArgs),
    /// Dry-run a proof plan TOML file (structured argv only).
    DryRun(DryRunArgs),
    /// Read-only validation and status for captured provider receipts.
    Receipts(ReceiptsArgs),
}

#[derive(Debug, Parser)]
pub struct PlanArgs {
    /// Path to an `intent.obligation-plan.v1` JSON file.
    #[arg(long)]
    pub obligation_plan: PathBuf,
    /// JSON provider capability catalogs, ordered deterministically by the planner.
    #[arg(long)]
    pub provider_catalog: Option<PathBuf>,
    /// JSON captured-receipt inventory used only for exact plan-id reuse.
    #[arg(long)]
    pub receipt_inventory: Option<PathBuf>,
    /// Explicit JSON output artifact path.
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Parser)]
pub struct DryRunArgs {
    /// Path to the generated `proof.plan.v2` JSON artifact.
    #[arg(long)]
    pub proof_plan: PathBuf,
}

#[derive(Debug, Parser)]
pub struct ReceiptsArgs {
    /// Proof plan JSON artifact consumed as the semantic authority.
    #[arg(long)]
    pub plan: PathBuf,
    /// Captured receipt manifest JSON artifact.
    #[arg(long)]
    pub receipts: PathBuf,
    /// Print validation or status projection.
    #[arg(long, value_enum, default_value_t = ReceiptAction::Status)]
    pub action: ReceiptAction,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ReceiptAction {
    Validate,
    Status,
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
        Some(CargoProofCommand::Providers) => provider_command_family(cmd_providers(output_format)),
        Some(CargoProofCommand::Plan(args)) => {
            let supplied = usize::from(args.provider_catalog.is_some())
                + usize::from(args.receipt_inventory.is_some())
                + usize::from(args.output.is_some());
            if supplied == 1 || supplied == 2 {
                eprintln!(
                    "error: --provider-catalog, --receipt-inventory, and --output must be supplied together"
                );
                return Ok(ProcessExitFamilyV1::Usage);
            }
            if supplied == 0 {
                let plan_error = match plan_from_obligation_path(&args.obligation_plan) {
                    Ok(_) => {
                        eprintln!(
                            "error: legacy planner unexpectedly produced a plan without explicit catalog inputs"
                        );
                        return Ok(ProcessExitFamilyV1::InstrumentFailure);
                    }
                    Err(error) => error,
                };
                let family = match plan_error.result_state {
                    proof_protocol::ProofResultStateV1::Missing => ProcessExitFamilyV1::Usage,
                    state => exit_family_for_result_class(state.as_str()),
                };
                eprintln!("error: {}", plan_error.message);
                return Ok(family);
            }
            let (Some(provider_catalog), Some(receipt_inventory), Some(output)) = (
                args.provider_catalog.as_ref(),
                args.receipt_inventory.as_ref(),
                args.output.as_ref(),
            ) else {
                eprintln!("error: incomplete V2 plan inputs");
                return Ok(ProcessExitFamilyV1::Usage);
            };
            let outcome = match plan_v2_from_paths(
                &args.obligation_plan,
                provider_catalog,
                receipt_inventory,
                output,
            ) {
                Ok(outcome) => outcome,
                Err(plan_error) => {
                    // Map the exit family from the proof-corpus result
                    // class instead of treating every plan failure as
                    // usage (#3598 exit-family follow-up). Input failures
                    // (missing/unreadable envelope) stay in the usage
                    // family: they are invocation errors, not provider
                    // posture.
                    let family = match plan_error.result_state {
                        proof_protocol::ProofResultStateV1::Missing => ProcessExitFamilyV1::Usage,
                        state => exit_family_for_result_class(state.as_str()),
                    };
                    eprintln!("error: {}", plan_error.message);
                    return Ok(family);
                }
            };
            let rendered = render_plan_v2_frame(&outcome, output_format)?;
            print!("{rendered}");
            Ok(ProcessExitFamilyV1::Success)
        }
        Some(CargoProofCommand::DryRun(args)) => {
            let report = dry_run_from_plan_path(&args.proof_plan)?;
            let rendered = render_dry_run_frame(&report, output_format)?;
            print!("{rendered}");
            Ok(ProcessExitFamilyV1::Success)
        }
        Some(CargoProofCommand::Receipts(args)) => {
            let report =
                match cargo_proof::captured_receipt_status_from_paths(&args.plan, &args.receipts) {
                    Ok(report) => report,
                    Err(error) => {
                        eprintln!("error: {}", error.message());
                        return Ok(error.family());
                    }
                };
            let rendered = match args.action {
                ReceiptAction::Validate => {
                    render_captured_receipt_validation(&report, output_format)?
                }
                ReceiptAction::Status => render_captured_receipt_status(&report, output_format)?,
            };
            print!("{rendered}");
            if matches!(args.action, ReceiptAction::Validate)
                && !receipt_validation_satisfies_plan(&report)
            {
                Ok(ProcessExitFamilyV1::Blocking)
            } else {
                Ok(ProcessExitFamilyV1::Success)
            }
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

fn cmd_providers(format: OutputFormat) -> Result<(), String> {
    let registry = cargo_proof::StaticProviderRegistryV1::selected()
        .map_err(|error| format!("provider registry: {}", error.as_str()))?;
    let projections = registry.projections();
    let availability = registry.availability();
    match format {
        OutputFormat::Json => {
            let frame = serde_json::json!({
                "schema_id": cargo_proof::PROVIDER_REGISTRY_SCHEMA_ID,
                "providers": projections,
                "availability": availability,
                "claim_boundary": "Read-only selected provider/capability projection; no provider execution occurred.",
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&frame).map_err(|error| error.to_string())?
            );
        }
        OutputFormat::Human => {
            if projections.is_empty() {
                println!("providers: none selected (feature-disabled providers are unavailable)");
            } else {
                for provider in projections {
                    println!("provider {}", provider.provider_id);
                    for capability in provider.capabilities.capabilities {
                        println!("  capability {}", capability.capability_id);
                    }
                }
            }
            for entry in availability {
                if entry.disposition != cargo_proof::ProviderDispositionV1::Selected {
                    println!("provider {}: {:?}", entry.provider_id, entry.disposition);
                }
            }
        }
    }
    Ok(())
}

fn provider_command_family(result: Result<(), String>) -> Result<ProcessExitFamilyV1, String> {
    match result {
        Ok(()) => Ok(ProcessExitFamilyV1::Success),
        Err(error) => {
            eprintln!("error: provider registry: {error}");
            Ok(ProcessExitFamilyV1::InstrumentFailure)
        }
    }
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

#[cfg(test)]
mod tests {
    use super::provider_command_family;
    use cargo_proof::{ProcessExitFamilyV1, ReceiptCommandError};

    #[test]
    fn provider_registry_failure_is_internal_not_usage() -> Result<(), String> {
        if provider_command_family(Err("invalid provider surface".to_string()))?
            != ProcessExitFamilyV1::InstrumentFailure
        {
            return Err("provider registry failure must map to instrument failure".to_string());
        }
        Ok(())
    }

    #[test]
    fn receipt_failures_keep_typed_exit_families() -> Result<(), String> {
        if ReceiptCommandError::ReadManifest("missing".to_string()).family()
            != ProcessExitFamilyV1::Usage
        {
            return Err("missing receipt input must remain usage".to_string());
        }
        for error in [
            ReceiptCommandError::MalformedManifest("malformed".to_string()),
            ReceiptCommandError::InvalidReceipt("conflict".to_string()),
            ReceiptCommandError::ProviderRegistry("unavailable".to_string()),
        ] {
            if error.family() != ProcessExitFamilyV1::InstrumentFailure {
                return Err("receipt semantic failures must not map to usage".to_string());
            }
        }
        Ok(())
    }
}
