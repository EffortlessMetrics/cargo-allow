use super::config::parse_extraction_shim_registry;
use super::validate::{ShimDiagnosticKind, validate_extraction_shim_registry_at};
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
