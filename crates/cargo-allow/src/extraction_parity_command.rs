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
    let cwd = current_dir()?;
    let root = resolve_source_tree_root(args.root.root.as_deref(), cwd.clone())?;
    let registry = load_registry(&root)?;
    let initial_cutover_source_inputs = if args.cutover_evidence.is_some() {
        let stage = requested_extraction_stage(args.stage)?;
        let ledger = load_ledger(&root)?;
        let ownership = derive_ownership(&root, &registry, &ledger, stage)?;
        ensure_cutover_inputs_clean(&root, &ownership.source_input_paths)?;
        ensure_cutover_output_separate(
            &root,
            &cwd,
            args.output.as_deref(),
            &ownership.source_input_paths,
        )?;
        Some(ownership.source_input_paths)
    } else {
        None
    };
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
    if let Some(source_input_paths) = initial_cutover_source_inputs.as_deref() {
        ensure_cutover_inputs_clean(&root, source_input_paths)?;
    }

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
    let mut cutover_source_input_paths = None;
    if let Some(evidence_path) = args.cutover_evidence.as_deref() {
        let assembly = assemble_cutover_receipt(
            &root,
            &registry,
            args.stage,
            &digest,
            &initial_source_identity,
            evidence_path,
            passed,
        )?;
        let receipt_value = serde_json::to_value(assembly.receipt).map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Artifact,
                format!("render extraction cutover receipt: {error}"),
            )
        })?;
        let object = payload.as_object_mut().ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Artifact,
                "render extraction cutover receipt requires JSON object payload",
            )
        })?;
        object.insert("cutover_receipt".to_string(), receipt_value);
        cutover_source_input_paths = Some(assembly.source_input_paths);
    }
    let rendered = serde_json::to_string_pretty(&payload).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!("render parity evidence: {error}"),
        )
    })? + "\n";
    if let Some(source_input_paths) = cutover_source_input_paths.as_deref() {
        ensure_cutover_inputs_clean(&root, source_input_paths)?;
    }
    emit_text(args.output.as_deref(), &rendered)?;
    if let Some(source_input_paths) = cutover_source_input_paths.as_deref() {
        ensure_cutover_inputs_clean(&root, source_input_paths)?;
    }
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

#[derive(Debug, Deserialize, Serialize)]
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
    source_input_paths: Vec<String>,
}

struct CutoverAssembly {
    receipt: ExtractionCutoverReceipt,
    source_input_paths: Vec<String>,
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
) -> CargoAllowResult<CutoverAssembly> {
    let stage = requested_extraction_stage(requested_stage)?;
    let ledger = load_ledger(root)?;
    let derived_ownership = derive_ownership(root, registry, &ledger, stage)?;
    ensure_cutover_inputs_clean(root, &derived_ownership.source_input_paths)?;
    let receipt = assemble_cutover_receipt_from_clean_inputs(
        root,
        registry,
        requested_stage,
        parity_digest,
        source_identity,
        evidence_path,
        parity_passed,
    )?;
    ensure_cutover_inputs_clean(root, &derived_ownership.source_input_paths)?;
    Ok(CutoverAssembly {
        receipt,
        source_input_paths: derived_ownership.source_input_paths,
    })
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
    let stage = requested_extraction_stage(requested_stage)?;
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
        "scripts/test-extraction-cutover-status.sh",
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
        source_input_paths: cutover_source_input_paths(root, &architecture, stage)?,
    })
}

fn requested_extraction_stage(requested: ParityStageArg) -> CargoAllowResult<ExtractionStage> {
    match requested {
        ParityStageArg::RepoSnapshot => Ok(ExtractionStage::RepoSnapshot),
        ParityStageArg::RepoEdit => Ok(ExtractionStage::RepoEdit),
        ParityStageArg::All => Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            "cutover receipts require one extraction stage, not `all`",
        )),
    }
}

fn cutover_source_input_paths(
    root: &Path,
    architecture: &allow_policy::product_crates::CurrentArchitectureReceiptV2,
    stage: ExtractionStage,
) -> CargoAllowResult<Vec<String>> {
    let mut paths = BTreeSet::from([
        "Cargo.toml".to_string(),
        "policy/extraction-parity.toml".to_string(),
        "policy/product-crates-v2.toml".to_string(),
        "policy/product-package-topology-v2.toml".to_string(),
        "policy/product-move-ledger.toml".to_string(),
        "crates/allow-policy/src/extraction_parity".to_string(),
        "crates/allow-policy/src/product_crates".to_string(),
        "crates/allow-policy/src/product_packages".to_string(),
        "crates/allow-policy/src/product_move".to_string(),
        "crates/cargo-allow/src/extraction_parity_command.rs".to_string(),
        "scripts/extraction-cutover-status.sh".to_string(),
        "scripts/test-extraction-cutover-status.sh".to_string(),
    ]);
    let runtime_path = match stage {
        ExtractionStage::RepoSnapshot => {
            paths.insert("crates/allow-diff/src/snapshot_package".to_string());
            paths.insert("crates/effortless-repo-snapshot/src".to_string());
            "crates/cargo-allow/src/extraction_parity_runtime.rs".to_string()
        }
        ExtractionStage::RepoEdit => {
            paths.insert("crates/effortless-repo-edit/src".to_string());
            "crates/cargo-allow/src/extraction_repo_edit_runtime.rs".to_string()
        }
        other => {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!(
                    "unsupported cutover source-input stage `{}`",
                    other.as_str()
                ),
            ));
        }
    };
    paths.insert(runtime_path);
    for path in &paths {
        if !root.join(path).exists() {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!("cutover source input `{path}` no longer exists in the source tree"),
            ));
        }
    }
    for package in &architecture.workspace_packages {
        paths.insert(relative_file(
            root,
            &root.join(&package.workspace_path).join("Cargo.toml"),
        )?);
    }
    let parity_paths = match stage {
        ExtractionStage::RepoSnapshot => effortless_repo_snapshot::parity_contract_paths(root),
        ExtractionStage::RepoEdit => effortless_repo_edit::parity_contract_paths(root),
        other => {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!(
                    "unsupported cutover parity-input stage `{}`",
                    other.as_str()
                ),
            ));
        }
    };
    for path in parity_paths {
        paths.insert(relative_file(root, &path)?);
    }
    Ok(paths.into_iter().collect())
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

