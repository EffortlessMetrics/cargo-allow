use super::config::{ExtractionParityRegistry, ExtractionStage, ParityDisposition};
use super::cutover_receipt::{
    EXTRACTION_CUTOVER_RECEIPT_SCHEMA_ID, EXTRACTION_CUTOVER_RECEIPT_SCHEMA_VERSION,
    ExtractionCutoverReceipt, validate_extraction_cutover_receipt,
};
use crate::product_move::ProductMoveLedger;
use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use std::collections::BTreeSet;

/// Runtime evidence supplied by a stage-specific parity and reachability
/// adapter. The producer owns the durable field assembly; adapters own how
/// each evidence value was obtained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionCutoverReceiptEvidence {
    pub source_identity: String,
    pub architecture_manifest_digest: String,
    pub old_tool_identity: String,
    pub new_tool_identity: String,
    pub parity_corpus_generation: String,
    pub parity_result_digest: String,
    pub accepted_intentional_differences: Vec<String>,
    pub latest_allowed_shim_stage: String,
    pub old_path_reachability_result: String,
    pub duplicate_semantic_implementation_result: String,
    pub package_assets_docs_ci_ownership_result: String,
    pub independent_build_package_result: String,
    pub rollback_route: String,
    pub result_class: String,
    pub completeness: String,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

