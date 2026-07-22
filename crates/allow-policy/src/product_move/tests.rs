use super::config::{
    MoveDisposition, MoveEntryStatus, MoveIdentityKind, parse_product_move_ledger,
    parse_product_move_ledger_at,
};
use super::validate::{
    MoveLedgerDiagnosticKind, validate_product_move_ledger, validate_product_move_ledger_at,
};
use std::path::{Path, PathBuf};

const MINIMAL_LEDGER: &str = r#"
schema_version = "1.0"
controlling_issue = 2598
ledger_id = "CARGO-ALLOW-MOVE-LEDGER-0001"
linked_plan = "plans/three-product-crate-extraction.md"
linked_adr = "CARGO-ALLOW-ADR-0002"

[[entry]]
id = "move-allow-policy-spec-system"
current_identity = "crates/allow-policy/src/spec_system/"
identity_kind = "rust_module_tree"
current_owner_product = "cargo-allow"
current_owner_crate = "allow-policy"
target_owner_product = "cargo-intent"
target_owner_crate = "intent-model"
disposition = "MoveToIntentModel"
status = "current"
claim_boundary = "Domain types vs compilation split deferred to extraction PRs."
parity_fixture = "tests/fixtures/three-product-design/"
removal_condition = "issue:#2606 parity receipts"
controlling_issue = 2584
"#;

#[test]
fn parse_product_move_ledger_reads_entries() -> Result<(), String> {
    let ledger = parse_product_move_ledger(MINIMAL_LEDGER)
        .map_err(|err| format!("parse move ledger: {err}"))?;
    assert_eq!(ledger.schema_version, "1.0");
    assert_eq!(ledger.controlling_issue, 2598);
    assert_eq!(ledger.entry.len(), 1);
    assert_eq!(ledger.entry[0].id, "move-allow-policy-spec-system");
    assert_eq!(
        ledger.entry[0].disposition,
        MoveDisposition::MoveToIntentModel
    );
    assert_eq!(ledger.entry[0].status, MoveEntryStatus::Current);
    assert_eq!(
        ledger.entry[0].identity_kind,
        MoveIdentityKind::RustModuleTree
    );
    Ok(())
}

#[test]
fn parse_product_move_ledger_at_preserves_location() -> Result<(), String> {
    let err = match parse_product_move_ledger_at(
        Some(Path::new("policy/product-move-ledger.toml")),
        "schema_version = [",
    ) {
        Ok(_) => return Err("invalid move ledger TOML unexpectedly parsed".to_string()),
        Err(err) => err,
    };
    assert_eq!(err.kind(), allow_core::CargoAllowErrorKind::InvalidConfig);
    let location = err
        .location()
        .ok_or_else(|| "move ledger parse error should have a location".to_string())?;
    assert_eq!(
        location.path.as_deref(),
        Some("policy/product-move-ledger.toml")
    );
    Ok(())
}

#[test]
fn validate_product_move_ledger_rejects_duplicate_ids() -> Result<(), String> {
    let ledger = parse_product_move_ledger(
        r#"
schema_version = "1.0"
controlling_issue = 2598
ledger_id = "CARGO-ALLOW-MOVE-LEDGER-0001"
linked_plan = "plans/three-product-crate-extraction.md"
linked_adr = "CARGO-ALLOW-ADR-0002"

[[entry]]
id = "dup"
current_identity = "crates/allow-policy/src/spec_system/"
identity_kind = "rust_module_tree"
current_owner_product = "cargo-allow"
current_owner_crate = "allow-policy"
target_owner_product = "cargo-intent"
target_owner_crate = "intent-model"
disposition = "MoveToIntentModel"
status = "current"
claim_boundary = "test"

[[entry]]
id = "dup"
current_identity = "crates/cargo-allow/src/spec_system.rs"
identity_kind = "rust_module"
current_owner_product = "cargo-allow"
current_owner_crate = "cargo-allow"
target_owner_product = "cargo-intent"
target_owner_crate = "cargo-intent"
disposition = "MoveToCargoIntentApp"
status = "current"
claim_boundary = "test"
"#,
    )
    .map_err(|err| format!("parse move ledger: {err}"))?;
    let validated = validate_product_move_ledger(ledger);
    if validated.valid {
        return Err("duplicate ids should fail validation".to_string());
    }
    Ok(())
}

#[test]
fn repository_move_ledger_validates_current_paths() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ledger_path = root.join("policy/product-move-ledger.toml");
    let (validated, diagnostics, report) = validate_product_move_ledger_at(&root, &ledger_path)
        .map_err(|err| format!("validate repository move ledger: {err}"))?;
    if !validated.valid {
        return Err(format!("diagnostics: {diagnostics:?}"));
    }
    if diagnostics
        .iter()
        .any(|diag| diag.kind == MoveLedgerDiagnosticKind::MissingCurrentPath)
    {
        return Err(format!("missing paths: {diagnostics:?}"));
    }
    if report.entry_count < 8 {
        return Err("seeded inventory too small".to_string());
    }
    if report.current_count < 8 {
        return Err("seeded current entries too small".to_string());
    }
    Ok(())
}