fn ensure_cutover_inputs_clean(root: &Path, source_input_paths: &[String]) -> CargoAllowResult<()> {
    let missing = source_input_paths
        .iter()
        .filter(|path| !root.join(path).exists())
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "cutover receipt requires all watched source inputs; missing paths:\n{}",
                missing.join("\n")
            ),
        ));
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain", "--untracked-files=all", "--"])
        .args(source_input_paths)
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

fn ensure_cutover_output_separate(
    root: &Path,
    cwd: &Path,
    output: Option<&Path>,
    source_input_paths: &[String],
) -> CargoAllowResult<()> {
    let Some(output) = output else {
        return Ok(());
    };
    let output = canonicalize_allow_missing(&if output.is_absolute() {
        output.to_path_buf()
    } else {
        cwd.join(output)
    })?;
    for source in source_input_paths {
        let source = root.join(source);
        let source_is_dir = source.is_dir();
        let source = canonicalize_allow_missing(&source)?;
        if output == source || source_is_dir && output.starts_with(&source) {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!(
                    "cutover evidence output `{}` overlaps watched source input `{}`",
                    output.display(),
                    source.display()
                ),
            ));
        }
    }
    Ok(())
}

fn canonicalize_allow_missing(path: &Path) -> CargoAllowResult<PathBuf> {
    let mut cursor = path;
    let mut suffix = Vec::new();
    while !cursor.exists() {
        let name = cursor.file_name().ok_or_else(|| {
            CargoAllowError::new(format!(
                "cannot resolve output path outside an existing filesystem root: {}",
                path.display()
            ))
        })?;
        suffix.push(name.to_os_string());
        cursor = cursor.parent().ok_or_else(|| {
            CargoAllowError::new(format!("output path has no parent: {}", path.display()))
        })?;
    }
    let mut resolved = fs::canonicalize(cursor).map_err(|error| {
        CargoAllowError::new(format!("canonicalize path {}: {error}", cursor.display()))
    })?;
    for component in suffix.into_iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
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

    fn assert_invalid<T>(
        case: &str,
        result: CargoAllowResult<T>,
        expected: &str,
    ) -> Result<(), String> {
        let error = result.err().ok_or_else(|| format!("{case} was accepted"))?;
        if !error.to_string().contains(expected) {
            return Err(format!(
                "{case} returned the wrong diagnostic: expected `{expected}`, got `{error}`"
            ));
        }
        Ok(())
    }

    fn assert_schema_accepts_and_rejects_shape(
        name: &str,
        schema_text: &str,
        valid: Value,
    ) -> Result<(), String> {
        let schema: Value =
            serde_json::from_str(schema_text).map_err(|error| format!("{name} schema: {error}"))?;
        let validator =
            jsonschema::validator_for(&schema).map_err(|error| format!("{name}: {error}"))?;
        validator
            .validate(&valid)
            .map_err(|error| format!("valid {name} sample rejected: {error}"))?;

        let mut unknown = valid.clone();
        unknown
            .as_object_mut()
            .ok_or_else(|| format!("{name} sample is not an object"))?
            .insert("unknown".to_string(), Value::Bool(true));
        if validator.validate(&unknown).is_ok() {
            return Err(format!("{name} schema accepted an unknown field"));
        }

        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("{name} schema has no required fields"))?;
        for required in required {
            let required = required
                .as_str()
                .ok_or_else(|| format!("{name} required field is not a string"))?;
            let mut missing = valid.clone();
            missing
                .as_object_mut()
                .ok_or_else(|| format!("{name} sample is not an object"))?
                .remove(required);
            if validator.validate(&missing).is_ok() {
                return Err(format!("{name} schema accepted missing `{required}`"));
            }
        }
        Ok(())
    }

    #[test]
    fn extraction_cutover_input_schemas_are_closed_and_conformant() -> Result<(), String> {
        let source = format!("commit:{}/tree:{}", "a".repeat(40), "b".repeat(40));
        assert_schema_accepts_and_rejects_shape(
            "extraction cutover evidence",
            ::std::include_str!("../../../docs/schemas/extraction-cutover-evidence.schema.json"),
            json!({
                "schema_id": CUTOVER_EVIDENCE_SCHEMA_ID,
                "schema_version": CUTOVER_EVIDENCE_SCHEMA_VERSION,
                "ownership_receipt": "target/extraction/ownership.json",
                "independent_build_package_receipt": "target/extraction/build.json"
            }),
        )?;
        let ownership_sample = json!({
            "schema_id": OWNERSHIP_EVIDENCE_SCHEMA_ID,
            "schema_version": 1,
            "stage": "RepoSnapshot",
            "source_identity": source,
            "parity_result_digest": "sha256:v1:parity",
            "result": "Passed",
            "package_paths": ["crates/effortless-repo-snapshot/Cargo.toml"],
            "asset_paths": ["tests/fixtures/repo-snapshot/parity-committed-head-v1.toml"],
            "docs_paths": ["docs/architecture/repo-snapshot.md"],
            "ci_paths": ["scripts/extraction-cutover-status.sh"],
            "claim_boundary": "topology-derived ownership"
        });
        assert_schema_accepts_and_rejects_shape(
            "extraction cutover ownership",
            ::std::include_str!("../../../docs/schemas/extraction-cutover-ownership.schema.json"),
            ownership_sample.clone(),
        )?;
        let ownership_schema: Value = serde_json::from_str(::std::include_str!(
            "../../../docs/schemas/extraction-cutover-ownership.schema.json"
        ))
        .map_err(|error| error.to_string())?;
        let ownership_validator =
            jsonschema::validator_for(&ownership_schema).map_err(|error| error.to_string())?;
        for field in ["package_paths", "asset_paths", "docs_paths", "ci_paths"] {
            let mut empty = ownership_sample.clone();
            empty
                .as_object_mut()
                .ok_or_else(|| "ownership sample is not an object".to_string())?
                .insert(field.to_string(), json!([]));
            if ownership_validator.validate(&empty).is_ok() {
                return Err(format!("ownership schema accepted empty `{field}`"));
            }
        }
        assert_schema_accepts_and_rejects_shape(
            "extraction cutover build/package",
            ::std::include_str!(
                "../../../docs/schemas/extraction-cutover-build-package.schema.json"
            ),
            json!({
                "schema_id": BUILD_PACKAGE_EVIDENCE_SCHEMA_ID,
                "schema_version": 1,
                "stage": "RepoSnapshot",
                "source_identity": source,
                "architecture_manifest_digest": "sha256:v1:architecture",
                "parity_result_digest": "sha256:v1:parity",
                "result": "Passed",
                "independent": true,
                "source_checkout_denied": true,
                "package_records": [{
                    "package_name": "effortless-repo-snapshot",
                    "path": "target/package/effortless-repo-snapshot.crate",
                    "sha256": "sha256:v1:package",
                    "result": "Passed"
                }],
                "build_records": [{
                    "artifact_name": "repo-snapshot-build",
                    "path": "target/build/repo-snapshot.bin",
                    "sha256": "sha256:v1:build",
                    "result": "Passed"
                }],
                "claim_boundary": "isolated build/package evidence"
            }),
        )
    }

    #[test]
    fn parity_command_emits_complete_repo_edit_runtime_evidence() -> Result<(), String> {
        let root = workspace_root();
        let fixture = fixture_root("command-repo-edit");
        fs::create_dir_all(&fixture).map_err(|error| error.to_string())?;
        let output = fixture.join("runtime.json");
        let args = ParityArgs {
            root: RootArgs {
                root: Some(root.clone()),
            },
            stage: ParityStageArg::RepoEdit,
            output: Some(output.clone()),
            cutover_evidence: None,
        };
        let result = cmd_parity(&args);
        let payload = fs::read_to_string(&output)
            .map_err(|error| format!("read command runtime evidence: {error}"))?;
        remove_fixture(&fixture);
        result.map_err(|error| error.to_string())?;
        let payload: Value = serde_json::from_str(&payload).map_err(|error| error.to_string())?;
        if payload.get("result").and_then(Value::as_str) != Some("Passed")
            || payload.get("completeness").and_then(Value::as_str) != Some("Complete")
            || payload.get("stage").and_then(Value::as_str) != Some("RepoEdit")
        {
            return Err(format!("unexpected repo-edit runtime evidence: {payload}"));
        }
        Ok(())
    }

    fn run_test_git(root: &Path, args: &[&str]) -> Result<(), String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .map_err(|error| format!("run test git {args:?}: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "test git {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(())
    }

    fn clean_guard_fixture(name: &str, tracked_path: &str) -> Result<PathBuf, String> {
        let root = fixture_root(name);
        fs::create_dir_all(
            root.join(tracked_path)
                .parent()
                .ok_or_else(|| "tracked fixture path has no parent".to_string())?,
        )
        .map_err(|error| error.to_string())?;
        fs::write(root.join(tracked_path), "committed\n").map_err(|error| error.to_string())?;
        run_test_git(&root, &["init", "--quiet"])?;
        run_test_git(
            &root,
            &["config", "user.email", "tests@cargo-allow.invalid"],
        )?;
        run_test_git(&root, &["config", "user.name", "cargo-allow tests"])?;
        run_test_git(&root, &["add", "--", tracked_path])?;
        run_test_git(&root, &["commit", "--quiet", "-m", "fixture"])?;
        Ok(root)
    }

    #[test]
    fn cutover_clean_guard_rejects_index_worktree_and_untracked_derivation_inputs()
    -> Result<(), String> {
        for (name, path, prepare) in [
            ("guard-unstaged", "Cargo.toml", "unstaged"),
            (
                "guard-staged",
                "policy/product-package-topology-v2.toml",
                "staged",
            ),
            (
                "guard-untracked-policy",
                "policy/product-crates-v2.toml",
                "untracked",
            ),
            (
                "guard-untracked-manifest",
                "crates/new-member/Cargo.toml",
                "untracked",
            ),
            (
                "guard-command",
                "crates/cargo-allow/src/extraction_parity_command.rs",
                "unstaged",
            ),
            (
                "guard-asset",
                "tests/fixtures/repo-snapshot/parity-committed-head-v1.toml",
                "staged",
            ),
            (
                "guard-stage-source",
                "crates/effortless-repo-snapshot/src/lib.rs",
                "unstaged",
            ),
        ] {
            let committed_path = if prepare == "untracked" {
                "README.md"
            } else {
                path
            };
            let root = clean_guard_fixture(name, committed_path)?;
            let paths = vec![path.to_string()];
            if prepare != "untracked" {
                ensure_cutover_inputs_clean(&root, &paths).map_err(|error| error.to_string())?;
            }
            if let Some(parent) = root.join(path).parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::write(root.join(path), format!("{prepare} change\n"))
                .map_err(|error| error.to_string())?;
            if prepare == "staged" {
                run_test_git(&root, &["add", "--", path])?;
            }
            let error = ensure_cutover_inputs_clean(&root, &paths)
                .err()
                .ok_or_else(|| format!("{prepare} derivation input `{path}` was accepted"))?;
            if !error.to_string().contains(path) {
                remove_fixture(&root);
                return Err(format!("dirty-input diagnostic omitted `{path}`: {error}"));
            }
            remove_fixture(&root);
        }
        Ok(())
    }

    #[test]
    fn cutover_clean_guard_rejects_missing_watched_input() -> Result<(), String> {
        let path = "policy/product-crates-v2.toml";
        let root = clean_guard_fixture("guard-missing", path)?;
        fs::remove_file(root.join(path)).map_err(|error| error.to_string())?;
        let result = ensure_cutover_inputs_clean(&root, &[path.to_string()]);
        remove_fixture(&root);
        let error = result
            .err()
            .ok_or_else(|| "missing watched input was accepted".to_string())?;
        if !error.to_string().contains(path) {
            return Err(format!(
                "missing-input diagnostic omitted `{path}`: {error}"
            ));
        }
        Ok(())
    }

    #[test]
    fn cutover_output_rejects_watched_files_and_directories() -> Result<(), String> {
        let root = fixture_root("output-separation");
        fs::create_dir_all(root.join("watched/directory")).map_err(|error| error.to_string())?;
        fs::create_dir_all(root.join("target")).map_err(|error| error.to_string())?;
        fs::write(root.join("watched/input.toml"), "source\n")
            .map_err(|error| error.to_string())?;
        let watched = vec![
            "watched/input.toml".to_string(),
            "watched/directory".to_string(),
        ];
        for output in [
            Path::new("watched/input.toml"),
            Path::new("watched/directory/new.json"),
        ] {
            if ensure_cutover_output_separate(&root, &root, Some(output), &watched).is_ok() {
                remove_fixture(&root);
                return Err(format!(
                    "watched output alias was accepted: {}",
                    output.display()
                ));
            }
        }
        ensure_cutover_output_separate(
            &root,
            &root,
            Some(Path::new("target/runtime.json")),
            &watched,
        )
        .map_err(|error| error.to_string())?;
        remove_fixture(&root);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cutover_output_rejects_symlink_alias_to_watched_directory() -> Result<(), String> {
        use std::os::unix::fs::symlink;

        let root = fixture_root("output-symlink");
        fs::create_dir_all(root.join("watched/directory")).map_err(|error| error.to_string())?;
        symlink(root.join("watched/directory"), root.join("alias"))
            .map_err(|error| error.to_string())?;
        let watched = vec!["watched/directory".to_string()];
        let result = ensure_cutover_output_separate(
            &root,
            &root,
            Some(Path::new("alias/new.json")),
            &watched,
        );
        remove_fixture(&root);
        if result.is_ok() {
            return Err("symlink alias to watched directory was accepted".to_string());
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cutover_status_adapter_fails_closed_for_receipt_location_and_lifecycle() -> Result<(), String>
    {
        use std::os::unix::fs::PermissionsExt;
        struct FixtureGuard(PathBuf);

        impl Drop for FixtureGuard {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        fn run_status(root: &Path, output: &Path, bin: &Path) -> Result<Value, String> {
            let current_path = std::env::var_os("PATH").unwrap_or_default();
            let mut paths = vec![bin.to_path_buf()];
            paths.extend(std::env::split_paths(&current_path));
            let joined = std::env::join_paths(paths).map_err(|error| error.to_string())?;
            let result = Command::new("bash")
                .arg("scripts/extraction-cutover-status.sh")
                .current_dir(root)
                .env("PATH", joined)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("EXTRACTION_CUTOVER_DIR", output)
                .output()
                .map_err(|error| format!("run extraction cutover status: {error}"))?;
            if !result.status.success() {
                return Err(format!(
                    "extraction cutover status failed: {}",
                    String::from_utf8_lossy(&result.stderr)
                ));
            }
            let text = fs::read_to_string(output.join("extraction-cutover-status.json"))
                .map_err(|error| format!("read cutover status: {error}"))?;
            serde_json::from_str(&text).map_err(|error| format!("parse cutover status: {error}"))
        }

        fn blockers(status: &Value) -> Result<Vec<&str>, String> {
            status
                .get("blockers")
                .and_then(Value::as_array)
                .ok_or_else(|| "cutover status has no blockers".to_string())?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .ok_or_else(|| "cutover blocker is not a string".to_string())
                })
                .collect()
        }

        let source_root = workspace_root();
        let fixture = fixture_root("status-adapter");
        remove_fixture(&fixture);
        let _fixture_guard = FixtureGuard(fixture.clone());
        let root = fixture.join("repo[status]");
        let bin = fixture.join("bin");
        fs::create_dir_all(&bin).map_err(|error| error.to_string())?;

        let mut repository_files = BTreeSet::from([
            "scripts/extraction-cutover-status.sh".to_string(),
            "policy/extraction-parity.toml".to_string(),
            "policy/product-move-ledger.toml".to_string(),
            "policy/product-crates-v2.toml".to_string(),
        ]);
        for requested_stage in [ParityStageArg::RepoSnapshot, ParityStageArg::RepoEdit] {
            let (_stage, _registry, _ledger, _source, ownership) =
                stage_inputs(requested_stage).map_err(|error| error.to_string())?;
            repository_files.extend(ownership.package_paths);
            repository_files.extend(ownership.asset_paths);
            repository_files.extend(ownership.docs_paths);
            repository_files.extend(ownership.ci_paths);
        }
        for relative in repository_files {
            let source = source_root.join(&relative);
            let destination = root.join(&relative);
            fs::create_dir_all(
                destination
                    .parent()
                    .ok_or_else(|| format!("repository fixture path has no parent: {relative}"))?,
            )
            .map_err(|error| error.to_string())?;
            fs::copy(&source, &destination).map_err(|error| {
                format!(
                    "copy repository fixture {} to {}: {error}",
                    source.display(),
                    destination.display()
                )
            })?;
        }
        run_test_git(&root, &["init", "--quiet"])?;
        run_test_git(
            &root,
            &["config", "user.email", "tests@cargo-allow.invalid"],
        )?;
        run_test_git(&root, &["config", "user.name", "cargo-allow tests"])?;
        run_test_git(&root, &["add", "."])?;
        run_test_git(&root, &["commit", "--quiet", "-m", "fixture"])?;

        let fake_cargo = bin.join("cargo");
        fs::write(
            &fake_cargo,
            r#"#!/usr/bin/env bash
set -eu
output=""
cutover=0
stage=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --output) output="$2"; shift 2 ;;
    --cutover-evidence) cutover=1; shift 2 ;;
    --stage) stage="$2"; shift 2 ;;
    *) shift ;;
  esac
done
mkdir -p "$(dirname "$output")"
if [[ "$cutover" -eq 1 ]]; then
  printf '%s\n' '{"stale":true}' >"$output"
  exit 17
fi
case "$stage" in
  repo-snapshot) stage_name=RepoSnapshot ;;
  repo-edit) stage_name=RepoEdit ;;
  *) exit 19 ;;
