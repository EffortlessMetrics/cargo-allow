use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult, sha256_v1_bytes};
use allow_inventory::resolve_source_tree_root;
use allow_policy::extraction_parity::{
    AuthorityKind, AuthorityNode, ExtractionCutoverReceipt, ExtractionCutoverReceiptEvidence,
    ExtractionParityRegistry, ExtractionStage, OldPathCase, ParityComparison,
    ParityComparisonResult, corpus_digest, produce_extraction_cutover_receipt,
    validate_cutover_reachability,
};
use allow_policy::product_crates::current_architecture_receipt_at;
use allow_policy::product_move::{ProductMoveLedger, parse_product_move_ledger_at};
use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{RootArgs, current_dir, emit_text};

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
        let receipt_value = serde_json::to_value(receipt).map_err(|error| {
            CargoAllowError::new(format!("render extraction cutover receipt: {error}"))
        })?;
        let object = payload.as_object_mut().ok_or_else(|| {
            CargoAllowError::new("render extraction cutover receipt requires JSON object payload")
        })?;
        object.insert("cutover_receipt".to_string(), receipt_value);
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
    ownership_receipt: PathBuf,
    independent_build_package_receipt: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct OwnershipEvidenceReceipt {
    schema_id: String,
    schema_version: u32,
    stage: String,
    source_identity: String,
    parity_result_digest: String,
    result: String,
    package_paths: Vec<String>,
    asset_paths: Vec<String>,
    docs_paths: Vec<String>,
    ci_paths: Vec<String>,
    claim_boundary: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct IndependentBuildPackageReceipt {
    schema_id: String,
    schema_version: u32,
    stage: String,
    source_identity: String,
    architecture_manifest_digest: String,
    parity_result_digest: String,
    result: String,
    independent: bool,
    source_checkout_denied: bool,
    package_records: Vec<PackageEvidenceRecord>,
    build_records: Vec<BuildEvidenceRecord>,
    claim_boundary: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PackageEvidenceRecord {
    package_name: String,
    path: String,
    sha256: String,
    result: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BuildEvidenceRecord {
    artifact_name: String,
    path: String,
    sha256: String,
    result: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DerivedOwnership {
    architecture_manifest_digest: String,
    package_paths: Vec<String>,
    asset_paths: Vec<String>,
    docs_paths: Vec<String>,
    ci_paths: Vec<String>,
    package_names: BTreeSet<String>,
}

const CUTOVER_EVIDENCE_SCHEMA_ID: &str = "cargo-allow.extraction-cutover-evidence.v2";
const CUTOVER_EVIDENCE_SCHEMA_VERSION: u32 = 2;
const OWNERSHIP_EVIDENCE_SCHEMA_ID: &str = "cargo-allow.extraction-cutover-ownership.v1";
const BUILD_PACKAGE_EVIDENCE_SCHEMA_ID: &str = "cargo-allow.extraction-cutover-build-package.v1";

fn assemble_cutover_receipt(
    root: &Path,
    registry: &ExtractionParityRegistry,
    requested_stage: ParityStageArg,
    parity_digest: &str,
    source_identity: &str,
    evidence_path: &Path,
    parity_passed: bool,
) -> CargoAllowResult<ExtractionCutoverReceipt> {
    ensure_cutover_inputs_clean(root)?;
    assemble_cutover_receipt_from_clean_inputs(
        root,
        registry,
        requested_stage,
        parity_digest,
        source_identity,
        evidence_path,
        parity_passed,
    )
}

fn assemble_cutover_receipt_from_clean_inputs(
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

    let evidence_path = resolve_evidence_path(root, evidence_path)?;

    let input_text = fs::read_to_string(&evidence_path).map_err(|error| {
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
    let ledger = load_ledger(root)?;
    let derived_ownership = derive_ownership(root, registry, &ledger, stage)?;
    let ownership_path = resolve_receipt_path(root, &evidence.ownership_receipt)?;
    let ownership_text = fs::read_to_string(&ownership_path).map_err(|error| {
        CargoAllowError::new(format!(
            "read ownership receipt {}: {error}",
            ownership_path.display()
        ))
    })?;
    let ownership: OwnershipEvidenceReceipt =
        serde_json::from_str(&ownership_text).map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!(
                    "parse ownership receipt {}: {error}",
                    ownership_path.display()
                ),
            )
        })?;
    let ownership_result = validate_ownership_receipt(
        root,
        &ownership,
        stage,
        source_identity,
        parity_digest,
        &derived_ownership,
    )?;
    let build_path = resolve_receipt_path(root, &evidence.independent_build_package_receipt)?;
    let build_text = fs::read_to_string(&build_path).map_err(|error| {
        CargoAllowError::new(format!(
            "read independent build/package receipt {}: {error}",
            build_path.display()
        ))
    })?;
    let build: IndependentBuildPackageReceipt =
        serde_json::from_str(&build_text).map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!(
                    "parse independent build/package receipt {}: {error}",
                    build_path.display()
                ),
            )
        })?;
    let build_result = validate_build_package_receipt(
        root,
        &build,
        stage,
        source_identity,
        parity_digest,
        &derived_ownership,
    )?;

    let stage_cases: Vec<_> = registry
        .case
        .iter()
        .filter(|case| case.stage == stage)
        .collect();
    let mut old_path_cases = Vec::new();
    let mut authority_nodes = Vec::new();
    let mut shim_stages = BTreeSet::new();
    for case in &stage_cases {
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
    let derived_shim_stage = shim_stages.into_iter().next().unwrap_or_default();
    let old_tool_identity = stage_cases
        .iter()
        .map(|case| case.old_producer.as_str())
        .collect::<Vec<_>>()
        .join("|");
    let new_tool_identity = stage_cases
        .iter()
        .map(|case| case.new_producer.as_str())
        .collect::<Vec<_>>()
        .join("|");
    let accepted_intentional_differences = stage_cases
        .iter()
        .filter(|case| {
            case.expected_result == ParityComparisonResult::IntentionalDifferenceAccepted
        })
        .map(|case| case.id.clone())
        .collect();
    let rollback_route = stage_cases
        .iter()
        .filter_map(|case| {
            ledger
                .entry
                .iter()
                .find(|entry| entry.id == case.move_ledger_entry)
                .map(|entry| entry.rollback.as_str())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(" | ");
    let evidence = ExtractionCutoverReceiptEvidence {
        source_identity: source_identity.to_string(),
        architecture_manifest_digest: derived_ownership.architecture_manifest_digest,
        old_tool_identity,
        new_tool_identity,
        parity_corpus_generation: format!("runtime:extraction-parity-v1:stage={}", stage.as_str()),
        parity_result_digest: parity_digest.to_string(),
        accepted_intentional_differences,
        latest_allowed_shim_stage: derived_shim_stage,
        old_path_reachability_result: old_path_result,
        duplicate_semantic_implementation_result: duplicate_result,
        package_assets_docs_ci_ownership_result: ownership_result,
        independent_build_package_result: build_result,
        rollback_route,
        result_class: "ParityAccepted".to_string(),
        completeness: "Complete".to_string(),
        limitations: vec![
            "Exact source identity and runtime parity digest only".to_string(),
            "No universal semantic equivalence or release-readiness claim".to_string(),
            "Policy promotion, publication, tagging, and release execution remain out of scope"
                .to_string(),
        ],
        claim_boundary: format!(
            "Stage-specific runtime parity, source-derived ownership, and independent build/package evidence for {}",
            stage.as_str()
        ),
    };
    produce_extraction_cutover_receipt(registry, &ledger, stage, evidence)
}

fn derive_ownership(
    root: &Path,
    registry: &ExtractionParityRegistry,
    ledger: &ProductMoveLedger,
    stage: ExtractionStage,
) -> CargoAllowResult<DerivedOwnership> {
    let architecture = current_architecture_receipt_at(root)?;
    let architecture_json = architecture.render_json()?;
    let architecture_manifest_digest = sha256_v1_bytes(architecture_json.as_bytes());
    let stage_cases: Vec<_> = registry
        .case
        .iter()
        .filter(|case| case.stage == stage)
        .collect();
    if stage_cases.is_empty() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("no ownership can be derived for stage `{}`", stage.as_str()),
        ));
    }

    let entries: Vec<_> = stage_cases
        .iter()
        .map(|case| {
            ledger
                .entry
                .iter()
                .find(|entry| entry.id == case.move_ledger_entry)
                .ok_or_else(|| {
                    CargoAllowError::with_kind(
                        CargoAllowErrorKind::InvalidConfig,
                        format!("missing ledger entry `{}`", case.move_ledger_entry),
                    )
                })
        })
        .collect::<CargoAllowResult<Vec<_>>>()?;

    let package_names: BTreeSet<String> = entries
        .iter()
        .flat_map(|entry| [entry.current_crate.clone(), entry.target_crate.clone()])
        .collect();
    let architecture_packages: BTreeMap<_, _> = architecture
        .workspace_packages
        .iter()
        .map(|package| (package.cargo_package_name.as_str(), package))
        .collect();
    let package_paths = package_names
        .iter()
        .map(|package_name| {
            let package = architecture_packages
                .get(package_name.as_str())
                .ok_or_else(|| {
                    CargoAllowError::with_kind(
                        CargoAllowErrorKind::InvalidConfig,
                        format!("V2 topology has no package identity for `{package_name}`"),
                    )
                })?;
            relative_file(root, &root.join(&package.workspace_path).join("Cargo.toml"))
        })
        .collect::<CargoAllowResult<Vec<_>>>()?;

    let asset_paths = match stage {
        ExtractionStage::RepoSnapshot => effortless_repo_snapshot::parity_contract_paths(root),
        ExtractionStage::RepoEdit => effortless_repo_edit::parity_contract_paths(root),
        other => {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!(
                    "unsupported ownership derivation stage `{}`",
                    other.as_str()
                ),
            ));
        }
    }
    .into_iter()
    .map(|path| relative_file(root, &path))
    .collect::<CargoAllowResult<Vec<_>>>()?;
    let docs_paths = [
        "docs/architecture/extraction-parity.md",
        match stage {
            ExtractionStage::RepoSnapshot => "docs/architecture/repo-snapshot.md",
            ExtractionStage::RepoEdit => "docs/architecture/repo-edit.md",
            other => {
                return Err(CargoAllowError::with_kind(
                    CargoAllowErrorKind::InvalidConfig,
                    format!(
                        "unsupported ownership documentation stage `{}`",
                        other.as_str()
                    ),
                ));
            }
        },
        "policy/extraction-parity.toml",
        "policy/product-move-ledger.toml",
    ]
    .into_iter()
    .map(|path| relative_file(root, &root.join(path)))
    .collect::<CargoAllowResult<Vec<_>>>()?;
    let ci_paths = [
        ".github/workflows/ci.yml",
        "scripts/extraction-cutover-status.sh",
    ]
    .into_iter()
    .map(|path| relative_file(root, &root.join(path)))
    .collect::<CargoAllowResult<Vec<_>>>()?;

    Ok(DerivedOwnership {
        architecture_manifest_digest,
        package_paths,
        asset_paths,
        docs_paths,
        ci_paths,
        package_names,
    })
}

