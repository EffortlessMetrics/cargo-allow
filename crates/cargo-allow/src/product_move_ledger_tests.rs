use allow_policy::product_move::{MoveLedgerDiagnosticKind, validate_product_move_ledger_at};
use std::path::PathBuf;

#[test]
fn product_move_ledger_repository_inventory_is_valid() -> Result<(), String> {
    let root = repo_root();
    let ledger_path = root.join("policy/product-move-ledger.toml");
    let (validated, diagnostics, report) = validate_product_move_ledger_at(&root, &ledger_path)
        .map_err(|err| format!("validate move ledger: {err}"))?;

    assert!(validated.valid, "diagnostics: {diagnostics:?}");
    assert!(
        diagnostics
            .iter()
            .all(|diag| diag.kind != MoveLedgerDiagnosticKind::MissingCurrentPath),
        "missing current paths: {diagnostics:?}"
    );
    assert!(
        report.entry_count >= 8,
        "seeded inventory should cover primary seams"
    );
    assert_eq!(
        validated.ledger.controlling_issue, 2598,
        "ledger controlling issue"
    );
    assert_eq!(
        validated.ledger.ledger_id, "CARGO-ALLOW-MOVE-LEDGER-0001",
        "ledger id"
    );

    let map = root.join("docs/architecture/product-move-map.md");
    let map_text =
        std::fs::read_to_string(&map).map_err(|err| format!("product move map readable: {err}"))?;
    if !map_text.contains("move-allow-policy-spec-system") {
        return Err("human projection missing primary inventory row".to_string());
    }

    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
