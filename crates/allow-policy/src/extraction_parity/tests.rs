use super::config::parse_extraction_parity_registry;
use super::validate::{ParityDiagnosticKind, validate_extraction_parity_registry_at};
use std::path::PathBuf;

#[test]
fn parse_extraction_parity_registry_reads_cases() -> Result<(), String> {
    let registry = parse_extraction_parity_registry(
        r#"
schema_version = "1.0"
registry_id = "CARGO-ALLOW-PARITY-0001"
controlling_issue = 2606
linked_shim_registry = "CARGO-ALLOW-SHIM-REGISTRY-0001"

[[case]]
id = "parity-repo-snapshot-staged-index-v1"
stage = "RepoSnapshot"
move_ledger_entry = "move-allow-diff-staged-index"
shim_id = "shim-allow-diff-staged-index"
old_producer = "allow-diff::staged_index"
new_producer = "repo-snapshot::staged_index"
expected_result = "SemanticallyEquivalent"
disposition = "contract_only"
claim_boundary = "test"
"#,
    )
    .map_err(|err| format!("parse parity registry: {err}"))?;
    assert_eq!(registry.case.len(), 1);
    Ok(())
}

#[test]
fn repository_parity_registry_links_shims_and_ledger() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (_, diagnostics, report) = validate_extraction_parity_registry_at(
        &root,
        &root.join("policy/extraction-parity.toml"),
        &root.join("policy/product-move-ledger.toml"),
        &root.join("policy/extraction-shims.toml"),
    )
    .map_err(|err| format!("validate parity registry: {err}"))?;
    if diagnostics
        .iter()
        .any(|diag| diag.kind == ParityDiagnosticKind::UnreferencedShimParityCase)
    {
        return Err(format!("unreferenced shim cases: {diagnostics:?}"));
    }
    if diagnostics
        .iter()
        .any(|diag| diag.kind == ParityDiagnosticKind::MissingMoveLedgerEntry)
    {
        return Err(format!("missing move ledger links: {diagnostics:?}"));
    }
    if report.case_count < 7 {
        return Err("seeded parity case inventory too small".to_string());
    }
    // #3309 installment 3: the IntentEngine stage is promoted alongside
    // RepoSnapshot/RepoEdit, so the live registry's proven IntentEngine
    // cases must not emit NonContractDisposition.
    if diagnostics
        .iter()
        .any(|diag| diag.kind == ParityDiagnosticKind::NonContractDisposition)
    {
        return Err(format!("non-contract dispositions: {diagnostics:?}"));
    }
    Ok(())
}