fn validate_ownership_receipt(
    root: &Path,
    receipt: &OwnershipEvidenceReceipt,
    stage: ExtractionStage,
    source_identity: &str,
    parity_digest: &str,
    expected: &DerivedOwnership,
) -> CargoAllowResult<String> {
    if receipt.schema_id != OWNERSHIP_EVIDENCE_SCHEMA_ID || receipt.schema_version != 1 {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            "unsupported extraction ownership receipt schema",
        ));
    }
    if receipt.stage != stage.as_str() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "ownership receipt stage `{}` does not match `{}`",
                receipt.stage,
                stage.as_str()
            ),
        ));
    }
    if receipt.source_identity != source_identity {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("ownership receipt source identity does not match `{source_identity}`"),
        ));
    }
    if receipt.parity_result_digest != parity_digest {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            "ownership receipt parity digest does not match runtime parity",
        ));
    }
    require_positive_outcome("ownership receipt result", &receipt.result)?;
    if receipt.claim_boundary.trim().is_empty() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            "ownership receipt claim_boundary is empty",
        ));
    }
    compare_owned_paths(
        "package_paths",
        root,
        &receipt.package_paths,
        &expected.package_paths,
    )?;
    compare_owned_paths(
        "asset_paths",
        root,
        &receipt.asset_paths,
        &expected.asset_paths,
    )?;
    compare_owned_paths(
        "docs_paths",
        root,
        &receipt.docs_paths,
        &expected.docs_paths,
    )?;
    compare_owned_paths("ci_paths", root, &receipt.ci_paths, &expected.ci_paths)?;
    let receipt_digest = serialized_receipt_digest(receipt)?;
    Ok(format!(
        "ownership_receipt:{};packages={}",
        receipt_digest,
        expected.package_names.len()
    ))
}

