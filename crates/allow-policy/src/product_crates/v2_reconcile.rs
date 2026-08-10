//! V2 denominator reconciliation: proves exact agreement across workspace
//! members, V2 architecture authority, V2 package topology (#2923).
//!
//! A member, identity, or package row that exists in only one authority fails.
//! This is the current-workspace validation that the V2 cutover requires.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use allow_core::CargoAllowResult;

use crate::product_crates::v2::{ArchitectureManifestV2, CrateIdentityV2};
use crate::product_packages::{PackageTopologyEntryV2, ProductPackageTopologyV2};

/// Diagnostic kind for V2 denominator reconciliation failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReconcileDiagnosticKind {
    WorkspaceMemberMissingFromArchitecture,
    WorkspaceMemberMissingFromTopology,
    ArchitectureIdentityMissingFromTopology,
    TopologyPackageMissingFromArchitecture,
    DuplicateLogicalId,
    LogicalIdMismatch,
}

impl ReconcileDiagnosticKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WorkspaceMemberMissingFromArchitecture => {
                "workspace_member_missing_from_architecture"
            }
            Self::WorkspaceMemberMissingFromTopology => "workspace_member_missing_from_topology",
            Self::ArchitectureIdentityMissingFromTopology => {
                "architecture_identity_missing_from_topology"
            }
            Self::TopologyPackageMissingFromArchitecture => {
                "topology_package_missing_from_architecture"
            }
            Self::DuplicateLogicalId => "duplicate_logical_id",
            Self::LogicalIdMismatch => "logical_id_mismatch",
        }
    }
}

/// A V2 denominator reconciliation diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconcileDiagnostic {
    pub kind: ReconcileDiagnosticKind,
    pub message: String,
    pub logical_ids: Vec<String>,
}

/// Result of V2 denominator reconciliation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReconcileReport {
    pub workspace_member_count: usize,
    pub architecture_identity_count: usize,
    pub topology_package_count: usize,
    pub diagnostics: Vec<ReconcileDiagnostic>,
}

