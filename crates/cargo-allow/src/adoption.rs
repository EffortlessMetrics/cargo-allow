use allow_core::{
    AllowConfig, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, MatchStatus,
    normalize_path, sha256_v1_bytes,
};
use allow_inventory::{
    Inventory, InventoryCompleteness, InventoryOptions, InventorySource, inventory,
    resolve_source_tree_root,
};
use allow_match::{CheckMode, evaluate};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[path = "adoption_args.rs"]
mod adoption_args;
pub(crate) use adoption_args::AdoptionArgs;

use crate::policy_config::discover_config_path;
use crate::{
    EvidenceReportSummary, EvidenceValidationMode, HumanJsonFormat, InventoryFacts, RootArgs,
    current_dir, emit_text, report_config,
};

const COMMAND: &str = "adopt";

#[derive(Debug, Serialize)]
struct AdoptionInventoryArtifact {
    scope: &'static str,
    scanner: &'static str,
    source: String,
    root: &'static str,
    files_scanned: usize,
    completeness: String,
    empty_git_tracked: bool,
}

#[derive(Debug, Serialize)]
struct AdoptionArtifact {
    schema_version: u32,
    schema_id: &'static str,
    tool: &'static str,
    command: &'static str,
    claim_boundary: &'static [&'static str],
    scanner_limitations: &'static [&'static str],
    inventory: AdoptionInventoryArtifact,
    plan: allow_report::CoreAdoptionPlanV1,
}

