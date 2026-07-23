use allow_policy::extraction_shims::{ShimDiagnosticKind, validate_extraction_shim_registry_at};
use std::path::PathBuf;

#[test]
fn extraction_shim_registry_report_only() -> Result<(), String> {
    let root = repo_root();
    let (_, diagnostics, report) = validate_extraction_shim_registry_at(
        &root,
        &root.join("policy/extraction-shims.toml"),
        &root.join("policy/product-move-ledger.toml"),
    )
    .map_err(|err| format!("validate shim registry: {err}"))?;

    if diagnostics
        .iter()
        .any(|diag| diag.kind == ShimDiagnosticKind::MissingMoveLedgerEntry)
    {
        return Err(format!("missing move ledger links: {diagnostics:?}"));
    }
    if report.shim_count < 7 {
        return Err("seeded shim inventory too small".to_string());
    }
    if report.active_count < 3 {
        return Err("expected repo-snapshot and rust-source-index shims active".to_string());
    }
    if report.planned_count < 4 {
        return Err("expected remaining shims planned".to_string());
    }

    let doc = root.join("docs/architecture/extraction-shims.md");
    let doc_text =
        std::fs::read_to_string(&doc).map_err(|err| format!("shim doc readable: {err}"))?;
    if !doc_text.contains("CARGO-ALLOW-SHIM-REGISTRY-0001") {
        return Err("human projection missing registry id".to_string());
    }

    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
