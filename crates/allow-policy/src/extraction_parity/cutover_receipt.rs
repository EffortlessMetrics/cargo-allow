use super::config::ExtractionStage;
use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

pub const EXTRACTION_CUTOVER_RECEIPT_SCHEMA_ID: &str = "cargo-allow.extraction-cutover-receipt.v1";
pub const EXTRACTION_CUTOVER_RECEIPT_SCHEMA_VERSION: u32 = 1;

/// A receipt proving one extraction stage against one exact source state.
///
/// This is the durable receipt contract. Generation and CI artifact upload are
/// separate adapters; this type refuses to validate a partial or stale receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExtractionCutoverReceipt {
    pub schema_id: String,
    pub schema_version: u32,
    pub stage: ExtractionStage,
    pub policy_generation: String,
    pub source_identity: String,
    pub move_ledger_generation: String,
    pub selected_move_ledger_entries: Vec<String>,
    pub architecture_manifest_digest: String,
    pub old_tool_identity: String,
    pub new_tool_identity: String,
    pub parity_corpus_generation: String,
    pub parity_result_digest: String,
    pub accepted_intentional_differences: Vec<String>,
    pub transitional_shims: Vec<String>,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtractionCutoverReceiptToml {
    schema_id: String,
    schema_version: u32,
    stage: String,
    policy_generation: String,
    source_identity: String,
    move_ledger_generation: String,
    selected_move_ledger_entries: Vec<String>,
    architecture_manifest_digest: String,
    old_tool_identity: String,
    new_tool_identity: String,
    parity_corpus_generation: String,
    parity_result_digest: String,
    accepted_intentional_differences: Vec<String>,
    transitional_shims: Vec<String>,
    latest_allowed_shim_stage: String,
    old_path_reachability_result: String,
    duplicate_semantic_implementation_result: String,
    package_assets_docs_ci_ownership_result: String,
    independent_build_package_result: String,
    rollback_route: String,
    result_class: String,
    completeness: String,
    limitations: Vec<String>,
    claim_boundary: String,
}

impl ExtractionCutoverReceiptToml {
    fn into_receipt(self) -> CargoAllowResult<ExtractionCutoverReceipt> {
        if self.schema_id != EXTRACTION_CUTOVER_RECEIPT_SCHEMA_ID {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!(
                    "unsupported extraction cutover receipt schema_id `{}`",
                    self.schema_id
                ),
            ));
        }
        if self.schema_version != EXTRACTION_CUTOVER_RECEIPT_SCHEMA_VERSION {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!(
                    "unsupported extraction cutover receipt schema_version `{}`",
                    self.schema_version
                ),
            ));
        }

        Ok(ExtractionCutoverReceipt {
            schema_id: self.schema_id,
            schema_version: self.schema_version,
            stage: ExtractionStage::parse(&self.stage)?,
            policy_generation: self.policy_generation,
            source_identity: self.source_identity,
            move_ledger_generation: self.move_ledger_generation,
            selected_move_ledger_entries: self.selected_move_ledger_entries,
            architecture_manifest_digest: self.architecture_manifest_digest,
            old_tool_identity: self.old_tool_identity,
            new_tool_identity: self.new_tool_identity,
            parity_corpus_generation: self.parity_corpus_generation,
            parity_result_digest: self.parity_result_digest,
            accepted_intentional_differences: self.accepted_intentional_differences,
            transitional_shims: self.transitional_shims,
            latest_allowed_shim_stage: self.latest_allowed_shim_stage,
            old_path_reachability_result: self.old_path_reachability_result,
            duplicate_semantic_implementation_result: self.duplicate_semantic_implementation_result,
            package_assets_docs_ci_ownership_result: self.package_assets_docs_ci_ownership_result,
            independent_build_package_result: self.independent_build_package_result,
            rollback_route: self.rollback_route,
            result_class: self.result_class,
            completeness: self.completeness,
            limitations: self.limitations,
            claim_boundary: self.claim_boundary,
        })
    }
}

pub fn parse_extraction_cutover_receipt(input: &str) -> CargoAllowResult<ExtractionCutoverReceipt> {
    parse_extraction_cutover_receipt_at(None, input)
}

pub fn parse_extraction_cutover_receipt_at(
    path: Option<&Path>,
    input: &str,
) -> CargoAllowResult<ExtractionCutoverReceipt> {
    let parsed = toml::from_str::<ExtractionCutoverReceiptToml>(input).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("failed to parse extraction cutover receipt TOML: {error}"),
        )
        .with_toml_span(path, input, error.span())
    })?;
    parsed.into_receipt()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutoverReceiptDiagnosticKind {
    MissingField,
    StaleSourceIdentity,
    MissingMoveLedgerEntry,
}

impl CutoverReceiptDiagnosticKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MissingField => "missing_field",
            Self::StaleSourceIdentity => "stale_source_identity",
            Self::MissingMoveLedgerEntry => "missing_move_ledger_entry",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutoverReceiptDiagnostic {
    pub kind: CutoverReceiptDiagnosticKind,
    pub field: String,
    pub message: String,
}