pub(crate) fn cmd_adopt(args: &AdoptionArgs) -> CargoAllowResult<()> {
    let inspection = inspect(args)?;
    let output = args
        .output
        .as_deref()
        .map(|path| resolve_output_path(&inspection.root, path));
    if let Some(output) = &output {
        validate_output_path(&inspection.root, output, inspection.policy_path.as_deref())?;
    }
    let artifact = AdoptionArtifact {
        schema_version: allow_report::CORE_ADOPTION_PLAN_SCHEMA_VERSION,
        schema_id: allow_report::CORE_ADOPTION_PLAN_SCHEMA_ID,
        tool: "cargo-allow",
        command: COMMAND,
        claim_boundary: allow_report::claim_boundary_for_schema_id(
            allow_report::CORE_ADOPTION_PLAN_SCHEMA_ID,
        ),
        scanner_limitations: allow_report::scanner_limitations_for_schema_id(
            allow_report::CORE_ADOPTION_PLAN_SCHEMA_ID,
        ),
        inventory: inventory_artifact(&inspection),
        plan: inspection.plan,
    };
    let json = serde_json::to_string_pretty(&artifact).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!("failed to render adoption plan JSON: {error}"),
        )
    })?;
    // Common operator grammar (#3149). The adoption plan remains authoritative
    // for its full semantics; this projection is additive and derived from the
    // same in-memory plan without re-inspecting the repository.
    let summary =
        crate::core_command_summary::core_command_summary_from_adoption_plan(&artifact.plan)
            .map_err(|error| {
                CargoAllowError::with_kind(
                    CargoAllowErrorKind::Internal,
                    format!("failed to build core command summary: {error}"),
                )
            })?;
    crate::core_command_router::write_summary_artifact(&inspection.root, &summary)?;

    let rendered = match args.format {
        HumanJsonFormat::Human => {
            let style = if output.is_none() {
                crate::reporting::output_style()
            } else {
                allow_report::Style::PLAIN
            };
            format!(
                "{}\n{}",
                crate::core_command_summary::render_core_command_summary_human(&summary),
                render_human(&artifact.plan, style)
            )
        }
        HumanJsonFormat::Json => json,
    };
    emit_text(output.as_deref(), &rendered)?;

    match artifact.plan.bootstrap_disposition {
        allow_report::BootstrapDisposition::PartialInventory
        | allow_report::BootstrapDisposition::InvalidPolicy
        | allow_report::BootstrapDisposition::UnsupportedRepositoryState
        | allow_report::BootstrapDisposition::InstrumentFailure => Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!(
                "cargo-allow adopt: {}",
                disposition_text(artifact.plan.bootstrap_disposition)
            ),
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
pub(crate) fn sample_adoption_json_for_contract_test() -> String {
    let facts = allow_report::AdoptionFacts {
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        repository_identity: "sha256:v1:sample-repository".to_string(),
        selected_root: "sample-root".to_string(),
        channel: "source-preview".to_string(),
        executable_identity: "sha256:v1:sample-executable".to_string(),
        inventory: allow_report::AdoptionInventoryFacts {
            mode: allow_report::InventoryMode::GitTracked,
            completeness: allow_report::InventoryCompleteness::Complete,
            limitations: Vec::new(),
        },
        policy: allow_report::AdoptionPolicyFacts {
            state: allow_report::PolicyState::Absent,
            path: None,
            schema_version: None,
            digest: None,
            total_findings: 0,
            new_unreceipted_findings: 0,
            stale_entries: 0,
            location_drift_entries: 0,
            broken_evidence_entries: 0,
            review_due_entries: 0,
            expired_entries: 0,
            occurrence_headroom_entries: 0,
            mirror_divergence: false,
        },
        policy_config_diagnostic: None,
        unsupported_repository_state: false,
        instrument_failure: None,
        strict_gate_requested: false,
        ci_guidance_completed: false,
    };
    let plan = allow_report::recommend_core_adoption_plan(&facts);
    let artifact = AdoptionArtifact {
        schema_version: allow_report::CORE_ADOPTION_PLAN_SCHEMA_VERSION,
        schema_id: allow_report::CORE_ADOPTION_PLAN_SCHEMA_ID,
        tool: "cargo-allow",
        command: COMMAND,
        claim_boundary: allow_report::claim_boundary_for_schema_id(
            allow_report::CORE_ADOPTION_PLAN_SCHEMA_ID,
        ),
        scanner_limitations: allow_report::scanner_limitations_for_schema_id(
            allow_report::CORE_ADOPTION_PLAN_SCHEMA_ID,
        ),
        inventory: AdoptionInventoryArtifact {
            scope: allow_report::INVENTORY_SCOPE_SOURCE_TREE,
            scanner: allow_report::INVENTORY_SCANNER_SOURCE_SYNTAX,
            source: "git_tracked".to_string(),
            root: "<repository-root>",
            files_scanned: 0,
            completeness: "complete".to_string(),
            empty_git_tracked: false,
        },
        plan,
    };
    match serde_json::to_string_pretty(&artifact) {
        Ok(json) => json,
        Err(error) => format!("serialization error: {error}"),
    }
}

struct Inspection {
    root: PathBuf,
    inventory: Option<Inventory>,
    inventory_facts: Option<InventoryFacts>,
    policy_path: Option<PathBuf>,
    plan: allow_report::CoreAdoptionPlanV1,
}

fn inspect(args: &AdoptionArgs) -> CargoAllowResult<Inspection> {
    let root = resolve_root(&args.root)?;
    let explicit_config = args
        .config
        .as_deref()
        .map(|config| resolve_config_path(&root, config))
        .transpose()?;
    let discovery = discover_config_path(&root, explicit_config.as_deref());
    // `discover_config_path` is the sole read-selection authority. Do not
    // rescan conventional paths here after it has classified candidates;
    // doing so could make `adopt` select a different policy than the central
    // resolver for the same repository state.
    let policy_path = explicit_config
        .or_else(|| discovery.path.clone())
        .or_else(|| {
            // Preserve the established fail-closed InvalidPolicy posture when
            // discovery observed a conventional candidate but rejected it during
            // parsing. This reuses the central candidate record rather than
            // probing the filesystem a second time; foreign candidates remain
            // absent and continue to produce the existing no-policy posture.
            discovery
                .skipped
                .iter()
                .find(|candidate| {
                    candidate.source == allow_policy::SOURCE_CONVENTIONAL_PATH
                        && (candidate.reason.contains("invalid")
                            || candidate.reason.contains("parse"))
                })
                .map(|candidate| candidate.path.clone())
        });
    let mut limitations = Vec::new();
    if !discovery.skipped.is_empty() {
        limitations.push("foreign policy candidates were skipped during discovery".to_string());
    }

    let (cfg, policy_digest, policy_state, policy_diagnostic) = match policy_path.as_deref() {
        Some(path) => {
            match crate::policy_config::load_policy_at_path_with_digest(
                path.to_path_buf(),
                EvidenceValidationMode::ReportOnly,
            ) {
                Ok((cfg, digest)) => (cfg, Some(digest), allow_report::PolicyState::Valid, None),
                Err(error) => (
                    AllowConfig::empty(),
                    None,
                    allow_report::PolicyState::Invalid,
                    Some(sanitize_diagnostic(&root, &error.to_string())),
                ),
            }
        }
        None => (
            AllowConfig::empty(),
            None,
            allow_report::PolicyState::Absent,
            None,
        ),
    };
    let options = InventoryOptions {
        ignored: cfg.workspace.ignored.clone(),
        generated: cfg.workspace.generated.clone(),
        include_untracked: args.include_untracked,
    };
    let inventory = match inventory(&root, &options) {
        Ok(inventory) => inventory,
        Err(error) => {
            let facts = adoption_facts(AdoptionFactInputs {
                root: &root,
                inventory: None,
                inventory_facts: None,
                policy_path: policy_path.as_deref(),
                cfg: &cfg,
                policy_state,
                policy_diagnostic,
                limitations,
                strict_gate_requested: args.strict,
                ci_guidance_completed: false,
                signals: None,
                instrument_failure: Some(format!(
                    "inventory failed: {}",
                    sanitize_diagnostic(&root, &error.to_string())
                )),
            })?;
            return Ok(Inspection {
                root,
                inventory: None,
                inventory_facts: None,
                policy_path,
                plan: allow_report::recommend_core_adoption_plan(&facts),
            });
        }
    };
    if inventory.git_error.is_some() {
        limitations.push("Git inventory was unavailable; a filesystem inventory was used".into());
    }
    if inventory.empty_git_tracked {
        limitations.push("Git reported no tracked files".into());
    }
    if !inventory.skipped_paths.is_empty() {
        limitations.push(format!(
            "{} inventory path(s) were skipped",
            inventory.skipped_paths.len()
        ));
    }
    if !inventory.deleted_tracked.is_empty() {
        limitations.push(format!(
            "{} tracked path(s) are missing",
            inventory.deleted_tracked.len()
        ));
    }
    if !inventory.submodule_paths.is_empty() {
        limitations.push(format!(
            "{} submodule path(s) are not recursively scanned",
            inventory.submodule_paths.len()
        ));
    }

    let scan = if policy_state == allow_report::PolicyState::Invalid {
        Ok((
            root.clone(),
            cfg.clone(),
            Vec::new(),
            InventoryFacts::scanned_inventory(&inventory),
            crate::world::default_federation_evaluation(),
        ))
    } else {
        let federation = discovery
            .federation
            .clone()
            .unwrap_or_else(crate::world::default_federation_evaluation);
        crate::world::load_world_from_resolved_policy(
            &root,
            cfg.clone(),
            policy_digest.clone(),
            federation,
            args.include_untracked,
        )
    };
    let (scan_root, scanned_cfg, findings, inventory_facts, federation, instrument_failure) =
        match scan {
            Ok((scan_root, scanned_cfg, findings, inventory_facts, federation)) => (
                scan_root,
                scanned_cfg,
                findings,
                inventory_facts,
                federation,
                None,
            ),
            Err(error) => (
                root.clone(),
                cfg.clone(),
                Vec::new(),
                InventoryFacts::scanned_inventory(&inventory),
                crate::world::default_federation_evaluation(),
                Some(format!(
                    "source inventory scan failed: {}",
                    sanitize_diagnostic(&root, &error.to_string())
                )),
            ),
        };
    let _ = scan_root;
    if inventory_facts.rust_files_skipped > 0 {
        limitations.push(format!(
            "{} Rust file(s) could not be read by the scanner",
            inventory_facts.rust_files_skipped
        ));
    }
    if inventory_facts.rust_files_with_parse_errors > 0 {
        limitations.push(format!(
            "{} Rust file(s) contained parse errors",
            inventory_facts.rust_files_with_parse_errors
        ));
    }

    let report_cfg = report_config(&scanned_cfg, None)?;
    let outcomes = evaluate(&report_cfg, &findings, CheckMode::Audit);
    let evidence_files = crate::evidence_inventory::current_evidence_source_tree_files(
        &root,
        args.include_untracked,
    );
    let evidence = EvidenceReportSummary::from_policy_with_source_tree_files(
        &root,
        &report_cfg,
        &outcomes,
        evidence_files.as_ref(),
    );
    let mut policy_diagnostic = policy_diagnostic;
    let invalid_match_count = outcomes
        .iter()
        .filter(|outcome| {
            matches!(
                outcome.status,
                MatchStatus::Ambiguous
                    | MatchStatus::InvalidSelector
                    | MatchStatus::MissingRequiredField
            )
        })
        .count();
    if invalid_match_count > 0 && policy_diagnostic.is_none() {
        policy_diagnostic = Some(format!(
            "{invalid_match_count} policy match outcome(s) require policy repair"
        ));
    }
    let facts = adoption_facts(AdoptionFactInputs {
        root: &root,
        inventory: Some(&inventory),
        inventory_facts: Some(&inventory_facts),
        policy_path: policy_path.as_deref(),
        cfg: &scanned_cfg,
        policy_state,
        policy_diagnostic,
        limitations,
        strict_gate_requested: args.strict,
        ci_guidance_completed: ci_guidance_completed(&root, &inventory.files),
        signals: Some(PolicySignals {
            outcomes: &outcomes,
            evidence,
            mirror_divergence: !federation.divergences.is_empty(),
            digest: policy_digest,
        }),
        instrument_failure,
    })?;
    Ok(Inspection {
        root,
        inventory: Some(inventory),
        inventory_facts: Some(inventory_facts),
        policy_path,
        plan: allow_report::recommend_core_adoption_plan(&facts),
    })
}

struct PolicySignals<'a> {
    outcomes: &'a [allow_core::MatchOutcome],
    evidence: EvidenceReportSummary,
    mirror_divergence: bool,
    digest: Option<String>,
}

