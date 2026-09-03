use cargo_proof::{
    IdentityFrameV1, OutputFormat, ProcessExitFamilyV1, ProductIdentityV1,
    captured_receipt_inputs_from_paths, dry_run_from_plan_path, emit_frame, exit_code_for_family,
    exit_family_for_result_class, explain_receipt_item, load_config, plan_from_obligation_path,
    plan_v2_from_paths, plan_v2_from_selected_registry, receipt_validation_satisfies_plan,
    reconcile_receipts, render_captured_receipt_status, render_captured_receipt_validation,
    render_dry_run_frame, render_plan_v2_frame, render_receipt_explain, render_receipt_reconcile,
};
use clap::{Parser, Subcommand, ValueEnum};
use proof_protocol::ProofItemDispositionV1;
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
    /// Proof-item ID or intent-obligation ID required by the explain action.
    #[arg(long)]
    pub item: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ReceiptAction {
    Validate,
    Status,
    Explain,
    Reconcile,
}

pub fn run() -> Result<ProcessExitFamilyV1, String> {
    let cli = CargoProofCli::parse();
    run_cli(cli)
}

fn run_cli(cli: CargoProofCli) -> Result<ProcessExitFamilyV1, String> {
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
            if args.provider_catalog.is_some()
                && (args.receipt_inventory.is_none() || args.output.is_none())
            {
                eprintln!(
                    "error: --provider-catalog, --receipt-inventory, and --output must be supplied together"
                );
                return Ok(ProcessExitFamilyV1::Usage);
            }
            if args.receipt_inventory.is_none() || args.output.is_none() {
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
            let (Some(receipt_inventory), Some(output)) =
                (args.receipt_inventory.as_ref(), args.output.as_ref())
            else {
                eprintln!("error: incomplete V2 plan inputs");
                return Ok(ProcessExitFamilyV1::Usage);
            };
            let plan_result = match args.provider_catalog.as_ref() {
                Some(provider_catalog) => plan_v2_from_paths(
                    &args.obligation_plan,
                    provider_catalog,
                    receipt_inventory,
                    output,
                ),
                None => {
                    plan_v2_from_selected_registry(&args.obligation_plan, receipt_inventory, output)
                }
            };
            let outcome = match plan_result {
                Ok(outcome) => {
                    if outcome.plan.items.iter().any(|item| {
                        item.blocking
                            && item.disposition == ProofItemDispositionV1::ProviderUnavailable
                    }) {
                        eprintln!(
                            "error: proof plan contains a blocking provider_unavailable item"
                        );
                        return Ok(exit_family_for_result_class("provider_unavailable"));
                    }
                    outcome
                }
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
        Some(CargoProofCommand::Receipts(args)) => run_receipts(args, output_format),
    }
}

fn run_receipts(
    args: ReceiptsArgs,
    output_format: OutputFormat,
) -> Result<ProcessExitFamilyV1, String> {
    let inputs = match captured_receipt_inputs_from_paths(&args.plan, &args.receipts) {
        Ok(inputs) => inputs,
        Err(error) => {
            eprintln!("error: {}", error.message());
            return Ok(error.family());
        }
    };
    let report = inputs.report;
    let plan = inputs.plan;
    let registry = match cargo_proof::StaticProviderRegistryV1::selected() {
        Ok(registry) => registry,
        Err(error) => {
            eprintln!("error: provider registry: {}", error.as_str());
            return Ok(ProcessExitFamilyV1::InstrumentFailure);
        }
    };
    let rendered = match args.action {
        ReceiptAction::Validate => render_captured_receipt_validation(&report, output_format)?,
        ReceiptAction::Status => render_captured_receipt_status(&report, output_format)?,
        ReceiptAction::Explain => {
            let Some(selector) = args.item.as_deref() else {
                eprintln!("error: receipts explain requires --item");
                return Ok(ProcessExitFamilyV1::Usage);
            };
            let projection = match explain_receipt_item(&plan, &report, &registry, selector) {
                Ok(projection) => projection,
                Err(error) => {
                    eprintln!("error: receipt explain: {}", error.message());
                    return Ok(match error {
                        cargo_proof::ReceiptProjectionError::MissingSelector(_)
                        | cargo_proof::ReceiptProjectionError::AmbiguousSelector(_) => {
                            ProcessExitFamilyV1::Usage
                        }
                        cargo_proof::ReceiptProjectionError::InvalidBinding(_) => {
                            ProcessExitFamilyV1::InstrumentFailure
                        }
                    });
                }
            };
            render_receipt_explain(&projection, output_format)?
        }
        ReceiptAction::Reconcile => {
            let projection = match reconcile_receipts(&plan, &report, &registry) {
                Ok(projection) => projection,
                Err(error) => {
                    eprintln!("error: receipt reconcile: {}", error.message());
                    return Ok(ProcessExitFamilyV1::InstrumentFailure);
                }
            };
            render_receipt_reconcile(&projection, output_format)?
        }
    };
    print!("{rendered}");
    if matches!(args.action, ReceiptAction::Validate) && !receipt_validation_satisfies_plan(&report)
    {
        Ok(ProcessExitFamilyV1::Blocking)
    } else {
        Ok(ProcessExitFamilyV1::Success)
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
    use super::{
        CargoProofCli, CargoProofCommand, FormatArg, PlanArgs, ReceiptAction, ReceiptsArgs,
        run_cli, run_receipts,
    };
    use cargo_proof::{ProcessExitFamilyV1, ReceiptCommandError};
    use effortless_repo_protocol::{
        AnalysisReceiptEnvelopeV1, ClaimBoundaryV1, RepositorySnapshotV1, ResolvedRevisionV1,
        ResultClassV1,
    };
    use intent_protocol::{
        IntentArtifactKindV1, IntentIdentityEnvelopeV1, IntentObligationPlanEnvelopeV1,
    };
    use proof_engine::CapturedReceiptStoreV1;
    use proof_protocol::{
        CapturedReceiptManifestRowV1, CapturedReceiptManifestV1, ExpectedReceiptContractV1,
        ProofItemDispositionV1, ProofItemExecutionPostureV1, ProofItemV1, ProofPlanV2,
        ProofSubjectClassV1, ProofSubjectV1, ProviderSelectionV1,
    };
    use std::path::PathBuf;

    #[test]
    fn plan_cli_requires_receipt_and_output_for_registry_route() -> Result<(), String> {
        let cli = CargoProofCli {
            root: PathBuf::from("."),
            config: None,
            format: FormatArg::Json,
            command: Some(CargoProofCommand::Plan(PlanArgs {
                obligation_plan: PathBuf::from("missing-obligation.json"),
                provider_catalog: None,
                receipt_inventory: Some(PathBuf::from("receipts.json")),
                output: None,
            })),
        };
        if run_cli(cli).map_err(|error| error.to_string())? != ProcessExitFamilyV1::Usage {
            return Err("partial registry inputs must be usage errors".to_string());
        }
        Ok(())
    }

    #[test]
    fn plan_cli_rejects_partial_explicit_catalog() -> Result<(), String> {
        let cli = CargoProofCli {
            root: PathBuf::from("."),
            config: None,
            format: FormatArg::Json,
            command: Some(CargoProofCommand::Plan(PlanArgs {
                obligation_plan: PathBuf::from("missing-obligation.json"),
                provider_catalog: Some(PathBuf::from("catalog.json")),
                receipt_inventory: Some(PathBuf::from("receipts.json")),
                output: None,
            })),
        };
        if run_cli(cli).map_err(|error| error.to_string())? != ProcessExitFamilyV1::Usage {
            return Err("partial explicit catalog inputs must be usage errors".to_string());
        }
        Ok(())
    }

    #[test]
    fn plan_cli_exercises_legacy_and_explicit_registry_routes() -> Result<(), String> {
        let directory =
            std::env::temp_dir().join(format!("cargo-proof-cli-{}", std::process::id()));
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let obligation = directory.join("obligation.json");
        let receipts = directory.join("receipts.json");
        let output = directory.join("plan.json");
        let envelope = IntentObligationPlanEnvelopeV1::new(
            IntentIdentityEnvelopeV1::new(
                RepositorySnapshotV1::new_committed_head(
                    "test",
                    "sha1",
                    ResolvedRevisionV1 {
                        requested: "HEAD".to_string(),
                        commit: "abc".to_string(),
                        tree: String::new(),
                    },
                ),
                IntentArtifactKindV1::RequirementDocument,
                "test-artifact",
                "test/source.md",
                "test-content",
            ),
            "precommit",
            vec![],
        );
        std::fs::write(
            &obligation,
            serde_json::to_vec(&envelope).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(
            &receipts,
            serde_json::to_vec(&CapturedReceiptStoreV1::new())
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;

        let base = |provider_catalog: Option<PathBuf>,
                    receipt_inventory: Option<PathBuf>,
                    output: Option<PathBuf>| {
            CargoProofCli {
                root: PathBuf::from("."),
                config: None,
                format: FormatArg::Json,
                command: Some(CargoProofCommand::Plan(PlanArgs {
                    obligation_plan: obligation.clone(),
                    provider_catalog,
                    receipt_inventory,
                    output,
                })),
            }
        };
        if run_cli(base(None, None, None))? != ProcessExitFamilyV1::InstrumentFailure {
            return Err("legacy plan should preserve provider-unavailable posture".to_string());
        }
        let catalog = directory.join("catalog.json");
        std::fs::write(&catalog, b"[]").map_err(|error| error.to_string())?;
        if run_cli(base(
            Some(catalog),
            Some(receipts.clone()),
            Some(output.clone()),
        ))? != ProcessExitFamilyV1::InstrumentFailure
        {
            return Err("explicit catalog route should reach planning".to_string());
        }
        let selected = run_cli(base(None, Some(receipts), Some(output)))?;
        if selected != ProcessExitFamilyV1::InstrumentFailure {
            return Err("selected registry route should reach planning".to_string());
        }
        let missing = CargoProofCli {
            root: PathBuf::from("."),
            config: None,
            format: FormatArg::Json,
            command: Some(CargoProofCommand::Plan(PlanArgs {
                obligation_plan: directory.join("missing-obligation.json"),
                provider_catalog: None,
                receipt_inventory: None,
                output: None,
            })),
        };
        if run_cli(missing)? != ProcessExitFamilyV1::Usage {
            return Err("missing legacy obligation should be usage".to_string());
        }
        std::fs::remove_dir_all(&directory).map_err(|error| error.to_string())?;
        Ok(())
    }

    fn receipt_cli_fixture() -> (ProofPlanV2, CapturedReceiptManifestV1) {
        let snapshot = RepositorySnapshotV1::new_committed_head(
            "snapshot-1",
            "sha",
            ResolvedRevisionV1 {
                requested: "HEAD".to_string(),
                commit: "commit".to_string(),
                tree: "tree".to_string(),
            },
        );
        let snapshot_identity = effortless_repo_protocol::stable_digest_json(&snapshot)
            .unwrap_or_else(|_| "invalid-snapshot".to_string());
        let plan = ProofPlanV2::new(
            "plan-1",
            "intent-1",
            snapshot_identity.clone(),
            vec![ProofItemV1 {
                proof_item_id: "item-1".to_string(),
                intent_obligation_id: "obligation-1".to_string(),
                phase: "precommit".to_string(),
                blocking: true,
                evidence_purpose_ref: "purpose".to_string(),
                required_capability_class: "capability-1".to_string(),
                snapshot_identity: snapshot_identity.clone(),
                subject: ProofSubjectV1 {
                    subject_class: ProofSubjectClassV1::Commit,
                    revision: Some(snapshot_identity.clone()),
                    selector: None,
                    body_identity: None,
                    limitations: Vec::new(),
                },
                disposition: ProofItemDispositionV1::SelectedForExecution,
                selection: Some(ProviderSelectionV1 {
                    provider_id: "provider-1".to_string(),
                    capability_id: "capability-1".to_string(),
                    request_digest: "request-1".to_string(),
                }),
                current_receipt: None,
                expected_receipt: Some(ExpectedReceiptContractV1 {
                    receipt_schema: effortless_repo_protocol::ANALYSIS_RECEIPT_SCHEMA_ID
                        .to_string(),
                    receipt_generation: 1,
                    config_identity: "config:test".to_string(),
                    currentness_dimensions: vec![
                        "snapshot_identity".to_string(),
                        "subject".to_string(),
                        "provider_request".to_string(),
                        "config".to_string(),
                    ],
                }),
                execution_posture: ProofItemExecutionPostureV1::Execute,
                dependency_group: None,
                limitations: Vec::new(),
                claim_boundary: "test".to_string(),
            }],
        );
        let receipt = AnalysisReceiptEnvelopeV1::new(
            "provider-1",
            snapshot,
            ResultClassV1::Findings,
            "provider.payload.v1",
            serde_json::json!({"payload": true}),
            ClaimBoundaryV1::new("captured"),
        );
        let manifest = CapturedReceiptManifestV1::new(
            "plan-1",
            vec![CapturedReceiptManifestRowV1 {
                proof_item_id: "item-1".to_string(),
                plan_id: "plan-1".to_string(),
                provider_id: "provider-1".to_string(),
                capability_id: "capability-1".to_string(),
                snapshot_identity,
                subject_identity: "invalid-subject".to_string(),
                provider_request_identity: "request-1".to_string(),
                config_identity: "config:test".to_string(),
                receipt_generation: 1,
                receipt,
            }],
        );
        (plan, manifest)
    }

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

    #[test]
    fn receipt_command_maps_read_and_parse_failures_without_execution() -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("cargo-proof-cli-receipts-{}", std::process::id()));
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let missing = run_receipts(
            ReceiptsArgs {
                plan: root.join("missing-plan.json"),
                receipts: root.join("missing-receipts.json"),
                action: ReceiptAction::Status,
                item: None,
            },
            cargo_proof::OutputFormat::Human,
        )?;
        if missing != ProcessExitFamilyV1::Usage {
            return Err("missing receipt input must map to usage".to_string());
        }

        let plan = root.join("plan.json");
        let receipts = root.join("receipts.json");
        std::fs::write(&plan, b"{}").map_err(|error| error.to_string())?;
        std::fs::write(&receipts, b"{}").map_err(|error| error.to_string())?;
        let malformed = run_receipts(
            ReceiptsArgs {
                plan: PathBuf::from(&plan),
                receipts: PathBuf::from(&receipts),
                action: ReceiptAction::Validate,
                item: None,
            },
            cargo_proof::OutputFormat::Json,
        )?;
        if malformed != ProcessExitFamilyV1::InstrumentFailure {
            return Err("malformed receipt input must map to instrument failure".to_string());
        }
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn receipt_command_projects_status_and_validation_exit_families() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "cargo-proof-cli-receipts-success-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let plan_path = root.join("plan.json");
        let manifest_path = root.join("manifest.json");
        let (plan, manifest) = receipt_cli_fixture();
        std::fs::write(
            &plan_path,
            serde_json::to_vec(&plan).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(
            &manifest_path,
            serde_json::to_vec(&manifest).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let status = run_receipts(
            ReceiptsArgs {
                plan: plan_path.clone(),
                receipts: manifest_path.clone(),
                action: ReceiptAction::Status,
                item: None,
            },
            cargo_proof::OutputFormat::Human,
        )?;
        if status != ProcessExitFamilyV1::Success {
            return Err("status projection did not return success".to_string());
        }
        let validation = run_receipts(
            ReceiptsArgs {
                plan: plan_path.clone(),
                receipts: manifest_path.clone(),
                action: ReceiptAction::Validate,
                item: None,
            },
            cargo_proof::OutputFormat::Json,
        )?;
        if validation != ProcessExitFamilyV1::Blocking {
            return Err("non-satisfying validation did not return blocking".to_string());
        }
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn receipt_command_projects_explain_and_reconcile_without_execution() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "cargo-proof-cli-receipts-projections-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let plan_path = root.join("plan.json");
        let manifest_path = root.join("manifest.json");
        let sentinel_path = root.join("source-sentinel.txt");
        let (plan, manifest) = receipt_cli_fixture();
        std::fs::write(
            &plan_path,
            serde_json::to_vec(&plan).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(&sentinel_path, b"must remain unchanged")
            .map_err(|error| error.to_string())?;
        std::fs::write(
            &manifest_path,
            serde_json::to_vec(&manifest).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let explain = run_receipts(
            ReceiptsArgs {
                plan: plan_path.clone(),
                receipts: manifest_path.clone(),
                action: ReceiptAction::Explain,
                item: Some("item-1".to_string()),
            },
            cargo_proof::OutputFormat::Json,
        )?;
        if explain != ProcessExitFamilyV1::Success {
            return Err("explain projection did not return success".to_string());
        }
        let explain_human = run_receipts(
            ReceiptsArgs {
                plan: plan_path.clone(),
                receipts: manifest_path.clone(),
                action: ReceiptAction::Explain,
                item: Some("item-1".to_string()),
            },
            cargo_proof::OutputFormat::Human,
        )?;
        if explain_human != ProcessExitFamilyV1::Success {
            return Err("human explain projection did not return success".to_string());
        }
        let reconcile = run_receipts(
            ReceiptsArgs {
                plan: plan_path.clone(),
                receipts: manifest_path.clone(),
                action: ReceiptAction::Reconcile,
                item: None,
            },
            cargo_proof::OutputFormat::Human,
        )?;
        if reconcile != ProcessExitFamilyV1::Success {
            return Err("reconcile projection did not return success".to_string());
        }
        let reconcile_json = run_receipts(
            ReceiptsArgs {
                plan: plan_path.clone(),
                receipts: manifest_path.clone(),
                action: ReceiptAction::Reconcile,
                item: None,
            },
            cargo_proof::OutputFormat::Json,
        )?;
        if reconcile_json != ProcessExitFamilyV1::Success {
            return Err("json reconcile projection did not return success".to_string());
        }
        if std::fs::read_to_string(&sentinel_path).map_err(|error| error.to_string())?
            != "must remain unchanged"
        {
            return Err("projection changed the source sentinel".to_string());
        }
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn receipt_projection_errors_keep_usage_and_instrument_families() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "cargo-proof-cli-receipts-errors-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let plan_path = root.join("plan.json");
        let manifest_path = root.join("manifest.json");
        let (plan, manifest) = receipt_cli_fixture();
        std::fs::write(
            &plan_path,
            serde_json::to_vec(&plan).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(
            &manifest_path,
            serde_json::to_vec(&manifest).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let missing_item = run_receipts(
            ReceiptsArgs {
                plan: plan_path.clone(),
                receipts: manifest_path.clone(),
                action: ReceiptAction::Explain,
                item: None,
            },
            cargo_proof::OutputFormat::Human,
        )?;
        if missing_item != ProcessExitFamilyV1::Usage {
            return Err("explain without a selector must map to usage".to_string());
        }
        let unknown_item = run_receipts(
            ReceiptsArgs {
                plan: plan_path.clone(),
                receipts: manifest_path.clone(),
                action: ReceiptAction::Explain,
                item: Some("unknown".to_string()),
            },
            cargo_proof::OutputFormat::Json,
        )?;
        if unknown_item != ProcessExitFamilyV1::Usage {
            return Err("unknown explain selector must map to usage".to_string());
        }
        let mut invalid_plan = plan;
        invalid_plan.schema_id = "wrong.schema".to_string();
        std::fs::write(
            &plan_path,
            serde_json::to_vec(&invalid_plan).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let invalid_binding = run_receipts(
            ReceiptsArgs {
                plan: plan_path,
                receipts: manifest_path,
                action: ReceiptAction::Reconcile,
                item: None,
            },
            cargo_proof::OutputFormat::Json,
        )?;
        if invalid_binding != ProcessExitFamilyV1::InstrumentFailure {
            return Err("invalid plan binding must map to instrument failure".to_string());
        }
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }
}
