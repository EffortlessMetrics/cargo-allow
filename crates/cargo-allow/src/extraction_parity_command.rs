use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_inventory::resolve_source_tree_root;
use allow_policy::extraction_parity::{
    corpus_digest, produce_extraction_cutover_receipt, validate_cutover_reachability,
    AuthorityKind, AuthorityNode, ExtractionCutoverReceipt, ExtractionCutoverReceiptEvidence,
    ExtractionParityRegistry, ExtractionStage, OldPathCase, ParityComparison,
    ParityComparisonResult,
};
use allow_policy::product_move::{ProductMoveLedger, parse_product_move_ledger_at};
use clap::{Args, ValueEnum};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{current_dir, emit_text, RootArgs};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ParityStageArg {
    All,
    RepoSnapshot,
    RepoEdit,
}

#[derive(Debug, Args)]
pub(crate) struct ParityArgs {
    #[command(flatten)]
    pub(crate) root: RootArgs,
    /// Extraction stage to execute.
    #[arg(long, value_enum, default_value_t = ParityStageArg::All)]
    pub(crate) stage: ParityStageArg,
    /// Write the runtime evidence JSON to this path instead of stdout.
    #[arg(long, value_name = "PATH")]
    pub(crate) output: Option<PathBuf>,
    /// Assemble a cutover receipt from an explicit package/build evidence bundle.
    /// The command derives source identity, parity digest, ledger coverage, and
    /// reachability from the current checkout and fails closed on stale or
    /// incomplete evidence.
    #[arg(long, value_name = "PATH")]
    pub(crate) cutover_evidence: Option<PathBuf>,
}

pub(crate) fn cmd_parity(args: &ParityArgs) -> CargoAllowResult<()> {
    let root = resolve_source_tree_root(args.root.root.as_deref(), current_dir()?)?;
    let initial_source_identity = source_identity(&root)?;
    let mut records = Vec::new();

    if matches!(
        args.stage,
        ParityStageArg::All | ParityStageArg::RepoSnapshot
    ) {
        let run = crate::extraction_parity_runtime::run_repo_snapshot_parity(&root)?;
        append_snapshot_case(
            &mut records,
            "parity-repo-snapshot-revision-identity-v1",
            run.committed,
        );
        append_snapshot_case(
            &mut records,
            "parity-repo-snapshot-staged-index-v1",
            run.staged,
        );
    }
    if matches!(args.stage, ParityStageArg::All | ParityStageArg::RepoEdit) {
        let run = crate::extraction_repo_edit_runtime::run_repo_edit_parity()?;
        for case in run.cases {
            append_record(
                &mut records,
                case.id,
                case.comparison,
                case.old_output,
                case.new_output,
            );
        }
    }

    if source_identity(&root)? != initial_source_identity {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            "source Git commit/tree changed during parity execution",
        ));
    }

    let registry = load_registry(&root)?;
    let expected_case_ids: BTreeSet<_> = registry
        .case
        .iter()
        .filter(|case| stage_matches(case.stage.as_str(), args.stage))
        .map(|case| case.id.as_str())
        .collect();
    let actual_case_ids: BTreeSet<_> = records
        .iter()
        .filter_map(|record| record.get("case_id").and_then(Value::as_str))
        .collect();
    let missing_case_ids: Vec<_> = expected_case_ids
        .difference(&actual_case_ids)
        .copied()
        .collect();
    let unexpected_case_ids: Vec<_> = actual_case_ids
        .difference(&expected_case_ids)
        .copied()
        .collect();
    let complete = missing_case_ids.is_empty() && unexpected_case_ids.is_empty();

    let digest_records = records
        .iter()
        .map(|record| -> CargoAllowResult<_> {
            Ok((
                record
                    .get("case_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                ParityComparison {
                    result: ParityComparisonResult::parse(
                        record
                            .get("result")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    )?,
                    source_identity: record
                        .get("source_identity")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                },
                record
                    .get("old_output")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                record
                    .get("new_output")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            ))
        })
        .collect::<CargoAllowResult<Vec<_>>>()?;
    let digest = corpus_digest(&digest_records);
    let equivalent = records.iter().all(|record| {
        record.get("result").and_then(Value::as_str)
            == Some(ParityComparisonResult::SemanticallyEquivalent.as_str())
    });
    let passed = complete && equivalent;
    let claim_boundary = if args.cutover_evidence.is_some() {
        vec![
            "runtime_old_new_parity_and_cutover_receipt",
            "exact_git_commit_and_tree_identity",
            "policy_derived_ledger_coverage",
            "policy_derived_reachability_only",
            "explicit_package_and_build_evidence",
        ]
    } else {
        vec![
            "runtime_old_new_parity_only",
            "exact_git_commit_and_tree_identity",
            "no_cutover_receipt_claim",
            "no_policy_disposition_promotion",
            "no_reachability_or_package_ownership_claim",
        ]
    };
    let mut payload = json!({
        "schema_id": "cargo-allow.extraction-parity-runtime.v1",
        "schema_version": 1,
        "tool": "cargo-allow extraction-parity",
        "result": if passed { "Passed" } else if !complete { "Incomplete" } else { "Failed" },
        "completeness": if complete { "Complete" } else { "Partial" },
        "source_identity": initial_source_identity,
        "stage": stage_name(args.stage),
        "parity_result_digest": digest,
        "records": records,
        "expected_case_count": expected_case_ids.len(),
        "emitted_case_count": actual_case_ids.len(),
        "missing_case_ids": missing_case_ids,
        "unexpected_case_ids": unexpected_case_ids,
        "claim_boundary": claim_boundary,
    });
    if let Some(evidence_path) = args.cutover_evidence.as_deref() {
        let receipt = assemble_cutover_receipt(
            &root,
            &registry,
            args.stage,
            &digest,
            &initial_source_identity,
            evidence_path,
            passed,
        )?;
        payload["cutover_receipt"] = serde_json::to_value(receipt).map_err(|error| {
            CargoAllowError::new(format!("render extraction cutover receipt: {error}"))
        })?;
    }
    let rendered = serde_json::to_string_pretty(&payload)
        .map_err(|error| CargoAllowError::new(format!("render parity evidence: {error}")))?
        + "\n";
    emit_text(args.output.as_deref(), &rendered)?;
    if passed {
        Ok(())
    } else {
        Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            "runtime parity evidence contains a non-equivalent case",
        ))
    }
}