struct AdoptionFactInputs<'a> {
    root: &'a Path,
    inventory: Option<&'a Inventory>,
    inventory_facts: Option<&'a InventoryFacts>,
    policy_path: Option<&'a Path>,
    cfg: &'a AllowConfig,
    policy_state: allow_report::PolicyState,
    policy_diagnostic: Option<String>,
    limitations: Vec<String>,
    strict_gate_requested: bool,
    ci_guidance_completed: bool,
    signals: Option<PolicySignals<'a>>,
    instrument_failure: Option<String>,
}

fn adoption_facts(inputs: AdoptionFactInputs<'_>) -> CargoAllowResult<allow_report::AdoptionFacts> {
    let tool = crate::precommit_tool::current_tool_identity().ok();
    adoption_facts_with_tool(inputs, tool)
}

fn adoption_facts_with_tool(
    inputs: AdoptionFactInputs<'_>,
    tool: Option<crate::precommit_tool::CargoAllowToolIdentityV1>,
) -> CargoAllowResult<allow_report::AdoptionFacts> {
    let AdoptionFactInputs {
        root,
        inventory,
        inventory_facts,
        policy_path,
        cfg,
        policy_state,
        policy_diagnostic,
        mut limitations,
        strict_gate_requested,
        ci_guidance_completed,
        signals,
        instrument_failure,
    } = inputs;
    let (channel, executable_identity, tool_failure) = match tool {
        Some(identity) => (
            match identity.channel {
                crate::precommit_tool::ToolChannel::PublishedRelease => "published".to_string(),
                crate::precommit_tool::ToolChannel::SourcePreview => "source-preview".to_string(),
            },
            identity.executable_digest,
            None,
        ),
        None => (
            "unknown".to_string(),
            "unknown".to_string(),
            Some("current executable identity could not be read".to_string()),
        ),
    };
    let instrument_failure = instrument_failure.or(tool_failure);
    let policy_digest = signals.as_ref().and_then(|signals| signals.digest.clone());
    let inventory_identity = inventory_identity(root, inventory, policy_digest.as_deref());
    let (inventory_mode, completeness) = inventory_projection(inventory, inventory_facts);
    let (
        total_findings,
        new_findings,
        stale,
        drift,
        broken,
        review_due,
        expired,
        occurrence,
        mirror,
    ) = if let Some(signals) = signals {
        let count = |status| {
            signals
                .outcomes
                .iter()
                .filter(|outcome| outcome.status == status)
                .count()
        };
        (
            signals.outcomes.len(),
            count(MatchStatus::New),
            count(MatchStatus::Stale),
            count(MatchStatus::LocationDrift),
            signals
                .evidence
                .broken_evidence_links
                .saturating_add(signals.evidence.weak_evidence_references),
            count(MatchStatus::ReviewDue),
            count(MatchStatus::Expired),
            signals.evidence.occurrence_headroom_entries,
            signals.mirror_divergence,
        )
    } else {
        (0, 0, 0, 0, 0, 0, 0, 0, false)
    };
    if instrument_failure.is_some() {
        limitations.push("instrumentation failed; the recommendation is fail-closed".into());
    }
    limitations.sort();
    limitations.dedup();
    Ok(allow_report::AdoptionFacts {
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        repository_identity: inventory_identity,
        selected_root: root.display().to_string(),
        channel,
        executable_identity,
        inventory: allow_report::AdoptionInventoryFacts {
            mode: inventory_mode,
            completeness,
            limitations,
        },
        policy: allow_report::AdoptionPolicyFacts {
            state: policy_state,
            path: policy_path.map(|path| path.display().to_string()),
            schema_version: (policy_state == allow_report::PolicyState::Valid)
                .then(|| cfg.schema_version.clone()),
            digest: policy_digest,
            total_findings,
            new_unreceipted_findings: new_findings,
            stale_entries: stale,
            location_drift_entries: drift,
            broken_evidence_entries: broken,
            review_due_entries: review_due,
            expired_entries: expired,
            occurrence_headroom_entries: occurrence,
            mirror_divergence: mirror,
        },
        policy_config_diagnostic: policy_diagnostic,
        unsupported_repository_state: inventory_mode == allow_report::InventoryMode::Unknown,
        instrument_failure,
        strict_gate_requested,
        ci_guidance_completed,
    })
}