fn validate_build_package_receipt(
    root: &Path,
    receipt: &IndependentBuildPackageReceipt,
    stage: ExtractionStage,
    source_identity: &str,
    parity_digest: &str,
    expected: &DerivedOwnership,
) -> CargoAllowResult<String> {
    if receipt.schema_id != BUILD_PACKAGE_EVIDENCE_SCHEMA_ID || receipt.schema_version != 1 {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            "unsupported extraction build/package receipt schema",
        ));
    }
    if receipt.stage != stage.as_str()
        || receipt.source_identity != source_identity
        || receipt.architecture_manifest_digest != expected.architecture_manifest_digest
        || receipt.parity_result_digest != parity_digest
    {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            "independent build/package receipt is stale or for another stage",
        ));
    }
    if !receipt.independent || !receipt.source_checkout_denied {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            "build/package receipt does not prove independent source-checkout isolation",
        ));
    }
    require_positive_outcome("build/package receipt result", &receipt.result)?;
    if receipt.claim_boundary.trim().is_empty() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            "build/package receipt claim_boundary is empty",
        ));
    }
    let mut package_names = BTreeSet::new();
    for record in &receipt.package_records {
        if !package_names.insert(record.package_name.clone()) {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!("duplicate package receipt `{}`", record.package_name),
            ));
        }
        require_positive_outcome("package receipt result", &record.result)?;
        verify_receipt_file(root, &record.path, &record.sha256, "package")?;
    }
    if package_names != expected.package_names {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "package receipt set {:?} does not match topology {:?}",
                package_names, expected.package_names
            ),
        ));
    }
    if receipt.build_records.is_empty() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            "build/package receipt has no build records",
        ));
    }
    let mut artifact_names = BTreeSet::new();
    for record in &receipt.build_records {
        if !artifact_names.insert(record.artifact_name.clone()) {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!("duplicate build receipt `{}`", record.artifact_name),
            ));
        }
        require_positive_outcome("build receipt result", &record.result)?;
        verify_receipt_file(root, &record.path, &record.sha256, "build")?;
    }
    let receipt_digest = serialized_receipt_digest(receipt)?;
    Ok(format!(
        "build_package_receipt:{};packages={};artifacts={}",
        receipt_digest,
        package_names.len(),
        artifact_names.len()
    ))
}

