use super::config::parse_product_package_topology;
use super::validate::{PackageTopologyDiagnosticKind, validate_product_package_topology_at};
use crate::product_crates::workspace_members_from_manifest;
use std::path::PathBuf;

#[test]
fn repository_package_topology_classifies_workspace() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let members = workspace_members_from_manifest(&root)
        .map_err(|err| format!("workspace members: {err}"))?;
    let topology_path = root.join("policy/product-package-topology.toml");
    let (topology, diagnostics, report) =
        validate_product_package_topology_at(&root, &topology_path)
            .map_err(|err| format!("validate topology: {err}"))?;
    if diagnostics
        .iter()
        .any(|diag| diag.kind == PackageTopologyDiagnosticKind::UnclassifiedWorkspacePackage)
    {
        return Err(format!("unclassified packages: {diagnostics:?}"));
    }
    assert_eq!(topology.topology_id, "CARGO-ALLOW-PKG-TOPOLOGY-0001");
    assert_eq!(report.workspace_member_count, members.len());
    Ok(())
}

#[test]
fn parse_product_package_topology_reads_entries() -> Result<(), String> {
    let topology = parse_product_package_topology(
        r#"
schema_version = "1.0"
topology_id = "CARGO-ALLOW-PKG-TOPOLOGY-0001"
controlling_issue = 2604
linked_architecture_manifest = "CARGO-ALLOW-ARCH-0001"

[[package]]
package = "cargo-allow"
product_family = "cargo-allow"
posture = "CargoAllowSupported"
publish = true
candidate_inclusion = true
release_order = 100
"#,
    )
    .map_err(|err| format!("parse topology: {err}"))?;
    assert_eq!(topology.package.len(), 1);
    Ok(())
}