fn inventory_projection(
    inventory: Option<&Inventory>,
    facts: Option<&InventoryFacts>,
) -> (
    allow_report::InventoryMode,
    allow_report::InventoryCompleteness,
) {
    let Some(inventory) = inventory else {
        return (
            allow_report::InventoryMode::Unknown,
            allow_report::InventoryCompleteness::Unknown,
        );
    };
    let mode = match inventory.source {
        InventorySource::GitTracked | InventorySource::GitIndexStagedCandidate => {
            allow_report::InventoryMode::GitTracked
        }
        InventorySource::FilesystemFallback | InventorySource::FilesystemIncludeUntracked => {
            allow_report::InventoryMode::Filesystem
        }
    };
    let scanner_complete = facts.is_none_or(|facts| {
        facts.rust_files_skipped == 0 && facts.rust_files_with_parse_errors == 0
    });
    let complete = inventory.completeness != InventoryCompleteness::Partial && scanner_complete;
    (
        mode,
        if complete {
            allow_report::InventoryCompleteness::Complete
        } else {
            allow_report::InventoryCompleteness::Partial
        },
    )
}

fn inventory_identity(
    root: &Path,
    inventory: Option<&Inventory>,
    policy_digest: Option<&str>,
) -> String {
    let mut values = vec![
        "cargo-allow.adoption-repository.v1".to_string(),
        policy_digest.unwrap_or("no-policy").to_string(),
    ];
    if let Some(inventory) = inventory {
        values.push(inventory.source.as_str().to_string());
        values.push(inventory.completeness.as_str().to_string());
        let mut paths = inventory
            .files
            .iter()
            .map(|path| normalize_path(path.strip_prefix(root).unwrap_or(path)))
            .collect::<Vec<_>>();
        paths.sort();
        values.extend(paths);
    } else {
        values.push("unknown-inventory".to_string());
    }
    sha256_v1_bytes(values.join("\n").as_bytes())
}

