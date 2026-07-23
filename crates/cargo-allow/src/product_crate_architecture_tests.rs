use allow_policy::product_crates::{
    ArchitectureDiagnosticKind, validate_architecture_manifest_at, workspace_members_from_manifest,
};
use std::path::PathBuf;

#[test]
fn product_crate_architecture_report_only_inventory() -> Result<(), String> {
    let root = repo_root();
    let members = workspace_members_from_manifest(&root)
        .map_err(|err| format!("workspace members: {err}"))?;
    let manifest_path = root.join("policy/product-crates.toml");
    let (manifest, diagnostics, report) =
        validate_architecture_manifest_at(&root, &manifest_path, &members)
            .map_err(|err| format!("validate architecture manifest: {err}"))?;

    if diagnostics
        .iter()
        .any(|diag| diag.kind == ArchitectureDiagnosticKind::UnownedWorkspaceCrate)
    {
        return Err(format!("unowned workspace crates: {diagnostics:?}"));
    }
    assert_eq!(manifest.manifest_id, "CARGO-ALLOW-ARCH-0001");
    assert_eq!(manifest.controlling_issue, 2580);
    assert!(report.planned_crate_count >= 4);

    let law = root.join("docs/architecture/product-crate-law.md");
    let law_text = std::fs::read_to_string(&law)
        .map_err(|err| format!("product crate law readable: {err}"))?;
    if !law_text.contains("cargo-allow") {
        return Err("human projection missing cargo-allow ownership".to_string());
    }

    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