/// Produce a cutover receipt only from proven parity cases for one stage.
///
/// The selected move-ledger entries and transitional shims are derived from
/// the source-of-truth registries instead of being caller-provided lists.
/// This prevents a receipt producer from silently omitting a covered case or
/// binding an entry from another extraction stage.
pub fn produce_extraction_cutover_receipt(
    registry: &ExtractionParityRegistry,
    ledger: &ProductMoveLedger,
    stage: ExtractionStage,
    evidence: ExtractionCutoverReceiptEvidence,
) -> CargoAllowResult<ExtractionCutoverReceipt> {
    let stage_cases: Vec<_> = registry
        .case
        .iter()
        .filter(|case| case.stage == stage)
        .collect();
    if stage_cases.is_empty() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "no parity cases are registered for extraction stage `{}`",
                stage.as_str()
            ),
        ));
    }

    let mut selected_entries = BTreeSet::new();
    let mut transitional_shims = BTreeSet::new();

    for case in stage_cases {
        if case.disposition != ParityDisposition::Proven {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!(
                    "parity case `{}` is `{}`; cutover receipts require proven cases",
                    case.id,
                    case.disposition.as_str()
                ),
            ));
        }
        let entry = ledger
            .entry
            .iter()
            .find(|entry| entry.id == case.move_ledger_entry)
            .ok_or_else(|| {
                CargoAllowError::with_kind(
                    CargoAllowErrorKind::InvalidConfig,
                    format!(
                        "move-ledger entry `{}` disappeared during receipt assembly",
                        case.move_ledger_entry
                    ),
                )
            })?;
        if entry.cutover_stage != stage.as_str() {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!(
                    "move-ledger entry `{}` belongs to `{}`, not `{}`",
                    entry.id,
                    entry.cutover_stage,
                    stage.as_str()
                ),
            ));
        }
        selected_entries.insert(case.move_ledger_entry.clone());
        if let Some(shim_id) = &case.shim_id {
            transitional_shims.insert(shim_id.clone());
        }
    }

    let receipt = ExtractionCutoverReceipt {
        schema_id: EXTRACTION_CUTOVER_RECEIPT_SCHEMA_ID.to_string(),
        schema_version: EXTRACTION_CUTOVER_RECEIPT_SCHEMA_VERSION,
        stage,
        policy_generation: format!("{}@{}", registry.registry_id, registry.schema_version),
        source_identity: evidence.source_identity,
        move_ledger_generation: format!("{}@{}", ledger.ledger_id, ledger.schema_version),
        selected_move_ledger_entries: selected_entries.into_iter().collect(),
        architecture_manifest_digest: evidence.architecture_manifest_digest,
        old_tool_identity: evidence.old_tool_identity,
        new_tool_identity: evidence.new_tool_identity,
        parity_corpus_generation: evidence.parity_corpus_generation,
        parity_result_digest: evidence.parity_result_digest,
        accepted_intentional_differences: evidence.accepted_intentional_differences,
        transitional_shims: transitional_shims.into_iter().collect(),
        latest_allowed_shim_stage: evidence.latest_allowed_shim_stage,
        old_path_reachability_result: evidence.old_path_reachability_result,
        duplicate_semantic_implementation_result: evidence.duplicate_semantic_implementation_result,
        package_assets_docs_ci_ownership_result: evidence.package_assets_docs_ci_ownership_result,
        independent_build_package_result: evidence.independent_build_package_result,
        rollback_route: evidence.rollback_route,
        result_class: evidence.result_class,
        completeness: evidence.completeness,
        limitations: evidence.limitations,
        claim_boundary: evidence.claim_boundary,
    };

    let required_entries = receipt.selected_move_ledger_entries.clone();
    let diagnostics =
        validate_extraction_cutover_receipt(&receipt, &receipt.source_identity, &required_entries);
    if let Some(diagnostic) = diagnostics.first() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "generated cutover receipt is invalid: {}",
                diagnostic.message
            ),
        ));
    }
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::super::config::{
        ExtractionParityCase, ExtractionStage, ParityComparisonResult, ParityDisposition,
    };
    use super::*;
    use crate::product_move::{MoveDiscovery, MoveEntry};

    fn evidence() -> ExtractionCutoverReceiptEvidence {
        ExtractionCutoverReceiptEvidence {
            source_identity: "commit:abc/tree:def".to_string(),
            architecture_manifest_digest: "sha256:architecture".to_string(),
            old_tool_identity: "allow-diff:old".to_string(),
            new_tool_identity: "repo-snapshot:new".to_string(),
            parity_corpus_generation: "parity:1".to_string(),
            parity_result_digest: "sha256:parity".to_string(),
            accepted_intentional_differences: Vec::new(),
            latest_allowed_shim_stage: "IntentEngine".to_string(),
            old_path_reachability_result: "Deleted".to_string(),
            duplicate_semantic_implementation_result: "none".to_string(),
            package_assets_docs_ci_ownership_result: "bound".to_string(),
            independent_build_package_result: "passed".to_string(),
            rollback_route: "revert:abc".to_string(),
            result_class: "ParityAccepted".to_string(),
            completeness: "complete".to_string(),
            limitations: vec!["No universal correctness claim".to_string()],
            claim_boundary: "Exact source-state parity evidence".to_string(),
        }
    }

    fn registry(disposition: ParityDisposition) -> ExtractionParityRegistry {
        ExtractionParityRegistry {
            schema_version: "1.0".to_string(),
            registry_id: "CARGO-ALLOW-PARITY-0001".to_string(),
            controlling_issue: 2606,
            linked_shim_registry: "CARGO-ALLOW-SHIM-REGISTRY-0001".to_string(),
            case: vec![ExtractionParityCase {
                id: "parity-repo-snapshot-v1".to_string(),
                stage: ExtractionStage::RepoSnapshot,
                move_ledger_entry: "move-a".to_string(),
                shim_id: Some("shim-a".to_string()),
                old_producer: "old".to_string(),
                new_producer: "new".to_string(),
                expected_result: ParityComparisonResult::SemanticallyEquivalent,
                disposition,
                claim_boundary: "fixture".to_string(),
            }],
            stage_receipt: Vec::new(),
        }
    }

    fn ledger(cutover_stage: &str) -> ProductMoveLedger {
        ProductMoveLedger {
            schema_id: "cargo-allow.three-product-move-ledger.v1".to_string(),
            schema_version: 1,
            ledger_id: "CARGO-ALLOW-MOVE-LEDGER-0001".to_string(),
            controlling_issue: 2606,
            owner_issue: 2606,
            topology_issue: 2606,
            architecture_issue: 2606,
            package_issue: 2606,
            parity_issue: 2606,
            shim_issue: 2606,
            linked_plan: "plan".to_string(),
            linked_adr: "adr".to_string(),
            projection: "projection".to_string(),
            plan: "plan".to_string(),
            claim_boundary: "boundary".to_string(),
            discovery: MoveDiscovery {
                recursive_roots: Vec::new(),
                token_scan_roots: Vec::new(),
                selected_files: Vec::new(),
                filename_tokens: Vec::new(),
                no_new_enforcement: false,
            },
            entry: vec![MoveEntry {
                id: "move-a".to_string(),
                source_kind: "RustModule".to_string(),
                current_paths: vec!["old.rs".to_string()],
                current_refs: Vec::new(),
                current_identity: "old".to_string(),
                current_product: "cargo-allow".to_string(),
                current_crate: "allow-diff".to_string(),
                current_consumers: Vec::new(),
                posture: "PrivateImplementation".to_string(),
                target_product: "shared".to_string(),
                target_crate: "repo-snapshot".to_string(),
                target_module: "git".to_string(),
                disposition: "MoveToSharedSnapshot".to_string(),
                compatibility_strategy: "ReExportThenDelete".to_string(),
                schema_producer_impact: "impact".to_string(),
                parity_case_ids: vec!["parity-repo-snapshot-v1".to_string()],
                cutover_stage: cutover_stage.to_string(),
                expected_cutover_receipt: "CUTOVER-REPO-SNAPSHOT".to_string(),
                old_path_reachability_disposition: "Deleted".to_string(),
                active_shim_ids: vec!["shim-a".to_string()],
                latest_allowed_shim_stage: "IntentEngine".to_string(),
                duplicate_authority_class: "BoundedParityOnly".to_string(),
                selected_public_producer_after_cutover: "repo-snapshot".to_string(),
                package_ci_docs_impact: vec!["package".to_string()],
                removal_issue_or_condition: "#2583".to_string(),
                migration_owner_issue: "#2583".to_string(),
                risk: "Critical".to_string(),
                rollback: "revert".to_string(),
                status: "TargetRatified".to_string(),
                claim_boundary: "boundary".to_string(),
                next_move: "move".to_string(),
                deletion_output: "deleted".to_string(),
            }],
        }
    }

    #[test]
    fn producer_derives_stage_entries_and_shims() -> Result<(), String> {
        let receipt = produce_extraction_cutover_receipt(
            &registry(ParityDisposition::Proven),
            &ledger("RepoSnapshot"),
            ExtractionStage::RepoSnapshot,
            evidence(),
        )
        .map_err(|error| error.to_string())?;
        if receipt.selected_move_ledger_entries != vec!["move-a".to_string()]
            || receipt.transitional_shims != vec!["shim-a".to_string()]
            || receipt.parity_corpus_generation != "parity:1"
        {
            return Err(format!("producer derived the wrong receipt: {receipt:?}"));
        }
        Ok(())
    }

    #[test]
    fn producer_rejects_unproven_or_cross_stage_inputs() -> Result<(), String> {
        if produce_extraction_cutover_receipt(
            &registry(ParityDisposition::ContractOnly),
            &ledger("RepoSnapshot"),
            ExtractionStage::RepoSnapshot,
            evidence(),
        )
        .is_ok()
        {
            return Err("contract-only parity was promoted to a cutover receipt".to_string());
        }
        if produce_extraction_cutover_receipt(
            &registry(ParityDisposition::Proven),
            &ledger("RepoEdit"),
            ExtractionStage::RepoSnapshot,
            evidence(),
        )
        .is_ok()
        {
            return Err("cross-stage move entry was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn producer_rejects_missing_stage_and_ledger_inputs() -> Result<(), String> {
        let mut empty_registry = registry(ParityDisposition::Proven);
        empty_registry.case.clear();
        if produce_extraction_cutover_receipt(
            &empty_registry,
            &ledger("RepoSnapshot"),
            ExtractionStage::RepoSnapshot,
            evidence(),
        )
        .is_ok()
        {
            return Err("empty stage was accepted".to_string());
        }

        let mut missing_entry_ledger = ledger("RepoSnapshot");
        if let Some(entry) = missing_entry_ledger.entry.first_mut() {
            entry.id = "other-entry".to_string();
        }
        if produce_extraction_cutover_receipt(
            &registry(ParityDisposition::Proven),
            &missing_entry_ledger,
            ExtractionStage::RepoSnapshot,
            evidence(),
        )
        .is_ok()
        {
            return Err("missing ledger entry was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn producer_rejects_incomplete_runtime_evidence() -> Result<(), String> {
        let mut incomplete = evidence();
        incomplete.claim_boundary.clear();
        if produce_extraction_cutover_receipt(
            &registry(ParityDisposition::Proven),
            &ledger("RepoSnapshot"),
            ExtractionStage::RepoSnapshot,
            incomplete,
        )
        .is_ok()
        {
            return Err("incomplete runtime evidence was accepted".to_string());
        }
        Ok(())
    }
}
