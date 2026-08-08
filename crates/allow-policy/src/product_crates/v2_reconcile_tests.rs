//! Tests for V2 denominator reconciliation (#2923).

use super::v2_reconcile::{ReconcileDiagnosticKind, reconcile_v2_denominators};
use super::{ArchitectureManifestV2, CrateIdentityV2, CrateRole};
use crate::product_packages::{
    PackagePosture, PackageTopologyEntryV2, ProductPackageTopologyV2, PublicationStateV2,
    VersionSourceV2,
};

fn make_architecture(identities: Vec<CrateIdentityV2>) -> ArchitectureManifestV2 {
    ArchitectureManifestV2 {
        schema_version: "2.0".to_string(),
        authority_generation: 2,
        manifest_id: "TEST".to_string(),
        controlling_issue: 2923,
        linked_move_ledger: "TEST".to_string(),
        crate_identity: identities,
    }
}

fn make_topology(entries: Vec<PackageTopologyEntryV2>) -> ProductPackageTopologyV2 {
    ProductPackageTopologyV2 {
        schema_version: "2.0".to_string(),
        authority_generation: 2,
        topology_id: "TEST".to_string(),
        controlling_issue: 2923,
        linked_architecture_manifest: "TEST".to_string(),
        package: entries,
    }
}

fn make_identity(logical: &str, package: &str, owner: &str) -> CrateIdentityV2 {
    CrateIdentityV2 {
        logical_id: logical.to_string(),
        workspace_path: format!("crates/{package}"),
        workspace_dependency_aliases: vec![package.to_string()],
        cargo_package_name: package.to_string(),
        rust_library_name: package.replace('-', "_"),
        product_or_shared_owner: owner.to_string(),
        crate_role: CrateRole::CargoAllowCore,
    }
}

fn make_entry(logical: &str, package: &str, family: &str, order: u32) -> PackageTopologyEntryV2 {
    PackageTopologyEntryV2 {
        logical_id: logical.to_string(),
        cargo_package_name: package.to_string(),
        product_family: family.to_string(),
        posture: PackagePosture::CargoAllowSupported,
        package_version: "0.2.0".to_string(),
        version_source: VersionSourceV2::WorkspaceProduct,
        publication_state: PublicationStateV2::UnpublishedInternal,
        publish: false,
        candidate_inclusion: false,
        release_order: order,
    }
}

#[test]
fn reconcile_passes_when_all_denominators_agree() -> Result<(), String> {
    let workspace = vec!["allow-core".to_string(), "cargo-allow".to_string()];
    let arch = make_architecture(vec![
        make_identity("allow-core", "allow-core", "cargo-allow"),
        make_identity("cargo-allow", "cargo-allow", "cargo-allow"),
    ]);
    let topo = make_topology(vec![
        make_entry("allow-core", "allow-core", "cargo-allow", 10),
        make_entry("cargo-allow", "cargo-allow", "cargo-allow", 100),
    ]);
    let report = reconcile_v2_denominators(&workspace, &arch, &topo);
    if !report.is_clean() {
        return Err(format!(
            "expected clean reconcile, got: {:?}",
            report.diagnostics
        ));
    }
    if report.workspace_member_count != 2 {
        return Err("workspace_member_count mismatch".to_string());
    }
    Ok(())
}

#[test]
fn reconcile_detects_workspace_member_missing_from_architecture() -> Result<(), String> {
    let workspace = vec![
        "allow-core".to_string(),
        "cargo-allow".to_string(),
        "extra".to_string(),
    ];
    let arch = make_architecture(vec![
        make_identity("allow-core", "allow-core", "cargo-allow"),
        make_identity("cargo-allow", "cargo-allow", "cargo-allow"),
    ]);
    let topo = make_topology(vec![
        make_entry("allow-core", "allow-core", "cargo-allow", 10),
        make_entry("cargo-allow", "cargo-allow", "cargo-allow", 100),
    ]);
    let report = reconcile_v2_denominators(&workspace, &arch, &topo);
    if !report
        .diagnostics
        .iter()
        .any(|d| d.kind == ReconcileDiagnosticKind::WorkspaceMemberMissingFromArchitecture)
    {
        return Err("should detect workspace member missing from architecture".to_string());
    }
    Ok(())
}