fn load_registry(
    root: &Path,
) -> CargoAllowResult<allow_policy::extraction_parity::ExtractionParityRegistry> {
    let path = root.join("policy/extraction-parity.toml");
    let input = fs::read_to_string(&path).map_err(|error| {
        CargoAllowError::new(format!(
            "read extraction parity registry {}: {error}",
            path.display()
        ))
    })?;
    allow_policy::extraction_parity::parse_extraction_parity_registry_at(Some(&path), &input)
}

fn load_ledger(root: &Path) -> CargoAllowResult<ProductMoveLedger> {
    let path = root.join("policy/product-move-ledger.toml");
    let input = fs::read_to_string(&path).map_err(|error| {
        CargoAllowError::new(format!(
            "read product move ledger {}: {error}",
            path.display()
        ))
    })?;
    parse_product_move_ledger_at(Some(&path), &input)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CutoverEvidenceInput {
    schema_id: String,
    schema_version: u32,
    source_identity: String,
    architecture_manifest_digest: String,
    old_tool_identity: String,
    new_tool_identity: String,
    #[serde(default)]
    accepted_intentional_differences: Vec<String>,
    latest_allowed_shim_stage: String,
    package_assets_docs_ci_ownership_result: String,
    independent_build_package_result: String,
    rollback_route: String,
    result_class: String,
    completeness: String,
    #[serde(default)]
    limitations: Vec<String>,
    claim_boundary: String,
}

const CUTOVER_EVIDENCE_SCHEMA_ID: &str = "cargo-allow.extraction-cutover-evidence.v1";
const CUTOVER_EVIDENCE_SCHEMA_VERSION: u32 = 1;

fn assemble_cutover_receipt(
    root: &Path,
    registry: &ExtractionParityRegistry,
    requested_stage: ParityStageArg,
    parity_digest: &str,
    source_identity: &str,
    evidence_path: &Path,
    parity_passed: bool,
) -> CargoAllowResult<ExtractionCutoverReceipt> {
    let stage = match requested_stage {
        ParityStageArg::RepoSnapshot => ExtractionStage::RepoSnapshot,
        ParityStageArg::RepoEdit => ExtractionStage::RepoEdit,
        ParityStageArg::All => {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                "cutover receipts require one extraction stage, not `all`",
            ));
        }
    };
    if !parity_passed {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            "cutover receipt requires complete, equivalent runtime parity",
        ));
    }

    let input_text = fs::read_to_string(evidence_path).map_err(|error| {
        CargoAllowError::new(format!(
            "read cutover evidence {}: {error}",
            evidence_path.display()
        ))
    })?;
    let evidence: CutoverEvidenceInput = serde_json::from_str(&input_text).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "parse cutover evidence {}: {error}",
                evidence_path.display()
            ),
        )
    })?;
    if evidence.schema_id != CUTOVER_EVIDENCE_SCHEMA_ID
        || evidence.schema_version != CUTOVER_EVIDENCE_SCHEMA_VERSION
    {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            "unsupported extraction cutover evidence schema",
        ));
    }
    if evidence.source_identity != source_identity {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "cutover evidence source identity `{}` does not match current `{source_identity}`",
                evidence.source_identity
            ),
        ));
    }
    ensure_cutover_inputs_clean(root)?;
    require_positive_outcome(
        "package_assets_docs_ci_ownership_result",
        &evidence.package_assets_docs_ci_ownership_result,
    )?;
    require_positive_outcome(
        "independent_build_package_result",
        &evidence.independent_build_package_result,
    )?;
    require_positive_outcome("completeness", &evidence.completeness)?;

    let ledger = load_ledger(root)?;
    let stage_cases: Vec<_> = registry
        .case
        .iter()
        .filter(|case| case.stage == stage)
        .collect();
    let mut old_path_cases = Vec::new();
    let mut authority_nodes = Vec::new();
    let mut shim_stages = BTreeSet::new();
    for case in stage_cases {
        let entry = ledger
            .entry
            .iter()
            .find(|entry| entry.id == case.move_ledger_entry)
            .ok_or_else(|| {
                CargoAllowError::with_kind(
                    CargoAllowErrorKind::InvalidConfig,
                    format!("missing ledger entry `{}`", case.move_ledger_entry),
                )
            })?;
        if entry.cutover_stage != stage.as_str() {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!(
                    "ledger entry `{}` belongs to `{}`, not `{}`",
                    entry.id,
                    entry.cutover_stage,
                    stage.as_str()
                ),
            ));
        }
        old_path_cases.push(OldPathCase {
            ledger_entry_id: entry.id.clone(),
            disposition: entry.old_path_reachability_disposition.clone(),
        });
        authority_nodes.push(AuthorityNode {
            id: entry.current_identity.clone(),
            ledger_entry_id: entry.id.clone(),
            kind: authority_kind(&entry.duplicate_authority_class),
            production_reachable: !matches!(
                entry.old_path_reachability_disposition.as_str(),
                "Deleted" | "CompileUnreachable" | "FeatureUnreachableInSupportedCandidate"
            ),
            bound: (!entry.active_shim_ids.is_empty())
                .then(|| entry.latest_allowed_shim_stage.clone()),
        });
        if !entry.active_shim_ids.is_empty() {
            shim_stages.insert(entry.latest_allowed_shim_stage.clone());
        }
    }
    let (reachability_report, reachability_diagnostics) =
        validate_cutover_reachability(&old_path_cases, &authority_nodes);
    if !reachability_diagnostics.is_empty() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "current ledger reachability is not cutover-clean: {reachability_diagnostics:?}"
            ),
        ));
    }
    let old_path_result = serde_json::to_string(&reachability_report.disposition_counts)
        .map_err(|error| CargoAllowError::new(format!("render reachability evidence: {error}")))?;
    let duplicate_result = format!(
        "semantic_authorities={}",
        reachability_report.production_semantic_authority_count
    );
    if shim_stages.len() > 1 {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("selected ledger entries have conflicting shim deadlines: {shim_stages:?}"),
        ));
    }
    let derived_shim_stage = match shim_stages.into_iter().next() {
        Some(stage) => stage,
        None => String::new(),
    };
    if evidence.latest_allowed_shim_stage != derived_shim_stage {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "evidence shim deadline `{}` does not match policy-derived `{derived_shim_stage}`",
                evidence.latest_allowed_shim_stage
            ),
        ));
    }
    let evidence = ExtractionCutoverReceiptEvidence {
        source_identity: source_identity.to_string(),
        architecture_manifest_digest: evidence.architecture_manifest_digest,
        old_tool_identity: evidence.old_tool_identity,
        new_tool_identity: evidence.new_tool_identity,
        parity_corpus_generation: format!("runtime:extraction-parity-v1:stage={}", stage.as_str()),
        parity_result_digest: parity_digest.to_string(),
        accepted_intentional_differences: evidence.accepted_intentional_differences,
        latest_allowed_shim_stage: derived_shim_stage,
        old_path_reachability_result: old_path_result,
        duplicate_semantic_implementation_result: duplicate_result,
        package_assets_docs_ci_ownership_result: evidence.package_assets_docs_ci_ownership_result,
        independent_build_package_result: evidence.independent_build_package_result,
        rollback_route: evidence.rollback_route,
        result_class: evidence.result_class,
        completeness: evidence.completeness,
        limitations: evidence.limitations,
        claim_boundary: evidence.claim_boundary,
    };
    produce_extraction_cutover_receipt(registry, &ledger, stage, evidence)
}