impl ReconcileReport {
    pub fn is_clean(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

/// Reconcile workspace members, V2 architecture, and V2 package topology (#2923).
///
/// Workspace membership is a path fact. Cargo package identity is a manifest
/// fact. The architecture authority binds those two facts explicitly, so this
/// reconciliation must not infer a package name from a directory basename.
pub fn reconcile_v2_denominators(
    workspace_members: &[String],
    architecture: &ArchitectureManifestV2,
    topology: &ProductPackageTopologyV2,
) -> ReconcileReport {
    let mut diagnostics = Vec::new();

    let arch_by_package: BTreeMap<&str, &CrateIdentityV2> = architecture
        .crate_identity
        .iter()
        .map(|entry| (entry.cargo_package_name.as_str(), entry))
        .collect();
    let arch_by_workspace_path: BTreeMap<&str, &CrateIdentityV2> = architecture
        .crate_identity
        .iter()
        .map(|entry| (entry.workspace_path.as_str(), entry))
        .collect();
    let topo_by_package: BTreeMap<&str, &PackageTopologyEntryV2> = topology
        .package
        .iter()
        .map(|entry| (entry.cargo_package_name.as_str(), entry))
        .collect();

    let arch_packages: BTreeSet<&str> = arch_by_package.keys().copied().collect();
    let topo_packages: BTreeSet<&str> = topo_by_package.keys().copied().collect();
    let workspace_set: BTreeSet<&str> = workspace_members.iter().map(String::as_str).collect();

    // Unit callers may pass package identities directly; repository callers
    // pass exact workspace paths from Cargo.toml.
    let identity_for_member = |member: &str| {
        arch_by_workspace_path
            .get(member)
            .copied()
            .or_else(|| arch_by_package.get(member).copied())
    };

    // 1. Every workspace member path/package must bind to one architecture row.
    for member in &workspace_set {
        if identity_for_member(member).is_none() {
            diagnostics.push(ReconcileDiagnostic {
                kind: ReconcileDiagnosticKind::WorkspaceMemberMissingFromArchitecture,
                message: format!(
                    "workspace member `{member}` is missing from V2 architecture authority"
                ),
                logical_ids: vec![member.to_string()],
            });
        }
    }

    // 2. Every workspace member's architecture-bound Cargo package must have
    // one topology row. Never substitute the path basename for package name.
    for member in &workspace_set {
        let package = identity_for_member(member)
            .map(|identity| identity.cargo_package_name.as_str())
            .unwrap_or(member);
        if !topo_packages.contains(package) {
            diagnostics.push(ReconcileDiagnostic {
                kind: ReconcileDiagnosticKind::WorkspaceMemberMissingFromTopology,
                message: format!(
                    "workspace member `{member}` (package `{package}`) is missing from V2 package topology"
                ),
                logical_ids: vec![member.to_string()],
            });
        }
    }

    // 3. Every architecture identity must have a topology entry.
    for (package, identity) in &arch_by_package {
        if !topo_packages.contains(package) {
            diagnostics.push(ReconcileDiagnostic {
                kind: ReconcileDiagnosticKind::ArchitectureIdentityMissingFromTopology,
                message: format!(
                    "architecture identity `{}` (package `{package}`) is missing from topology",
                    identity.logical_id
                ),
                logical_ids: vec![identity.logical_id.clone()],
            });
        }
    }

    // 4. Every topology entry must have an architecture identity.
    for (package, entry) in &topo_by_package {
        if !arch_packages.contains(package) {
            diagnostics.push(ReconcileDiagnostic {
                kind: ReconcileDiagnosticKind::TopologyPackageMissingFromArchitecture,
                message: format!(
                    "topology package `{package}` (logical `{}`) is missing from architecture",
                    entry.logical_id
                ),
                logical_ids: vec![entry.logical_id.clone()],
            });
        }
    }

    // 5. logical_id must match between architecture and topology for the same package.
    for (package, arch_entry) in &arch_by_package {
        if let Some(topo_entry) = topo_by_package.get(package)
            && arch_entry.logical_id != topo_entry.logical_id
        {
            diagnostics.push(ReconcileDiagnostic {
                kind: ReconcileDiagnosticKind::LogicalIdMismatch,
                message: format!(
                    "logical_id mismatch for package `{package}`: architecture=`{}`, topology=`{}`",
                    arch_entry.logical_id, topo_entry.logical_id
                ),
                logical_ids: vec![arch_entry.logical_id.clone(), topo_entry.logical_id.clone()],
            });
        }
    }

    ReconcileReport {
        workspace_member_count: workspace_members.len(),
        architecture_identity_count: architecture.crate_identity.len(),
        topology_package_count: topology.package.len(),
        diagnostics,
    }
}

/// Reconcile V2 denominators from files on disk.
pub fn reconcile_v2_denominators_at(root: &Path) -> CargoAllowResult<ReconcileReport> {
    let arch_path = root.join("policy/product-crates-v2.toml");
    let topo_path = root.join("policy/product-package-topology-v2.toml");

    let arch_text = std::fs::read_to_string(&arch_path).map_err(|err| {
        allow_core::CargoAllowError::new(format!(
            "V2 architecture manifest unreadable at {}: {err}",
            arch_path.display()
        ))
    })?;
    let topo_text = std::fs::read_to_string(&topo_path).map_err(|err| {
        allow_core::CargoAllowError::new(format!(
            "V2 package topology unreadable at {}: {err}",
            topo_path.display()
        ))
    })?;

    let architecture = crate::product_crates::v2::parse_architecture_manifest_v2(&arch_text)?;
    let topology = crate::product_packages::parse_product_package_topology_v2(&topo_text)?;
    let workspace_members =
        crate::product_crates::workspace::workspace_members_from_manifest(root)?;

    Ok(reconcile_v2_denominators(
        &workspace_members,
        &architecture,
        &topology,
    ))
}