/// Validate a parsed receipt against the source identity and ledger entries it
/// claims to cover. A receipt is only current for the exact identity supplied
/// by the caller.
pub fn validate_extraction_cutover_receipt(
    receipt: &ExtractionCutoverReceipt,
    expected_source_identity: &str,
    required_move_ledger_entries: &[String],
) -> Vec<CutoverReceiptDiagnostic> {
    let mut diagnostics = Vec::new();
    let required_fields = [
        ("policy_generation", receipt.policy_generation.as_str()),
        ("source_identity", receipt.source_identity.as_str()),
        (
            "move_ledger_generation",
            receipt.move_ledger_generation.as_str(),
        ),
        (
            "architecture_manifest_digest",
            receipt.architecture_manifest_digest.as_str(),
        ),
        ("old_tool_identity", receipt.old_tool_identity.as_str()),
        ("new_tool_identity", receipt.new_tool_identity.as_str()),
        (
            "parity_corpus_generation",
            receipt.parity_corpus_generation.as_str(),
        ),
        (
            "parity_result_digest",
            receipt.parity_result_digest.as_str(),
        ),
        (
            "latest_allowed_shim_stage",
            receipt.latest_allowed_shim_stage.as_str(),
        ),
        (
            "old_path_reachability_result",
            receipt.old_path_reachability_result.as_str(),
        ),
        (
            "duplicate_semantic_implementation_result",
            receipt.duplicate_semantic_implementation_result.as_str(),
        ),
        (
            "package_assets_docs_ci_ownership_result",
            receipt.package_assets_docs_ci_ownership_result.as_str(),
        ),
        (
            "independent_build_package_result",
            receipt.independent_build_package_result.as_str(),
        ),
        ("rollback_route", receipt.rollback_route.as_str()),
        ("result_class", receipt.result_class.as_str()),
        ("completeness", receipt.completeness.as_str()),
        ("claim_boundary", receipt.claim_boundary.as_str()),
    ];
    for (field, value) in required_fields {
        if value.trim().is_empty() {
            diagnostics.push(CutoverReceiptDiagnostic {
                kind: CutoverReceiptDiagnosticKind::MissingField,
                field: field.to_string(),
                message: format!("cutover receipt field `{field}` is empty"),
            });
        }
    }

    if receipt.source_identity != expected_source_identity {
        diagnostics.push(CutoverReceiptDiagnostic {
            kind: CutoverReceiptDiagnosticKind::StaleSourceIdentity,
            field: "source_identity".to_string(),
            message: format!(
                "receipt source identity `{}` does not match current `{expected_source_identity}`",
                receipt.source_identity
            ),
        });
    }

    let selected: BTreeSet<&str> = receipt
        .selected_move_ledger_entries
        .iter()
        .map(String::as_str)
        .collect();
    for entry in required_move_ledger_entries {
        if !selected.contains(entry.as_str()) {
            diagnostics.push(CutoverReceiptDiagnostic {
                kind: CutoverReceiptDiagnosticKind::MissingMoveLedgerEntry,
                field: "selected_move_ledger_entries".to_string(),
                message: format!("receipt does not cover move-ledger entry `{entry}`"),
            });
        }
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt() -> ExtractionCutoverReceipt {
        ExtractionCutoverReceipt {
            schema_id: EXTRACTION_CUTOVER_RECEIPT_SCHEMA_ID.to_string(),
            schema_version: EXTRACTION_CUTOVER_RECEIPT_SCHEMA_VERSION,
            stage: ExtractionStage::RepoSnapshot,
            policy_generation: "policy:42".to_string(),
            source_identity: "commit:abc/tree:def".to_string(),
            move_ledger_generation: "ledger:7".to_string(),
            selected_move_ledger_entries: vec![
                "move-allow-diff-staged-index".to_string(),
                "move-allow-diff-revision-identity".to_string(),
            ],
            architecture_manifest_digest: "sha256:architecture".to_string(),
            old_tool_identity: "cargo-allow:old".to_string(),
            new_tool_identity: "repo-snapshot:new".to_string(),
            parity_corpus_generation: "parity:1".to_string(),
            parity_result_digest: "sha256:parity".to_string(),
            accepted_intentional_differences: Vec::new(),
            transitional_shims: vec!["shim-allow-diff-staged-index".to_string()],
            latest_allowed_shim_stage: "RepoSnapshot".to_string(),
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

    #[test]
    fn parses_and_validates_current_receipt() -> Result<(), String> {
        let parsed = parse_extraction_cutover_receipt(
            &toml::to_string(&receipt()).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let required = vec![
            "move-allow-diff-staged-index".to_string(),
            "move-allow-diff-revision-identity".to_string(),
        ];
        let diagnostics =
            validate_extraction_cutover_receipt(&parsed, "commit:abc/tree:def", &required);
        if !diagnostics.is_empty() {
            return Err(format!("current receipt rejected: {diagnostics:?}"));
        }
        Ok(())
    }

    #[test]
    fn stale_identity_and_missing_entry_are_rejected() -> Result<(), String> {
        let receipt = receipt();
        let diagnostics = validate_extraction_cutover_receipt(
            &receipt,
            "commit:new/tree:new",
            &["move-missing".to_string()],
        );
        if !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == CutoverReceiptDiagnosticKind::StaleSourceIdentity)
        {
            return Err("stale source identity was accepted".to_string());
        }
        if !diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == CutoverReceiptDiagnosticKind::MissingMoveLedgerEntry
        }) {
            return Err("missing ledger entry was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn empty_required_field_is_rejected() -> Result<(), String> {
        let mut receipt = receipt();
        receipt.claim_boundary.clear();
        let diagnostics = validate_extraction_cutover_receipt(&receipt, "commit:abc/tree:def", &[]);
        if !diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == CutoverReceiptDiagnosticKind::MissingField
                && diagnostic.field == "claim_boundary"
        }) {
            return Err("empty claim boundary was accepted".to_string());
        }
        Ok(())
    }
}