fn require_positive_outcome(field: &str, value: &str) -> CargoAllowResult<()> {
    let normalized = value.trim().to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "pass" | "passed" | "proven" | "success" | "successful" | "complete" | "completed"
    ) {
        Ok(())
    } else {
        Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("{field} must be a positive complete outcome, got `{value}`"),
        ))
    }
}

fn ensure_cutover_inputs_clean(root: &Path) -> CargoAllowResult<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--",
            "crates/cargo-allow/src/extraction_parity_command.rs",
            "crates/allow-policy/src/extraction_parity",
            "policy/extraction-parity.toml",
            "policy/product-move-ledger.toml",
        ])
        .output()
        .map_err(|error| CargoAllowError::new(format!("run git status for cutover inputs: {error}")))?;
    if !output.status.success() {
        return Err(CargoAllowError::new(format!(
            "git status for cutover inputs failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let changed = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !changed.is_empty() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("cutover receipt requires clean source inputs; changed paths:\n{changed}"),
        ));
    }
    Ok(())
}

fn authority_kind(value: &str) -> AuthorityKind {
    match value {
        "CompatibilityProjection" => AuthorityKind::CompatibilityProjection,
        "HistoricalReader" => AuthorityKind::HistoricalReader,
        "TestFixtureOnly" => AuthorityKind::TestFixtureOnly,
        "GeneratedView" => AuthorityKind::GeneratedView,
        _ => AuthorityKind::SemanticEvaluator,
    }
}

