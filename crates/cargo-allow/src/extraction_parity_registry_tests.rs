use allow_policy::extraction_parity::{
    ParityDiagnosticKind, validate_extraction_parity_registry_at,
};
use std::path::PathBuf;

#[test]
fn extraction_parity_registry_report_only() -> Result<(), String> {
    let root = repo_root();
    let (registry, diagnostics, report) = validate_extraction_parity_registry_at(
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
        return Err(format!("shim/parity drift: {diagnostics:?}"));
    }
    assert_eq!(registry.registry_id, "CARGO-ALLOW-PARITY-0001");
    assert_eq!(registry.controlling_issue, 2606);
    assert!(report.case_count >= 7);
    assert!(report.stage_receipt_count >= 2);

    let doc = root.join("docs/architecture/extraction-parity.md");
    let doc_text =
        std::fs::read_to_string(&doc).map_err(|err| format!("parity doc readable: {err}"))?;
    if !doc_text.contains("CARGO-ALLOW-PARITY-0001") {
        return Err("human projection missing registry id".to_string());
    }

    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