#[test]
fn reconcile_detects_workspace_member_missing_from_topology() -> Result<(), String> {
    let workspace = vec!["allow-core".to_string(), "cargo-allow".to_string()];
    let arch = make_architecture(vec![
        make_identity("allow-core", "allow-core", "cargo-allow"),
        make_identity("cargo-allow", "cargo-allow", "cargo-allow"),
    ]);
    let topo = make_topology(vec![
        make_entry("allow-core", "allow-core", "cargo-allow", 10),
        // cargo-allow missing from topology
    ]);
    let report = reconcile_v2_denominators(&workspace, &arch, &topo);
    if !report
        .diagnostics
        .iter()
        .any(|d| d.kind == ReconcileDiagnosticKind::WorkspaceMemberMissingFromTopology)
    {
        return Err("should detect workspace member missing from topology".to_string());
    }
    Ok(())
}

#[test]
fn reconcile_detects_topology_package_missing_from_architecture() -> Result<(), String> {
    let workspace = vec!["allow-core".to_string()];
    let arch = make_architecture(vec![make_identity(
        "allow-core",
        "allow-core",
        "cargo-allow",
    )]);
    let topo = make_topology(vec![
        make_entry("allow-core", "allow-core", "cargo-allow", 10),
        make_entry("extra", "extra", "cargo-allow", 20),
    ]);
    let report = reconcile_v2_denominators(&workspace, &arch, &topo);
    if !report
        .diagnostics
        .iter()
        .any(|d| d.kind == ReconcileDiagnosticKind::TopologyPackageMissingFromArchitecture)
    {
        return Err("should detect topology package missing from architecture".to_string());
    }
    Ok(())
}

#[test]
fn reconcile_detects_logical_id_mismatch() -> Result<(), String> {
    let workspace = vec!["allow-core".to_string()];
    let arch = make_architecture(vec![make_identity("core", "allow-core", "cargo-allow")]);
    let topo = make_topology(vec![make_entry(
        "different-id",
        "allow-core",
        "cargo-allow",
        10,
    )]);
    let report = reconcile_v2_denominators(&workspace, &arch, &topo);
    if !report
        .diagnostics
        .iter()
        .any(|d| d.kind == ReconcileDiagnosticKind::LogicalIdMismatch)
    {
        return Err("should detect logical_id mismatch".to_string());
    }
    Ok(())
}

#[test]
fn reconcile_handles_effortless_identity_split() -> Result<(), String> {
    // logical_id "repo-protocol" ≠ package "effortless-repo-protocol"
    let workspace = vec!["crates/effortless-repo-protocol".to_string()];
    let arch = make_architecture(vec![CrateIdentityV2 {
        logical_id: "repo-protocol".to_string(),
        workspace_path: "crates/effortless-repo-protocol".to_string(),
        workspace_dependency_aliases: vec!["effortless-repo-protocol".to_string()],
        cargo_package_name: "effortless-repo-protocol".to_string(),
        rust_library_name: "effortless_repo_protocol".to_string(),
        product_or_shared_owner: "shared".to_string(),
        crate_role: CrateRole::SharedProtocol,
    }]);
    let topo = make_topology(vec![make_entry(
        "repo-protocol",
        "effortless-repo-protocol",
        "shared",
        95,
    )]);
    let report = reconcile_v2_denominators(&workspace, &arch, &topo);
    if !report.is_clean() {
        return Err(format!(
            "effortless identity split should reconcile cleanly: {:?}",
            report.diagnostics
        ));
    }
    Ok(())
}