esac
python3 - "$output" "$stage_name" <<'PY'
import json
import subprocess
import sys
import tomllib
from pathlib import Path
output, stage = sys.argv[1:]
source = "commit:{}/tree:{}".format(
    subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
    subprocess.check_output(["git", "rev-parse", "HEAD^{tree}"], text=True).strip(),
)
registry = tomllib.loads(Path("policy/extraction-parity.toml").read_text())
case_ids = sorted(case["id"] for case in registry["case"] if case["stage"] == stage)
records = [{"case_id": case_id, "result": "SemanticallyEquivalent", "source_identity": source,
            "old_output": "fixture-old", "new_output": "fixture-new"} for case_id in case_ids]
Path(output).write_text(json.dumps({
    "schema_id": "cargo-allow.extraction-parity-runtime.v1", "schema_version": 1,
    "tool": "cargo-allow extraction-parity", "result": "Passed", "completeness": "Complete",
    "source_identity": source, "stage": stage, "parity_result_digest": "sha256:v1:" + "a" * 64,
    "records": records, "expected_case_count": len(case_ids), "emitted_case_count": len(case_ids),
    "missing_case_ids": [], "unexpected_case_ids": [], "claim_boundary": ["fixture-runtime-parity"],
}) + "\n")
PY
"#,
        )
        .map_err(|error| error.to_string())?;
        let mut permissions = fs::metadata(&fake_cargo)
            .map_err(|error| error.to_string())?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_cargo, permissions).map_err(|error| error.to_string())?;

        let missing_output = root.join("target/missing");
        let stale_receipt = missing_output.join("repo-snapshot/cutover-receipt.json");
        fs::create_dir_all(
            stale_receipt
                .parent()
                .ok_or_else(|| "stale receipt has no parent".to_string())?,
        )
        .map_err(|error| error.to_string())?;
        fs::write(&stale_receipt, "stale\n").map_err(|error| error.to_string())?;
        let missing = run_status(&root, &missing_output, &bin)?;
        let missing_blockers = blockers(&missing)?;
        if missing.get("result").and_then(Value::as_str) != Some("Blocked")
            || !missing_blockers.contains(&"independent_build_package_receipt_missing:RepoSnapshot")
            || stale_receipt.exists()
        {
            return Err("missing build receipt did not fail closed and remove stale output".into());
        }

        let outside = fixture.join("outside-build-package.json");
        fs::write(&outside, "{}\n").map_err(|error| error.to_string())?;
        let outside_output = root.join("target/outside");
        let outside_status = Command::new("bash")
            .arg("scripts/extraction-cutover-status.sh")
            .current_dir(&root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("PATH", {
                let mut paths = vec![bin.clone()];
                paths.extend(std::env::split_paths(
                    &std::env::var_os("PATH").unwrap_or_default(),
                ));
                std::env::join_paths(paths).map_err(|error| error.to_string())?
            })
            .env("EXTRACTION_CUTOVER_DIR", &outside_output)
            .env("EXTRACTION_BUILD_PACKAGE_RECEIPT_REPO_SNAPSHOT", &outside)
            .env("EXTRACTION_BUILD_PACKAGE_RECEIPT_REPO_EDIT", &outside)
            .output()
            .map_err(|error| error.to_string())?;
        if !outside_status.status.success() {
            return Err(format!(
                "outside-receipt status run failed: {}",
                String::from_utf8_lossy(&outside_status.stderr)
            ));
        }
        let outside_value: Value = serde_json::from_str(
            &fs::read_to_string(outside_output.join("extraction-cutover-status.json"))
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        if !blockers(&outside_value)?.contains(&"build_package_receipt_outside_repo:RepoSnapshot") {
            return Err("outside build receipt did not fail closed".into());
        }

        let configured_output = root.join("target/configured");
        for stage in ["repo-snapshot", "repo-edit"] {
            let path = configured_output.join(stage).join("build-package.json");
            fs::create_dir_all(
                path.parent()
                    .ok_or_else(|| "configured receipt has no parent".to_string())?,
            )
            .map_err(|error| error.to_string())?;
            fs::write(path, "{}\n").map_err(|error| error.to_string())?;
        }
        let configured = run_status(&root, &configured_output, &bin)?;
        let configured_blockers = blockers(&configured)?;
        for stage in ["repo-snapshot", "repo-edit"] {
            let requested_stage = if stage == "repo-snapshot" {
                ParityStageArg::RepoSnapshot
            } else {
                ParityStageArg::RepoEdit
            };
            let (_stage, _registry, _ledger, _source, expected) =
                stage_inputs(requested_stage).map_err(|error| error.to_string())?;
            let ownership: OwnershipEvidenceReceipt = serde_json::from_str(
                &fs::read_to_string(configured_output.join(stage).join("ownership.json"))
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            let stage_status = configured
                .get("stages")
                .and_then(Value::as_array)
                .and_then(|stages| {
                    stages.iter().find(|item| {
                        item.get("stage").and_then(Value::as_str)
                            == Some(if stage == "repo-snapshot" {
                                "RepoSnapshot"
                            } else {
                                "RepoEdit"
                            })
                    })
                })
                .ok_or_else(|| format!("configured status omitted {stage}"))?;
            let expected_log = format!("target/configured/{stage}/cutover-receipt.log");
            let stage_dir = configured_output.join(stage);
            if !stage_dir.join("cutover-evidence.json").is_file() {
                return Err(format!(
                    "configured status omitted evidence manifest for {stage}"
                ));
            }
            if stage_dir.join("cutover-receipt.json").exists() {
                return Err(format!(
                    "rejected configured status retained receipt for {stage}"
                ));
            }
            let expected_blocker = format!("cutover_receipt_rejected:{stage}:17");
            if !configured_blockers.contains(&expected_blocker.as_str()) {
                return Err(format!(
                    "configured status omitted blocker `{expected_blocker}`; actual={configured_blockers:?}"
                ));
            }
            for (field, actual, expected) in [
                (
                    "package_paths",
                    &ownership.package_paths,
                    &expected.package_paths,
                ),
                ("asset_paths", &ownership.asset_paths, &expected.asset_paths),
                ("docs_paths", &ownership.docs_paths, &expected.docs_paths),
                ("ci_paths", &ownership.ci_paths, &expected.ci_paths),
            ] {
                if actual.iter().collect::<BTreeSet<_>>()
                    != expected.iter().collect::<BTreeSet<_>>()
                {
                    return Err(format!(
                        "configured ownership `{field}` mismatch for {stage}: actual={actual:?}, expected={expected:?}"
                    ));
                }
            }
            let actual_log = stage_status
                .get("cutover_receipt_log")
                .and_then(Value::as_str);
            if actual_log != Some(expected_log.as_str()) {
                return Err(format!(
                    "configured receipt log mismatch for {stage}: actual={actual_log:?}, expected={expected_log:?}"
                ));
            }
        }
        Ok(())
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
    fn cutover_evidence_rejects_incomplete_manifest_and_receipt_paths() -> Result<(), String> {
        let root = workspace_root();
        let registry = load_registry(&root).map_err(|error| error.to_string())?;
        let current = source_identity(&root).map_err(|error| error.to_string())?;

        assert_invalid(
            "failed parity",
            assemble_cutover_receipt_from_clean_inputs(
                &root,
                &registry,
                ParityStageArg::RepoSnapshot,
                "sha256:parity",
                &current,
                Path::new("missing-cutover-evidence.json"),
                false,
            ),
            "requires complete, equivalent runtime parity",
        )?;

        for (case, value, expected) in [
            ("malformed", None, "parse cutover evidence"),
            (
                "unsupported-schema",
                Some(json!({
                    "schema_id": "wrong.schema",
                    "schema_version": CUTOVER_EVIDENCE_SCHEMA_VERSION,
                    "ownership_receipt": "ownership.json",
                    "independent_build_package_receipt": "build.json"
                })),
                "unsupported extraction cutover evidence schema",
            ),
            (
                "absolute-ownership",
                Some(json!({
                    "schema_id": CUTOVER_EVIDENCE_SCHEMA_ID,
                    "schema_version": CUTOVER_EVIDENCE_SCHEMA_VERSION,
                    "ownership_receipt": root.join("README.md"),
                    "independent_build_package_receipt": "build.json"
                })),
                "must be repository-relative",
            ),
            (
                "escaping-ownership",
                Some(json!({
                    "schema_id": CUTOVER_EVIDENCE_SCHEMA_ID,
                    "schema_version": CUTOVER_EVIDENCE_SCHEMA_VERSION,
                    "ownership_receipt": "../outside.json",
                    "independent_build_package_receipt": "build.json"
                })),
                "missing or not a file",
            ),
            (
                "missing-ownership",
                Some(json!({
                    "schema_id": CUTOVER_EVIDENCE_SCHEMA_ID,
                    "schema_version": CUTOVER_EVIDENCE_SCHEMA_VERSION,
                    "ownership_receipt": "target/missing-ownership.json",
                    "independent_build_package_receipt": "target/missing-build.json"
                })),
                "missing or not a file",
            ),
        ] {
            let dir = fixture_root(case);
            fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
            let manifest = dir.join("manifest.json");
            if let Some(value) = value {
                write_json(&manifest, &value).map_err(|error| error.to_string())?;
            } else {
                fs::write(&manifest, "{not-json\n").map_err(|error| error.to_string())?;
            }
            let result = assemble_cutover_receipt_from_clean_inputs(
                &root,
                &registry,
                ParityStageArg::RepoSnapshot,
                "sha256:parity",
                &current,
                &manifest,
                true,
            );
            remove_fixture(&dir);
            assert_invalid(case, result, expected)?;
        }

        let (manifest, dir) = write_bundle(
            "missing-build-receipt",
            &current,
            "sha256:parity",
            "Passed",
            "Passed",
        )
        .map_err(|error| error.to_string())?;
        fs::remove_file(dir.join("build-package.json")).map_err(|error| error.to_string())?;
        let result = assemble_cutover_receipt_from_clean_inputs(
            &root,
            &registry,
            ParityStageArg::RepoSnapshot,
            "sha256:parity",
            &current,
            &manifest,
            true,
        );
        remove_fixture(&dir);
        assert_invalid("missing build receipt", result, "missing or not a file")?;

        for (case, member, expected) in [
            (
                "malformed ownership receipt",
                "ownership.json",
                "parse ownership receipt",
            ),
            (
                "malformed build receipt",
                "build-package.json",
                "parse independent build/package receipt",
            ),
        ] {
            let (manifest, dir) = write_bundle(case, &current, "sha256:parity", "Passed", "Passed")
                .map_err(|error| error.to_string())?;
            fs::write(dir.join(member), "{not-json\n").map_err(|error| error.to_string())?;
            let result = assemble_cutover_receipt_from_clean_inputs(
                &root,
                &registry,
                ParityStageArg::RepoSnapshot,
                "sha256:parity",
                &current,
                &manifest,
                true,
            );
            remove_fixture(&dir);
            assert_invalid(case, result, expected)?;
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
    fn ownership_receipt_rejects_each_stale_or_incomplete_binding() -> Result<(), String> {
        let root = workspace_root();
        let (stage, _registry, _ledger, source, ownership) =
            stage_inputs(ParityStageArg::RepoSnapshot).map_err(|error| error.to_string())?;
        for (case, expected) in [
            ("schema", "unsupported extraction ownership receipt schema"),
            ("version", "unsupported extraction ownership receipt schema"),
            ("stage", "does not match"),
            ("source", "source identity"),
            ("parity", "parity digest"),
            ("result", "must be a positive complete outcome"),
            ("claim", "claim_boundary is empty"),
            ("package_paths", "package_paths"),
            ("asset_paths", "asset_paths"),
            ("docs_paths", "docs_paths"),
            ("ci_paths", "ci_paths"),
        ] {
            let mut receipt =
                ownership_receipt(stage, &source, "sha256:parity", &ownership, "Passed");
            match case {
                "schema" => receipt.schema_id = "wrong.schema".to_string(),
                "version" => receipt.schema_version = 2,
                "stage" => receipt.stage = "RepoEdit".to_string(),
                "source" => receipt.source_identity = "commit:stale/tree:stale".to_string(),
                "parity" => receipt.parity_result_digest = "sha256:stale".to_string(),
                "result" => receipt.result = "Blocked".to_string(),
                "claim" => receipt.claim_boundary = "  ".to_string(),
                "package_paths" => receipt.package_paths.push("README.md".to_string()),
                "asset_paths" => receipt.asset_paths.push("README.md".to_string()),
                "docs_paths" => receipt.docs_paths.push("README.md".to_string()),
                "ci_paths" => receipt.ci_paths.push("README.md".to_string()),
                _ => return Err(format!("unknown ownership mutation `{case}`")),
            }
            assert_invalid(
                case,
                validate_ownership_receipt(
                    &root,
                    &receipt,
                    stage,
                    &source,
                    "sha256:parity",
                    &ownership,
                ),
                expected,
            )?;
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
    fn build_package_receipt_rejects_each_stale_or_incomplete_binding() -> Result<(), String> {
        let root = workspace_root();
        let (stage, _registry, _ledger, source, ownership) =
            stage_inputs(ParityStageArg::RepoSnapshot).map_err(|error| error.to_string())?;
        let fixture = fixture_root("build-negative-table");
        fs::create_dir_all(&fixture).map_err(|error| error.to_string())?;
        for (case, expected) in [
            (
                "schema",
                "unsupported extraction build/package receipt schema",
            ),
            (
                "version",
                "unsupported extraction build/package receipt schema",
            ),
            ("stage", "stale or for another stage"),
            ("source", "stale or for another stage"),
            ("architecture", "stale or for another stage"),
            ("parity", "stale or for another stage"),
            ("not-independent", "source-checkout isolation"),
            ("checkout-not-denied", "source-checkout isolation"),
            ("result", "must be a positive complete outcome"),
            ("claim", "claim_boundary is empty"),
            ("duplicate-package", "duplicate package receipt"),
            ("negative-package", "package receipt result"),
            ("missing-package", "does not match topology"),
            ("unexpected-package", "does not match topology"),
            ("noncanonical-package", "not canonical"),
            ("missing-package-file", "missing or not a file"),
            ("stale-package-digest", "package receipt digest mismatch"),
            ("no-builds", "has no build records"),
            ("duplicate-build", "duplicate build receipt"),
            ("negative-build", "build receipt result"),
            ("noncanonical-build", "not canonical"),
            ("missing-build-file", "missing or not a file"),
            ("stale-build-digest", "build receipt digest mismatch"),
        ] {
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
            match case {
                "schema" => receipt.schema_id = "wrong.schema".to_string(),
                "version" => receipt.schema_version = 2,
                "stage" => receipt.stage = "RepoEdit".to_string(),
                "source" => receipt.source_identity = "commit:stale/tree:stale".to_string(),
                "architecture" => receipt.architecture_manifest_digest = "sha256:stale".to_string(),
                "parity" => receipt.parity_result_digest = "sha256:stale".to_string(),
                "not-independent" => receipt.independent = false,
                "checkout-not-denied" => receipt.source_checkout_denied = false,
                "result" => receipt.result = "Blocked".to_string(),
                "claim" => receipt.claim_boundary = "  ".to_string(),
                "duplicate-package" => {
                    let duplicate = receipt
                        .package_records
                        .first()
                        .ok_or_else(|| "missing package fixture".to_string())?;
                    receipt.package_records.push(PackageEvidenceRecord {
                        package_name: duplicate.package_name.clone(),
                        path: duplicate.path.clone(),
                        sha256: duplicate.sha256.clone(),
                        result: duplicate.result.clone(),
                    });
                }
                "negative-package" => {
                    receipt
                        .package_records
                        .first_mut()
                        .ok_or_else(|| "missing package fixture".to_string())?
                        .result = "Blocked".to_string();
                }
                "missing-package" => {
                    receipt.package_records.pop();
                }
                "unexpected-package" => {
                    receipt
                        .package_records
                        .first_mut()
                        .ok_or_else(|| "missing package fixture".to_string())?
                        .package_name = "unexpected-package".to_string();
                }
                "noncanonical-package" => {
                    let record = receipt
                        .package_records
                        .first_mut()
                        .ok_or_else(|| "missing package fixture".to_string())?;
                    record.path = format!("./{}", record.path);
                }
                "missing-package-file" => {
                    receipt
                        .package_records
                        .first_mut()
                        .ok_or_else(|| "missing package fixture".to_string())?
                        .path = "target/missing-package.crate".to_string();
                }
                "stale-package-digest" => {
                    receipt
                        .package_records
                        .first_mut()
                        .ok_or_else(|| "missing package fixture".to_string())?
                        .sha256 = "sha256:v1:stale".to_string();
                }
                "no-builds" => receipt.build_records.clear(),
                "duplicate-build" => {
                    let duplicate = receipt
                        .build_records
                        .first()
                        .ok_or_else(|| "missing build fixture".to_string())?;
                    receipt.build_records.push(BuildEvidenceRecord {
                        artifact_name: duplicate.artifact_name.clone(),
                        path: duplicate.path.clone(),
                        sha256: duplicate.sha256.clone(),
                        result: duplicate.result.clone(),
                    });
                }
                "negative-build" => {
                    receipt
                        .build_records
                        .first_mut()
                        .ok_or_else(|| "missing build fixture".to_string())?
                        .result = "Blocked".to_string();
                }
                "noncanonical-build" => {
                    let record = receipt
                        .build_records
                        .first_mut()
                        .ok_or_else(|| "missing build fixture".to_string())?;
                    record.path = format!("./{}", record.path);
                }
                "missing-build-file" => {
                    receipt
                        .build_records
                        .first_mut()
                        .ok_or_else(|| "missing build fixture".to_string())?
                        .path = "target/missing-build.bin".to_string();
                }
                "stale-build-digest" => {
                    receipt
                        .build_records
                        .first_mut()
                        .ok_or_else(|| "missing build fixture".to_string())?
                        .sha256 = "sha256:v1:stale".to_string();
                }
                _ => return Err(format!("unknown build mutation `{case}`")),
            }
            assert_invalid(
                case,
                validate_build_package_receipt(
                    &root,
                    &receipt,
                    stage,
                    &source,
                    "sha256:parity",
                    &ownership,
                ),
                expected,
            )?;
        }
        remove_fixture(&fixture);
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