fn serialized_receipt_digest<T: Serialize>(receipt: &T) -> CargoAllowResult<String> {
    let rendered = serde_json::to_vec(receipt)
        .map_err(|error| CargoAllowError::new(format!("render receipt digest input: {error}")))?;
    Ok(sha256_v1_bytes(&rendered))
}

fn compare_owned_paths(
    field: &str,
    root: &Path,
    observed: &[String],
    expected: &[String],
) -> CargoAllowResult<()> {
    let observed = observed
        .iter()
        .map(|path| relative_file(root, &root.join(path)))
        .collect::<CargoAllowResult<BTreeSet<_>>>()?;
    let expected: BTreeSet<_> = expected.iter().cloned().collect();
    if observed != expected {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("{field} does not match current topology-derived ownership"),
        ));
    }
    Ok(())
}

fn verify_receipt_file(
    root: &Path,
    relative: &str,
    expected_digest: &str,
    kind: &str,
) -> CargoAllowResult<()> {
    let path = root.join(relative);
    let normalized = relative_file(root, &path)?;
    if normalized != relative.replace('\\', "/") {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("{kind} receipt path `{relative}` is not canonical"),
        ));
    }
    let observed = sha256_v1_bytes(&fs::read(&path).map_err(|error| {
        CargoAllowError::new(format!(
            "read {kind} receipt artifact {}: {error}",
            path.display()
        ))
    })?);
    if observed != expected_digest {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("{kind} receipt digest mismatch for `{relative}`"),
        ));
    }
    Ok(())
}