fn ci_guidance_completed(root: &Path, files: &[PathBuf]) -> bool {
    files
        .iter()
        .filter_map(|path| {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            relative
                .starts_with(".github/workflows/")
                .then(|| fs::read(root.join(path)).ok())
        })
        .flatten()
        .any(|bytes| {
            let text = String::from_utf8_lossy(&bytes);
            text.contains("cargo-allow") && text.contains("--mode") && text.contains("no-new")
        })
}

fn resolve_root(root_args: &RootArgs) -> CargoAllowResult<PathBuf> {
    let cwd = current_dir()?;
    resolve_source_tree_root(root_args.root.as_deref(), cwd)
}

fn inventory_artifact(inspection: &Inspection) -> AdoptionInventoryArtifact {
    let (source, completeness, files_scanned, empty_git_tracked) =
        match (&inspection.inventory, &inspection.inventory_facts) {
            (Some(inventory), Some(facts)) => (
                inventory.source.as_str().to_string(),
                if facts.rust_files_skipped > 0 || facts.rust_files_with_parse_errors > 0 {
                    "partial".to_string()
                } else {
                    inventory.completeness.as_str().to_string()
                },
                inventory.files.len(),
                inventory.empty_git_tracked,
            ),
            _ => ("unknown".to_string(), "unknown".to_string(), 0, false),
        };
    AdoptionInventoryArtifact {
        scope: allow_report::INVENTORY_SCOPE_SOURCE_TREE,
        scanner: allow_report::INVENTORY_SCANNER_SOURCE_SYNTAX,
        source,
        root: "<repository-root>",
        files_scanned,
        completeness,
        empty_git_tracked,
    }
}

fn validate_output_path(root: &Path, output: &Path, policy: Option<&Path>) -> CargoAllowResult<()> {
    crate::assert_path_within_root(root, output)?;
    let output_absolute = if output.is_absolute() {
        output.to_path_buf()
    } else {
        root.join(output)
    };
    if policy.is_some_and(|policy| same_path(&output_absolute, policy)) {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "--output may not overwrite the selected policy config",
        ));
    }
    let tracked = match allow_inventory::git_ls_files(root) {
        Ok(files) => files,
        Err(error) if is_missing_git_metadata(&error.to_string()) => Vec::new(),
        Err(error) => {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Inventory,
                format!("cannot verify tracked output collision: {error}"),
            ));
        }
    };
    let relative = output_absolute
        .strip_prefix(root)
        .map(normalize_path)
        .unwrap_or_default();
    if tracked.iter().any(|path| normalize_path(path) == relative) {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "--output may not overwrite a tracked or staged repository file",
        ));
    }
    Ok(())
}

fn resolve_output_path(root: &Path, output: &Path) -> PathBuf {
    if output.is_absolute() {
        output.to_path_buf()
    } else {
        root.join(output)
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    comparable_path(left) == comparable_path(right)
}

fn comparable_path(path: &Path) -> PathBuf {
    if let Ok(path) = path.canonicalize() {
        return path;
    }
    let Some(file_name) = path.file_name() else {
        return path.to_path_buf();
    };
    path.parent()
        .and_then(|parent| parent.canonicalize().ok())
        .map(|parent| parent.join(file_name))
        .unwrap_or_else(|| path.to_path_buf())
}

fn is_missing_git_metadata(diagnostic: &str) -> bool {
    let diagnostic = diagnostic.to_ascii_lowercase();
    diagnostic.contains("not a git repository") || diagnostic.contains("not a git repo")
}

fn resolve_config_path(root: &Path, config: &Path) -> CargoAllowResult<PathBuf> {
    let path = if config.is_absolute() {
        config.to_path_buf()
    } else {
        root.join(config)
    };
    crate::assert_path_within_root(root, &path).map_err(|error| {
        CargoAllowError::with_kind(CargoAllowErrorKind::Usage, error.to_string())
    })?;
    Ok(path)
}

fn sanitize_diagnostic(root: &Path, diagnostic: &str) -> String {
    let root_text = root.to_string_lossy();
    let root_forward = root_text.replace('\\', "/");
    diagnostic
        .replace(root_text.as_ref(), "<repository-root>")
        .replace(&root_forward, "<repository-root>")
}

fn disposition_text(disposition: allow_report::BootstrapDisposition) -> &'static str {
    match disposition {
        allow_report::BootstrapDisposition::CleanNoPolicy => "clean_no_policy",
        allow_report::BootstrapDisposition::FindingsNoPolicy => "findings_no_policy",
        allow_report::BootstrapDisposition::ExistingPolicyHealthy => "existing_policy_healthy",
        allow_report::BootstrapDisposition::ExistingPolicyHasNewFindings => {
            "existing_policy_has_new_findings"
        }
        allow_report::BootstrapDisposition::ExistingPolicyNeedsRepair => {
            "existing_policy_needs_repair"
        }
        allow_report::BootstrapDisposition::PartialInventory => "partial_inventory",
        allow_report::BootstrapDisposition::InvalidPolicy => "invalid_policy",
        allow_report::BootstrapDisposition::UnsupportedRepositoryState => {
            "unsupported_repository_state"
        }
        allow_report::BootstrapDisposition::InstrumentFailure => "instrument_failure",
    }
}

