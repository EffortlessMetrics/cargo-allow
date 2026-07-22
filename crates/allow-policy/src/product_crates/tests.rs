use super::config::parse_architecture_manifest;
use super::validate::{
    ArchitectureDiagnosticKind, validate_architecture_manifest, workspace_members_from_manifest,
};
use std::path::PathBuf;

#[test]
fn parse_architecture_manifest_reads_products() -> Result<(), String> {
    let manifest = parse_architecture_manifest(
        r#"
schema_version = "1.0"
manifest_id = "CARGO-ALLOW-ARCH-0001"
controlling_issue = 2580
linked_move_ledger = "CARGO-ALLOW-MOVE-LEDGER-0001"

[[product]]
id = "cargo-allow"
binary = "cargo-allow"
owned_crates = ["cargo-allow"]
forbid_product_dependencies = ["cargo-intent"]
"#,
    )
    .map_err(|err| format!("parse architecture manifest: {err}"))?;
    assert_eq!(manifest.manifest_id, "CARGO-ALLOW-ARCH-0001");
    assert_eq!(manifest.product.len(), 1);
    assert_eq!(
        manifest.product[0].owned_crates,
        vec!["cargo-allow".to_string()]
    );
    Ok(())
}

#[test]
fn repository_architecture_manifest_covers_workspace() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let members = workspace_members_from_manifest(&root)
        .map_err(|err| format!("workspace members: {err}"))?;
    let manifest_path = root.join("policy/product-crates.toml");
    let text = std::fs::read_to_string(&manifest_path)
        .map_err(|err| format!("manifest readable: {err}"))?;
    let manifest =
        parse_architecture_manifest(&text).map_err(|err| format!("parse manifest: {err}"))?;
    let (_, diagnostics, report) = validate_architecture_manifest(manifest, &members);
    if diagnostics
        .iter()
        .any(|diag| diag.kind == ArchitectureDiagnosticKind::UnownedWorkspaceCrate)
    {
        return Err(format!("unowned workspace crates: {diagnostics:?}"));
    }
    if report.owned_crate_count < members.len() {
        return Err("owned crate count should cover workspace members".to_string());
    }
    Ok(())
}