fn resolve_receipt_path(root: &Path, relative: &Path) -> CargoAllowResult<PathBuf> {
    if relative.is_absolute() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            "cutover evidence receipt paths must be repository-relative",
        ));
    }
    let path = root.join(relative);
    relative_file(root, &path).map(|_| path)
}

fn resolve_evidence_path(root: &Path, path: &Path) -> CargoAllowResult<PathBuf> {
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    relative_file(root, &resolved).map(|_| resolved)
}

fn relative_file(root: &Path, path: &Path) -> CargoAllowResult<String> {
    let relative = path.strip_prefix(root).map_err(|_| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("path `{}` is outside the repository root", path.display()),
        )
    })?;
    if relative
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
        || !path.is_file()
    {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "owned path `{}` is missing or not a file",
                relative.display()
            ),
        ));
    }
    let canonical_root = fs::canonicalize(root)
        .map_err(|error| CargoAllowError::new(format!("canonicalize repository root: {error}")))?;
    let canonical_path = fs::canonicalize(path).map_err(|error| {
        CargoAllowError::new(format!(
            "canonicalize owned path {}: {error}",
            path.display()
        ))
    })?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "owned path `{}` escapes the repository root",
                path.display()
            ),
        ));
    }
    Ok(relative.to_string_lossy().replace('\\', "/"))
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
            "Cargo.toml",
            "crates/*/Cargo.toml",
            "crates/allow-policy/src/extraction_parity",
            "crates/cargo-allow/src/extraction_parity_command.rs",
            "policy/product-crates-v2.toml",
            "policy/product-package-topology-v2.toml",
            "policy/extraction-parity.toml",
            "policy/product-move-ledger.toml",
        ])
        .output()
        .map_err(|error| {
            CargoAllowError::new(format!("run git status for cutover inputs: {error}"))
        })?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn fixture_root(name: &str) -> PathBuf {
        workspace_root()
            .join("target")
            .join(format!("extraction-cutover-{name}-{}", std::process::id()))
    }

    fn write_json(path: &Path, value: &impl Serialize) -> CargoAllowResult<()> {
        let contents = serde_json::to_vec_pretty(value)
            .map_err(|error| CargoAllowError::new(format!("render test evidence: {error}")))?;
        fs::write(path, contents).map_err(|error| {
            CargoAllowError::new(format!("write test evidence {}: {error}", path.display()))
        })
    }

    fn stage_inputs(
        stage: ParityStageArg,
    ) -> CargoAllowResult<(
        ExtractionStage,
        ExtractionParityRegistry,
        ProductMoveLedger,
        String,
        DerivedOwnership,
    )> {
        let root = workspace_root();
        let registry = load_registry(&root)?;
        let ledger = load_ledger(&root)?;
        let extraction_stage = match stage {
            ParityStageArg::RepoSnapshot => ExtractionStage::RepoSnapshot,
            ParityStageArg::RepoEdit => ExtractionStage::RepoEdit,
            ParityStageArg::All => {
                return Err(CargoAllowError::new("test stage must be stage-specific"));
            }
        };
        let source = source_identity(&root)?;
        let ownership = derive_ownership(&root, &registry, &ledger, extraction_stage)?;
        Ok((extraction_stage, registry, ledger, source, ownership))
    }

    fn ownership_receipt(
        stage: ExtractionStage,
        source: &str,
        parity: &str,
        ownership: &DerivedOwnership,
        result: &str,
    ) -> OwnershipEvidenceReceipt {
        OwnershipEvidenceReceipt {
            schema_id: OWNERSHIP_EVIDENCE_SCHEMA_ID.to_string(),
            schema_version: 1,
            stage: stage.as_str().to_string(),
            source_identity: source.to_string(),
            parity_result_digest: parity.to_string(),
            result: result.to_string(),
            package_paths: ownership.package_paths.clone(),
            asset_paths: ownership.asset_paths.clone(),
            docs_paths: ownership.docs_paths.clone(),
            ci_paths: ownership.ci_paths.clone(),
            claim_boundary: "Current topology-derived ownership paths".to_string(),
        }
    }

    fn build_receipt(
        root: &Path,
        dir: &Path,
        stage: ExtractionStage,
        source: &str,
        parity: &str,
        ownership: &DerivedOwnership,
        result: &str,
    ) -> CargoAllowResult<IndependentBuildPackageReceipt> {
        let mut package_records = Vec::new();
        for package_name in &ownership.package_names {
            let path = dir.join(format!("{package_name}.crate"));
            fs::write(&path, format!("package:{package_name}").as_bytes())
                .map_err(|error| CargoAllowError::new(format!("write package fixture: {error}")))?;
            package_records.push(PackageEvidenceRecord {
                package_name: package_name.clone(),
                path: path
                    .strip_prefix(root)
                    .map_err(|error| {
                        CargoAllowError::new(format!("package fixture path: {error}"))
                    })?
                    .to_string_lossy()
                    .replace('\\', "/"),
                sha256: sha256_v1_bytes(&fs::read(&path).map_err(|error| {
                    CargoAllowError::new(format!("read package fixture: {error}"))
                })?),
                result: result.to_string(),
            });
        }
        let build_path = dir.join("build-artifact.bin");
        fs::write(&build_path, b"build-artifact")
            .map_err(|error| CargoAllowError::new(format!("write build fixture: {error}")))?;
        Ok(IndependentBuildPackageReceipt {
            schema_id: BUILD_PACKAGE_EVIDENCE_SCHEMA_ID.to_string(),
            schema_version: 1,
            stage: stage.as_str().to_string(),
            source_identity: source.to_string(),
            architecture_manifest_digest: ownership.architecture_manifest_digest.clone(),
            parity_result_digest: parity.to_string(),
            result: result.to_string(),
            independent: true,
            source_checkout_denied: true,
            package_records,
            build_records: vec![BuildEvidenceRecord {
                artifact_name: format!("{}-build", stage.as_str()),
                path: build_path
                    .strip_prefix(root)
                    .map_err(|error| CargoAllowError::new(format!("build fixture path: {error}")))?
                    .to_string_lossy()
                    .replace('\\', "/"),
                sha256: sha256_v1_bytes(b"build-artifact"),
                result: result.to_string(),
            }],
            claim_boundary: "Independent package and build artifacts".to_string(),
        })
    }

    fn write_bundle(
        name: &str,
        source: &str,
        parity: &str,
        ownership_result: &str,
        build_result: &str,
    ) -> CargoAllowResult<(PathBuf, PathBuf)> {
        let root = workspace_root();
        let (stage, _registry, _ledger, _current, ownership) =
            stage_inputs(ParityStageArg::RepoSnapshot)?;
        let dir = fixture_root(name);
        fs::create_dir_all(&dir)
            .map_err(|error| CargoAllowError::new(format!("create fixture root: {error}")))?;
        let ownership_value =
            ownership_receipt(stage, source, parity, &ownership, ownership_result);
        let ownership_path = dir.join("ownership.json");
        write_json(&ownership_path, &ownership_value)?;
        let build_value =
            build_receipt(&root, &dir, stage, source, parity, &ownership, build_result)?;
        let build_path = dir.join("build-package.json");
        write_json(&build_path, &build_value)?;
        let relative = |path: &Path| {
            path.strip_prefix(&root)
                .map(|path| path.to_string_lossy().replace('\\', "/"))
                .map_err(|error| CargoAllowError::new(format!("fixture path: {error}")))
        };
        let manifest_path = dir.join("manifest.json");
        write_json(
            &manifest_path,
            &json!({
                "schema_id": CUTOVER_EVIDENCE_SCHEMA_ID,
                "schema_version": CUTOVER_EVIDENCE_SCHEMA_VERSION,
                "ownership_receipt": relative(&ownership_path)?,
                "independent_build_package_receipt": relative(&build_path)?
            }),
        )?;
        Ok((manifest_path, dir))
    }

    fn remove_fixture(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }

    fn expect_error(result: CargoAllowResult<ExtractionCutoverReceipt>) -> CargoAllowError {
        match result {
            Ok(_) => CargoAllowError::new("expected cutover evidence rejection"),
            Err(error) => error,
        }
    }

    #[test]
    fn cutover_evidence_rejects_stale_source_identity() -> Result<(), String> {
        let root = workspace_root();
        let registry = load_registry(&root).map_err(|error| error.to_string())?;
        let current = source_identity(&root).map_err(|error| error.to_string())?;
        let (path, fixture) = write_bundle(
            "stale-source",
            "commit:stale/tree:stale",
            "sha256:parity",
            "passed",
            "passed",
        )
        .map_err(|error| error.to_string())?;
        let result = assemble_cutover_receipt_from_clean_inputs(
            &root,
            &registry,
            ParityStageArg::RepoSnapshot,
            "sha256:parity",
            &current,
            &path,
            true,
        );
        remove_fixture(&fixture);
        let message = expect_error(result).to_string();
        if !message.contains("source identity") {
            return Err(format!("stale identity was not reported: {message}"));
        }
        Ok(())
    }

    #[test]
    fn cutover_evidence_rejects_all_stage_before_reading_input() -> Result<(), String> {
        let root = workspace_root();
        let registry = load_registry(&root).map_err(|error| error.to_string())?;
        let current = source_identity(&root).map_err(|error| error.to_string())?;
        let result = assemble_cutover_receipt_from_clean_inputs(
            &root,
            &registry,
            ParityStageArg::All,
            "sha256:parity",
            &current,
            Path::new("missing-cutover-evidence.json"),
            true,
        );
        let message = expect_error(result).to_string();
        if !message.contains("one extraction stage") {
            return Err(format!("all-stage rejection was not reported: {message}"));
        }
        Ok(())
    }

    #[test]
    fn cutover_evidence_rejects_incomplete_ownership_and_build_results() -> Result<(), String> {
        let root = workspace_root();
        let registry = load_registry(&root).map_err(|error| error.to_string())?;
        let current = source_identity(&root).map_err(|error| error.to_string())?;
        for (name, ownership, build, expected) in [
            ("ownership", "missing", "passed", "ownership receipt result"),
            ("build", "passed", "missing", "build/package receipt result"),
        ] {
            let (path, fixture) = write_bundle(name, &current, "sha256:parity", ownership, build)
                .map_err(|error| error.to_string())?;
            let result = assemble_cutover_receipt_from_clean_inputs(
                &root,
                &registry,
                ParityStageArg::RepoSnapshot,
                "sha256:parity",
                &current,
                &path,
                true,
            );
            remove_fixture(&fixture);
            let message = expect_error(result).to_string();
            if !message.contains(expected) {
                return Err(format!("{name} rejection was not reported: {message}"));
            }
        }
        Ok(())
    }

    #[test]
    fn ownership_is_derived_from_current_topology_and_receipt_paths() -> Result<(), String> {
        let root = workspace_root();
        for requested_stage in [ParityStageArg::RepoSnapshot, ParityStageArg::RepoEdit] {
            let (stage, _registry, _ledger, source, ownership) =
                stage_inputs(requested_stage).map_err(|error| error.to_string())?;
            let receipt = ownership_receipt(stage, &source, "sha256:parity", &ownership, "Passed");
            let result = validate_ownership_receipt(
                &root,
                &receipt,
                stage,
                &source,
                "sha256:parity",
                &ownership,
            )
            .map_err(|error| error.to_string())?;
            if !result.contains("ownership_receipt:") || ownership.package_paths.is_empty() {
                return Err(format!("unexpected ownership result: {result}"));
            }
        }
        Ok(())
    }

    #[test]
    fn ownership_rejects_contradictory_current_paths() -> Result<(), String> {
        let root = workspace_root();
        let (stage, _registry, _ledger, source, ownership) =
            stage_inputs(ParityStageArg::RepoSnapshot).map_err(|error| error.to_string())?;
        let mut receipt = ownership_receipt(stage, &source, "sha256:parity", &ownership, "Passed");
        receipt.package_paths.push("README.md".to_string());
        let error = validate_ownership_receipt(
            &root,
            &receipt,
            stage,
            &source,
            "sha256:parity",
            &ownership,
        )
        .err()
        .ok_or_else(|| "contradictory ownership paths were accepted".to_string())?;
        if !error.to_string().contains("package_paths") {
            return Err(format!("wrong ownership contradiction: {error}"));
        }
        Ok(())
    }

    #[test]
    fn independent_build_package_receipt_binds_artifact_digests() -> Result<(), String> {
        let root = workspace_root();
        let (stage, _registry, _ledger, source, ownership) =
            stage_inputs(ParityStageArg::RepoSnapshot).map_err(|error| error.to_string())?;
        let fixture = fixture_root("build-positive");
        fs::create_dir_all(&fixture).map_err(|error| error.to_string())?;
        let receipt = build_receipt(
            &root,
            &fixture,
            stage,
            &source,
            "sha256:parity",
            &ownership,
            "Passed",
        )
        .map_err(|error| error.to_string())?;
        let validation = validate_build_package_receipt(
            &root,
            &receipt,
            stage,
            &source,
            "sha256:parity",
            &ownership,
        );
        remove_fixture(&fixture);
        let result = validation.map_err(|error| error.to_string())?;
        if !result.contains("build_package_receipt:") {
            return Err(format!("unexpected build/package result: {result}"));
        }
        Ok(())
    }

    #[test]
    fn independent_build_package_receipt_rejects_stale_digest() -> Result<(), String> {
        let root = workspace_root();
        let (stage, _registry, _ledger, source, ownership) =
            stage_inputs(ParityStageArg::RepoSnapshot).map_err(|error| error.to_string())?;
        let fixture = fixture_root("build-stale");
        fs::create_dir_all(&fixture).map_err(|error| error.to_string())?;
        let mut receipt = build_receipt(
            &root,
            &fixture,
            stage,
            &source,
            "sha256:parity",
            &ownership,
            "Passed",
        )
        .map_err(|error| error.to_string())?;
        if let Some(record) = receipt.package_records.first_mut() {
            record.sha256 = "sha256:v1:stale".to_string();
        }
        let validation = validate_build_package_receipt(
            &root,
            &receipt,
            stage,
            &source,
            "sha256:parity",
            &ownership,
        );
        remove_fixture(&fixture);
        let error = validation
            .err()
            .ok_or_else(|| "stale package digest was accepted".to_string())?;
        if !error
            .to_string()
            .contains("package receipt digest mismatch")
        {
            return Err(format!("wrong stale digest error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn independent_build_package_receipt_rejects_stale_architecture_digest() -> Result<(), String> {
        let root = workspace_root();
        let (stage, _registry, _ledger, source, ownership) =
            stage_inputs(ParityStageArg::RepoSnapshot).map_err(|error| error.to_string())?;
        let fixture = fixture_root("build-stale-architecture");
        fs::create_dir_all(&fixture).map_err(|error| error.to_string())?;
        let mut receipt = build_receipt(
            &root,
            &fixture,
            stage,
            &source,
            "sha256:parity",
            &ownership,
            "Passed",
        )
        .map_err(|error| error.to_string())?;
        receipt.architecture_manifest_digest = "sha256:v1:stale-architecture".to_string();
        let validation = validate_build_package_receipt(
            &root,
            &receipt,
            stage,
            &source,
            "sha256:parity",
            &ownership,
        );
        remove_fixture(&fixture);
        let error = validation
            .err()
            .ok_or_else(|| "stale architecture digest was accepted".to_string())?;
        if !error.to_string().contains("stale or for another stage") {
            return Err(format!("wrong stale architecture error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn negative_cutover_outcomes_are_not_positive_evidence() -> Result<(), String> {
        for value in ["", "missing", "failed", "blocked", "unknown"] {
            if require_positive_outcome("evidence", value).is_ok() {
                return Err(format!("negative outcome `{value}` was accepted"));
            }
        }
        Ok(())
    }
}