fn render_human(plan: &allow_report::CoreAdoptionPlanV1, style: allow_report::Style) -> String {
    let primary = &plan.primary_action;
    let run = primary.argv.join(" ");
    let writes = if plan.may_write_paths.is_empty() {
        "nothing".to_string()
    } else {
        plan.may_write_paths.join(", ")
    };
    let then = plan
        .follow_up_actions
        .first()
        .map(|action| action.kind.as_str().to_string())
        .unwrap_or_else(|| "none".to_string());
    let ci = if plan.policy.state == allow_report::PolicyState::Valid
        && plan.primary_action.kind == allow_report::AdoptionActionKind::RunNoNewCheck
    {
        format!("ready ({})", plan.ci_example_path)
    } else {
        plan.ci_example_path.clone()
    };
    format!(
        "{} {}\n{} {}\n{} {run}\n{} {writes}\n{} {}\n{} {then}\n{} {ci}\n{} {}\n{} {}\n{} {}",
        style.strong("Repository state:"),
        disposition_text(plan.bootstrap_disposition),
        style.strong("Recommended next step:"),
        primary.kind.as_str(),
        style.strong("Run:"),
        style.strong("Writes:"),
        style.strong("Why:"),
        primary.reason,
        style.strong("Then:"),
        style.strong("CI:"),
        style.strong("Rollback:"),
        plan.rollback_guide_path,
        style.strong("Schema:"),
        plan.schema_id,
        style.strong("Claim boundary:"),
        plan.claim_boundary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn require(condition: bool, message: &str) -> Result<(), String> {
        condition.then_some(()).ok_or_else(|| message.to_string())
    }

    fn sample_plan() -> allow_report::CoreAdoptionPlanV1 {
        let facts = allow_report::AdoptionFacts {
            tool_version: "0.1.11".to_string(),
            repository_identity: "sha256:v1:test-repository".to_string(),
            selected_root: "C:\\repo".to_string(),
            channel: "source-preview".to_string(),
            executable_identity: "sha256:v1:test-executable".to_string(),
            inventory: allow_report::AdoptionInventoryFacts {
                mode: allow_report::InventoryMode::GitTracked,
                completeness: allow_report::InventoryCompleteness::Complete,
                limitations: Vec::new(),
            },
            policy: allow_report::AdoptionPolicyFacts {
                state: allow_report::PolicyState::Absent,
                path: None,
                schema_version: None,
                digest: None,
                total_findings: 0,
                new_unreceipted_findings: 0,
                stale_entries: 0,
                location_drift_entries: 0,
                broken_evidence_entries: 0,
                review_due_entries: 0,
                expired_entries: 0,
                occurrence_headroom_entries: 0,
                mirror_divergence: false,
            },
            policy_config_diagnostic: None,
            unsupported_repository_state: false,
            instrument_failure: None,
            strict_gate_requested: false,
            ci_guidance_completed: false,
        };
        allow_report::recommend_core_adoption_plan(&facts)
    }

    fn inventory(source: InventorySource, completeness: InventoryCompleteness) -> Inventory {
        Inventory {
            files: vec![PathBuf::from("src/lib.rs")],
            source,
            completeness,
            empty_git_tracked: false,
            deleted_tracked: Vec::new(),
            inaccessible_paths: Vec::new(),
            git_error: None,
            skipped_paths: Vec::new(),
            submodule_paths: Vec::new(),
        }
    }

    #[test]
    fn projections_and_artifacts_fail_closed_for_unknown_or_partial_inputs() -> Result<(), String> {
        let unknown = inventory_projection(None, None);
        require(
            unknown
                == (
                    allow_report::InventoryMode::Unknown,
                    allow_report::InventoryCompleteness::Unknown,
                ),
            "missing inventory should remain unknown",
        )?;

        let complete_inventory =
            inventory(InventorySource::GitTracked, InventoryCompleteness::Complete);
        let complete_facts = InventoryFacts::scanned_inventory(&complete_inventory);
        require(
            inventory_projection(Some(&complete_inventory), Some(&complete_facts))
                == (
                    allow_report::InventoryMode::GitTracked,
                    allow_report::InventoryCompleteness::Complete,
                ),
            "complete tracked inventory should project as complete",
        )?;

        let mut partial_facts = InventoryFacts::scanned_inventory(&complete_inventory);
        partial_facts.rust_files_with_parse_errors = 1;
        require(
            inventory_projection(Some(&complete_inventory), Some(&partial_facts)).1
                == allow_report::InventoryCompleteness::Partial,
            "parse errors should make the projection partial",
        )?;

        let filesystem_inventory = inventory(
            InventorySource::FilesystemIncludeUntracked,
            InventoryCompleteness::Fallback,
        );
        require(
            inventory_projection(Some(&filesystem_inventory), None).0
                == allow_report::InventoryMode::Filesystem,
            "filesystem inventory should preserve its mode",
        )?;

        let mut rooted_inventory = inventory(
            InventorySource::GitIndexStagedCandidate,
            InventoryCompleteness::Scoped,
        );
        *rooted_inventory
            .files
            .first_mut()
            .ok_or_else(|| "sample inventory should contain a file".to_string())? =
            PathBuf::from("repo-root/src/lib.rs");
        let rooted_identity = inventory_identity(
            Path::new("repo-root"),
            Some(&rooted_inventory),
            Some("digest"),
        );
        require(
            !rooted_identity.is_empty(),
            "rooted inventory paths should contribute to identity",
        )?;

        let plan = sample_plan();
        let missing = Inspection {
            root: PathBuf::from("C:\\repo"),
            inventory: None,
            inventory_facts: None,
            policy_path: None,
            plan: plan.clone(),
        };
        let missing_artifact = inventory_artifact(&missing);
        require(
            missing_artifact.source == "unknown" && missing_artifact.completeness == "unknown",
            "missing artifact inputs should remain unknown",
        )?;

        let present = Inspection {
            root: PathBuf::from("C:\\repo"),
            inventory: Some(complete_inventory),
            inventory_facts: Some(partial_facts),
            policy_path: None,
            plan,
        };
        let present_artifact = inventory_artifact(&present);
        require(
            present_artifact.source == "git_tracked"
                && present_artifact.completeness == "partial"
                && present_artifact.files_scanned == 1,
            "partial scanner facts should be visible in the artifact",
        )?;
        let complete_facts = InventoryFacts::scanned_inventory(
            present
                .inventory
                .as_ref()
                .ok_or_else(|| "inventory should exist".to_string())?,
        );
        let complete = inventory_artifact(&Inspection {
            root: present.root.clone(),
            inventory: present.inventory.clone(),
            inventory_facts: Some(complete_facts),
            policy_path: None,
            plan: present.plan.clone(),
        });
        require(
            complete.completeness == "complete",
            "complete scanner facts should remain complete",
        )
    }

    #[test]
    fn path_and_diagnostic_helpers_preserve_repository_boundaries() -> Result<(), String> {
        let root = std::env::temp_dir().join("cargo-allow-adoption-path-root");
        require(
            resolve_output_path(&root, Path::new("target/plan.json"))
                == root.join("target").join("plan.json"),
            "relative output should resolve under the root",
        )?;
        let absolute = std::env::temp_dir().join("cargo-allow-adoption-plan.json");
        require(
            resolve_output_path(&root, &absolute) == absolute,
            "absolute output should remain unchanged for boundary validation",
        )?;
        require(
            same_path(Path::new("plan.json"), Path::new("plan.json")),
            "equal paths should match",
        )?;
        require(
            comparable_path(&root.join("plan.json")) == root.join("plan.json"),
            "unresolved paths should remain comparable",
        )?;
        require(
            is_missing_git_metadata("fatal: not a git repository"),
            "git repository errors should be recognized",
        )?;
        require(
            is_missing_git_metadata("not a git repo"),
            "short git repository errors should be recognized",
        )?;
        require(
            !is_missing_git_metadata("permission denied while reading the index"),
            "unrelated git errors should not be hidden",
        )?;
        let sanitized = sanitize_diagnostic(
            &root,
            &format!(
                "{}{}policy{}allow.toml failed",
                root.display(),
                std::path::MAIN_SEPARATOR,
                std::path::MAIN_SEPARATOR,
            ),
        );
        require(
            sanitized.contains("<repository-root>")
                && !sanitized.contains(root.to_string_lossy().as_ref()),
            "diagnostics should use a portable root marker",
        )?;
        require(
            resolve_config_path(&root, Path::new("policy/allow.toml")).is_ok(),
            "in-root config should resolve",
        )?;
        require(
            resolve_config_path(
                &root,
                &root
                    .parent()
                    .ok_or_else(|| "temporary root should have a parent".to_string())?
                    .join("outside.toml"),
            )
            .is_err(),
            "outside config should fail",
        )
    }

    #[test]
    fn human_rendering_covers_write_then_and_ready_ci_variants() -> Result<(), String> {
        let mut plan = sample_plan();
        plan.follow_up_actions.clear();
        let plain = render_human(&plan, allow_report::Style::PLAIN);
        require(
            plain.contains("Writes: nothing"),
            "empty write posture should be explicit",
        )?;
        require(
            plain.contains("Then: none"),
            "missing follow-up should be explicit",
        )?;
        require(
            plain.contains("CI: docs/how-to/adopt-cargo-allow.md"),
            "ci guidance should be shown",
        )?;

        plan.policy.state = allow_report::PolicyState::Valid;
        plan.primary_action.kind = allow_report::AdoptionActionKind::RunNoNewCheck;
        plan.primary_action.argv.clear();
        plan.may_write_paths = vec!["policy/allow.toml".to_string()];
        let ready = render_human(&plan, allow_report::Style::PLAIN);
        require(
            ready.contains("ready (docs/how-to/adopt-cargo-allow.md"),
            "ready ci state should be labeled",
        )?;
        require(
            ready.contains("Writes: policy/allow.toml"),
            "write paths should be rendered",
        )?;
        let ansi = render_human(&plan, allow_report::Style::ANSI);
        require(
            ansi.contains("\u{1b}"),
            "ansi output should use the selected style",
        )
    }

    #[test]
    fn identity_and_ci_guidance_are_deterministic() -> Result<(), String> {
        let root = PathBuf::from("C:\\repo");
        let tracked = inventory(InventorySource::GitTracked, InventoryCompleteness::Complete);
        let first = inventory_identity(&root, Some(&tracked), None);
        let second = inventory_identity(&root, Some(&tracked), None);
        require(
            first == second,
            "same inventory should have a stable identity",
        )?;
        require(
            inventory_identity(&root, None, Some("digest")) != first,
            "policy digest should contribute to identity",
        )?;

        let test_root =
            std::env::temp_dir().join(format!("cargo-allow-adoption-unit-{}", std::process::id()));
        fs::create_dir_all(test_root.join(".github/workflows"))
            .map_err(|error| error.to_string())?;
        let workflow = test_root.join(".github/workflows/ci.yml");
        fs::write(&workflow, "cargo-allow check --mode no-new")
            .map_err(|error| error.to_string())?;
        let unrelated = test_root.join("README.md");
        fs::write(&unrelated, "readme").map_err(|error| error.to_string())?;
        require(
            ci_guidance_completed(&test_root, &[workflow, unrelated.clone()]),
            "matching workflow should be detected",
        )?;
        require(
            !ci_guidance_completed(&test_root, &[unrelated]),
            "non-workflow files should not provide ci guidance",
        )?;
        fs::remove_dir_all(test_root).map_err(|error| error.to_string())
    }

    #[test]
    fn adoption_facts_preserve_published_and_fail_closed_tool_identity() -> Result<(), String> {
        let root = PathBuf::from("C:\\repo");
        let cfg = AllowConfig::empty();
        let inputs = || AdoptionFactInputs {
            root: &root,
            inventory: None,
            inventory_facts: None,
            policy_path: None,
            cfg: &cfg,
            policy_state: allow_report::PolicyState::Absent,
            policy_diagnostic: None,
            limitations: Vec::new(),
            strict_gate_requested: false,
            ci_guidance_completed: false,
            signals: None,
            instrument_failure: None,
        };
        let mut published = crate::precommit_tool::identity_for_bytes(b"published-test");
        published.channel = crate::precommit_tool::ToolChannel::PublishedRelease;
        let published_facts = adoption_facts_with_tool(inputs(), Some(published))
            .map_err(|error| error.to_string())?;
        require(
            published_facts.channel == "published",
            "published identities should retain their release channel",
        )?;

        let unknown_facts =
            adoption_facts_with_tool(inputs(), None).map_err(|error| error.to_string())?;
        require(
            unknown_facts.channel == "unknown" && unknown_facts.instrument_failure.is_some(),
            "missing identities should fail closed",
        )
    }

    #[test]
    fn every_bootstrap_disposition_has_a_stable_text_name() -> Result<(), String> {
        let dispositions = [
            allow_report::BootstrapDisposition::CleanNoPolicy,
            allow_report::BootstrapDisposition::FindingsNoPolicy,
            allow_report::BootstrapDisposition::ExistingPolicyHealthy,
            allow_report::BootstrapDisposition::ExistingPolicyHasNewFindings,
            allow_report::BootstrapDisposition::ExistingPolicyNeedsRepair,
            allow_report::BootstrapDisposition::PartialInventory,
            allow_report::BootstrapDisposition::InvalidPolicy,
            allow_report::BootstrapDisposition::UnsupportedRepositoryState,
            allow_report::BootstrapDisposition::InstrumentFailure,
        ];
        dispositions.iter().try_for_each(|disposition| {
            require(
                !disposition_text(*disposition).is_empty(),
                "bootstrap disposition names must not be empty",
            )
        })
    }

    #[test]
    fn command_projection_writes_json_without_mutating_source_state() -> Result<(), String> {
        let root = current_dir().map_err(|error| error.to_string())?;
        let output = PathBuf::from(format!(
            "target/cargo-allow/adoption-unit-{}.json",
            std::process::id()
        ));
        fs::create_dir_all(root.join("target/cargo-allow")).map_err(|error| error.to_string())?;
        let result = cmd_adopt(&AdoptionArgs {
            root: RootArgs {
                root: Some(root.clone()),
            },
            config: None,
            include_untracked: false,
            strict: false,
            format: HumanJsonFormat::Json,
            output: Some(output.clone()),
        });
        result.map_err(|error| error.to_string())?;
        let artifact = fs::read_to_string(root.join(&output)).map_err(|error| error.to_string())?;
        require(
            artifact.contains("\"command\": \"adopt\""),
            "direct command projection should write the adoption artifact",
        )?;
        fs::remove_file(root.join(output)).map_err(|error| error.to_string())
    }

    #[test]
    fn command_projection_renders_human_output_without_writing() -> Result<(), String> {
        let root = current_dir().map_err(|error| error.to_string())?;
        cmd_adopt(&AdoptionArgs {
            root: RootArgs { root: Some(root) },
            config: None,
            include_untracked: false,
            strict: false,
            format: HumanJsonFormat::Human,
            output: None,
        })
        .map_err(|error| error.to_string())
    }
}
