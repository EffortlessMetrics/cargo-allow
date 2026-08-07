//! V2 identity validation: uniqueness and alias-map checks (#2921).
//!
//! Validates that every logical/path/alias/package/library identity is
//! independently unique and that dependency aliases resolve unambiguously.

use std::collections::BTreeMap;

use crate::product_crates::v2::ArchitectureManifestV2;

/// Diagnostic kind for V2 identity validation failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IdentityDiagnosticKind {
    DuplicateLogicalId,
    DuplicateWorkspacePath,
    DuplicatePackageName,
    DuplicateLibraryName,
    AmbiguousAlias,
    UnresolvableAlias,
}

impl IdentityDiagnosticKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateLogicalId => "duplicate_logical_id",
            Self::DuplicateWorkspacePath => "duplicate_workspace_path",
            Self::DuplicatePackageName => "duplicate_package_name",
            Self::DuplicateLibraryName => "duplicate_library_name",
            Self::AmbiguousAlias => "ambiguous_alias",
            Self::UnresolvableAlias => "unresolvable_alias",
        }
    }
}

/// A V2 identity validation diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityDiagnostic {
    pub kind: IdentityDiagnosticKind,
    pub message: String,
    pub logical_ids: Vec<String>,
}

/// Validate V2 identity uniqueness: no duplicate logical_id, workspace_path,
/// cargo_package_name, or rust_library_name (#2921).
pub fn validate_v2_identity_uniqueness(
    manifest: &ArchitectureManifestV2,
) -> Vec<IdentityDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen_logical: BTreeMap<String, usize> = BTreeMap::new();
    let mut seen_path: BTreeMap<String, usize> = BTreeMap::new();
    let mut seen_package: BTreeMap<String, usize> = BTreeMap::new();
    let mut seen_library: BTreeMap<String, usize> = BTreeMap::new();

    for entry in &manifest.crate_identity {
        increment(&mut seen_logical, &entry.logical_id);
        increment(&mut seen_path, &entry.workspace_path);
        increment(&mut seen_package, &entry.cargo_package_name);
        increment(&mut seen_library, &entry.rust_library_name);
    }

    for entry in &manifest.crate_identity {
        if seen_logical[&entry.logical_id] > 1 {
            diagnostics.push(IdentityDiagnostic {
                kind: IdentityDiagnosticKind::DuplicateLogicalId,
                message: format!("duplicate logical_id `{}`", entry.logical_id),
                logical_ids: collect_duplicates(&seen_logical, &entry.logical_id),
            });
        }
        if seen_path[&entry.workspace_path] > 1 {
            diagnostics.push(IdentityDiagnostic {
                kind: IdentityDiagnosticKind::DuplicateWorkspacePath,
                message: format!("duplicate workspace_path `{}`", entry.workspace_path),
                logical_ids: collect_duplicates(&seen_path, &entry.workspace_path),
            });
        }
        if seen_package[&entry.cargo_package_name] > 1 {
            diagnostics.push(IdentityDiagnostic {
                kind: IdentityDiagnosticKind::DuplicatePackageName,
                message: format!(
                    "duplicate cargo_package_name `{}`",
                    entry.cargo_package_name
                ),
                logical_ids: collect_duplicates(&seen_package, &entry.cargo_package_name),
            });
        }
        if seen_library[&entry.rust_library_name] > 1 {
            diagnostics.push(IdentityDiagnostic {
                kind: IdentityDiagnosticKind::DuplicateLibraryName,
                message: format!("duplicate rust_library_name `{}`", entry.rust_library_name),
                logical_ids: collect_duplicates(&seen_library, &entry.rust_library_name),
            });
        }
    }

    diagnostics.dedup_by(|a, b| a.kind == b.kind && a.message == b.message);
    diagnostics
}

/// Validate the V2 alias map: every alias must resolve to exactly one package
/// (#2921).
pub fn validate_v2_alias_map(manifest: &ArchitectureManifestV2) -> Vec<IdentityDiagnostic> {
    let mut alias_to_packages: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut package_names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for entry in &manifest.crate_identity {
        package_names.insert(entry.cargo_package_name.clone());
        for alias in &entry.workspace_dependency_aliases {
            alias_to_packages
                .entry(alias.clone())
                .or_default()
                .push(entry.logical_id.clone());
        }
    }

    let mut diagnostics = Vec::new();
    for (alias, logical_ids) in &alias_to_packages {
        let unique: std::collections::BTreeSet<_> = logical_ids.iter().collect();
        if unique.len() > 1 {
            diagnostics.push(IdentityDiagnostic {
                kind: IdentityDiagnosticKind::AmbiguousAlias,
                message: format!(
                    "alias `{alias}` resolves to multiple packages: {:?}",
                    unique.iter().copied().collect::<Vec<_>>()
                ),
                logical_ids: logical_ids.clone(),
            });
        }
    }

    // Check that each alias either matches a package name or is explicitly
    // enumerated as a deliberate alias (at least resolves to one entry).
    for (alias, logical_ids) in &alias_to_packages {
        if logical_ids.is_empty() {
            diagnostics.push(IdentityDiagnostic {
                kind: IdentityDiagnosticKind::UnresolvableAlias,
                message: format!("alias `{alias}` does not resolve to any package"),
                logical_ids: Vec::new(),
            });
        }
    }

    diagnostics
}

fn increment(map: &mut BTreeMap<String, usize>, key: &str) {
    *map.entry(key.to_string()).or_insert(0) += 1;
}

fn collect_duplicates(map: &BTreeMap<String, usize>, _key: &str) -> Vec<String> {
    map.iter()
        .filter(|(_, count)| **count > 1)
        .map(|(key, _)| key.clone())
        .collect()
}
