use super::config::{ProductPackageTopology, parse_product_package_topology_at};
use crate::product_crates::workspace_members_from_manifest;
use allow_core::CargoAllowResult;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageTopologyDiagnosticKind {
    DuplicatePackage,
    UnclassifiedWorkspacePackage,
    EmptyTopology,
}

impl PackageTopologyDiagnosticKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DuplicatePackage => "duplicate_package",
            Self::UnclassifiedWorkspacePackage => "unclassified_workspace_package",
            Self::EmptyTopology => "empty_topology",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageTopologyDiagnostic {
    pub kind: PackageTopologyDiagnosticKind,
    pub message: String,
    pub package_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackageTopologyReport {
    pub classified_count: usize,
    pub workspace_member_count: usize,
    pub cargo_allow_supported_count: usize,
}

pub fn validate_product_package_topology(
    topology: ProductPackageTopology,
    workspace_members: &[String],
) -> (
    ProductPackageTopology,
    Vec<PackageTopologyDiagnostic>,
    PackageTopologyReport,
) {
    let mut diagnostics = Vec::new();
    if topology.package.is_empty() {
        diagnostics.push(PackageTopologyDiagnostic {
            kind: PackageTopologyDiagnosticKind::EmptyTopology,
            message: "package topology has no entries".to_string(),
            package_names: Vec::new(),
        });
    }

    let mut seen = BTreeSet::new();
    let mut cargo_allow_supported_count = 0usize;
    for entry in &topology.package {
        if !seen.insert(entry.package.clone()) {
            diagnostics.push(PackageTopologyDiagnostic {
                kind: PackageTopologyDiagnosticKind::DuplicatePackage,
                message: format!("duplicate package entry `{}`", entry.package),
                package_names: vec![entry.package.clone()],
            });
        }
        if entry.posture.as_str() == "CargoAllowSupported" {
            cargo_allow_supported_count += 1;
        }
    }

    for member in workspace_members {
        let package = member
            .rsplit('/')
            .next()
            .unwrap_or(member.as_str())
            .to_string();
        if !seen.contains(&package) {
            diagnostics.push(PackageTopologyDiagnostic {
                kind: PackageTopologyDiagnosticKind::UnclassifiedWorkspacePackage,
                message: format!("workspace package `{package}` is not classified"),
                package_names: vec![package],
            });
        }
    }

    let report = PackageTopologyReport {
        classified_count: topology.package.len(),
        workspace_member_count: workspace_members.len(),
        cargo_allow_supported_count,
    };

    (topology, diagnostics, report)
}

pub fn validate_product_package_topology_at(
    root: &Path,
    topology_path: &Path,
) -> CargoAllowResult<(
    ProductPackageTopology,
    Vec<PackageTopologyDiagnostic>,
    PackageTopologyReport,
)> {
    let members = workspace_members_from_manifest(root)?;
    let text = std::fs::read_to_string(topology_path).map_err(|err| {
        allow_core::CargoAllowError::new(format!(
            "package topology unreadable at {}: {err}",
            topology_path.display()
        ))
    })?;
    let topology = parse_product_package_topology_at(Some(topology_path), &text)?;
    Ok(validate_product_package_topology(topology, &members))
}
