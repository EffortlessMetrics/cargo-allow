use allow_policy::product_packages::{
    PackageTopologyDiagnosticKind, validate_product_package_topology_at,
};
use std::path::PathBuf;

#[test]
fn product_package_topology_report_only() -> Result<(), String> {
    let root = repo_root();
    let (_, diagnostics, report) = validate_product_package_topology_at(
        &root,
        &root.join("policy/product-package-topology.toml"),
    )
    .map_err(|err| format!("validate topology: {err}"))?;
    if diagnostics
        .iter()
        .any(|diag| diag.kind == PackageTopologyDiagnosticKind::UnclassifiedWorkspacePackage)
    {
        return Err(format!("unclassified packages: {diagnostics:?}"));
    }
    assert_eq!(report.cargo_allow_supported_count, 9);
    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