fn stage_name(stage: ParityStageArg) -> &'static str {
    match stage {
        ParityStageArg::All => "All",
        ParityStageArg::RepoSnapshot => "RepoSnapshot",
        ParityStageArg::RepoEdit => "RepoEdit",
    }
}

fn stage_matches(registered: &str, requested: ParityStageArg) -> bool {
    match requested {
        ParityStageArg::All => matches!(registered, "RepoSnapshot" | "RepoEdit"),
        ParityStageArg::RepoSnapshot => registered == "RepoSnapshot",
        ParityStageArg::RepoEdit => registered == "RepoEdit",
    }
}

fn append_snapshot_case(
    records: &mut Vec<Value>,
    label: &str,
    case: crate::extraction_parity_runtime::RepoSnapshotParityCase,
) {
    append_record(
        records,
        label.to_string(),
        case.comparison,
        case.old_output,
        case.new_output,
    );
}

fn append_record(
    records: &mut Vec<Value>,
    case_id: String,
    comparison: ParityComparison,
    old_output: String,
    new_output: String,
) {
    records.push(json!({
        "case_id": case_id,
        "result": comparison.result.as_str(),
        "source_identity": comparison.source_identity,
        "old_output": old_output,
        "new_output": new_output,
    }));
}

fn source_identity(root: &Path) -> CargoAllowResult<String> {
    let commit = git_value(root, &["rev-parse", "HEAD"])?;
    let tree = git_value(root, &["rev-parse", "HEAD^{tree}"])?;
    Ok(format!("commit:{commit}/tree:{tree}"))
}

fn git_value(root: &Path, args: &[&str]) -> CargoAllowResult<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|error| CargoAllowError::new(format!("run git {:?}: {error}", args)))?;
    if !output.status.success() {
        return Err(CargoAllowError::new(format!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        return Err(CargoAllowError::new(format!(
            "git {:?} returned an empty value",
            args
        )));
    }
    Ok(value)
}
