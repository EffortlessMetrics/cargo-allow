use super::config::parse_extraction_shim_registry;
use super::validate::{
    validate_extraction_shim_registry, validate_extraction_shim_registry_at, ShimDiagnosticKind,
};
use crate::product_move::parse_product_move_ledger;
use std::path::PathBuf;

#[test]
fn parse_extraction_shim_registry_reads_entries() -> Result<(), String> {
    let registry = parse_extraction_shim_registry(
        r#"
schema_version = "1.0"
registry_id = "CARGO-ALLOW-SHIM-REGISTRY-0001"
controlling_issue = 2607
linked_move_ledger = "CARGO-ALLOW-MOVE-LEDGER-0001"

[[shim]]
id = "shim-allow-diff-staged-index"
old_identity = "allow-diff::staged_index"
new_identity = "repo-snapshot::staged_index"
kind = "ModuleFacade"
posture = "private"
status = "planned"
move_ledger_entry = "move-allow-diff-staged-index"
controlling_issue = 2583
latest_allowed_stage = 1
removal_condition = "issue:#2606 stage-1 cutover receipt"
parity_case = "parity-repo-snapshot-staged-index-v1"
claim_boundary = "Identity forwarding only; no second semantic implementation."
"#,
    )
    .map_err(|err| format!("parse shim registry: {err}"))?;
    assert_eq!(registry.shim.len(), 1);
    assert_eq!(registry.shim[0].id, "shim-allow-diff-staged-index");
    Ok(())
}

#[test]
fn repository_shim_registry_links_move_ledger() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry_path = root.join("policy/extraction-shims.toml");
    let ledger_path = root.join("policy/product-move-ledger.toml");
    let (_, diagnostics, report) =
        validate_extraction_shim_registry_at(&root, &registry_path, &ledger_path)
            .map_err(|err| format!("validate shim registry: {err}"))?;
    if diagnostics
        .iter()
        .any(|diag| diag.kind == ShimDiagnosticKind::MissingMoveLedgerEntry)
    {
        return Err(format!("missing move ledger links: {diagnostics:?}"));
    }
    if report.shim_count < 6 {
        return Err("seeded shim inventory too small".to_string());
    }
    Ok(())
}

#[test]
fn snapshot_shims_record_live_public_compatibility_boundary() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry_path = root.join("policy/extraction-shims.toml");
    let ledger_path = root.join("policy/product-move-ledger.toml");
    let (registry, diagnostics, _) =
        validate_extraction_shim_registry_at(&root, &registry_path, &ledger_path)
            .map_err(|err| format!("validate shim registry: {err}"))?;
    if !diagnostics.is_empty() {
        return Err(format!("unexpected shim diagnostics: {diagnostics:?}"));
    }

    for id in [
        "shim-allow-diff-staged-index",
        "shim-allow-diff-revision-identity",
    ] {
        let shim = registry
            .shim
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| format!("missing snapshot shim {id}"))?;
        if shim.posture != super::config::ShimPosture::Public
            || shim.status != super::config::ShimStatus::Active
            || !shim.removal_condition.contains("#2606")
        {
            return Err(format!(
                "snapshot shim {id} must remain an active public compatibility boundary until #2606"
            ));
        }
    }

    Ok(())
}

#[test]
fn lifecycle_bounds_fail_closed() -> Result<(), String> {
    let registry_text = r#"
registry_id = "test"
controlling_issue = 2607
linked_move_ledger = "test"

[[shim]]
id = "shim-test"
old_identity = "old"
new_identity = "new"
kind = "ModuleFacade"
posture = "private"
status = "planned"
move_ledger_entry = "move-test"
controlling_issue = 2607
latest_allowed_stage = 0
removal_condition = "issue:#2606 stage-1 cutover receipt"
parity_case = "parity-test"
claim_boundary = "test boundary"
"#;
    let registry = parse_extraction_shim_registry(registry_text)
        .map_err(|err| format!("parse registry: {err}"))?;
    let ledger = parse_product_move_ledger(
        &std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../policy/product-move-ledger.toml"),
        )
        .map_err(|err| format!("read ledger: {err}"))?,
    )
    .map_err(|err| format!("parse ledger: {err}"))?;

    let move_ids = ledger
        .entry
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<Vec<_>>();
    let (_, diagnostics, _) = validate_extraction_shim_registry(registry, &move_ids);
    if !diagnostics
        .iter()
        .any(|diagnostic| diagnostic.kind == ShimDiagnosticKind::Expired)
    {
        return Err(format!("expected expired diagnostic, got {diagnostics:?}"));
    }

    let inconsistent_registry = parse_extraction_shim_registry(
        &registry_text
            .replace("latest_allowed_stage = 0", "latest_allowed_stage = 1")
            .replace(
                "removal_condition = \"issue:#2606 stage-1 cutover receipt\"",
                "removal_condition = \"issue:#2606 stage-2 cutover receipt\"",
            ),
    )
    .map_err(|err| format!("parse inconsistent registry: {err}"))?;
    let (_, diagnostics, _) = validate_extraction_shim_registry(inconsistent_registry, &move_ids);
    if !diagnostics
        .iter()
        .any(|diagnostic| diagnostic.kind == ShimDiagnosticKind::Expired)
    {
        return Err(format!(
            "expected inconsistent stage diagnostic, got {diagnostics:?}"
        ));
    }
    Ok(())
}
