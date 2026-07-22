use super::config::{ArchitectureManifest, parse_architecture_manifest_at};
use allow_core::{CargoAllowError, CargoAllowResult};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchitectureDiagnosticKind {
    DuplicateCrateOwner,
    UnownedWorkspaceCrate,
    UnknownOwnedCrate,
    EmptyManifest,
}

impl ArchitectureDiagnosticKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateCrateOwner => "duplicate_crate_owner",
            Self::UnownedWorkspaceCrate => "unowned_workspace_crate",
            Self::UnknownOwnedCrate => "unknown_owned_crate",
            Self::EmptyManifest => "empty_manifest",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchitectureDiagnostic {
    pub kind: ArchitectureDiagnosticKind,
    pub message: String,
    pub crate_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArchitectureReport {
    pub owned_crate_count: usize,
    pub planned_crate_count: usize,
    pub workspace_member_count: usize,
    pub product_count: usize,
}

pub fn validate_architecture_manifest(
    manifest: ArchitectureManifest,
    workspace_members: &[String],
) -> (
    ArchitectureManifest,
    Vec<ArchitectureDiagnostic>,
    ArchitectureReport,
) {
    let mut diagnostics = Vec::new();
    if manifest.product.is_empty() {
        diagnostics.push(ArchitectureDiagnostic {
            kind: ArchitectureDiagnosticKind::EmptyManifest,
            message: "architecture manifest has no products".to_string(),
            crate_names: Vec::new(),
        });
    }

    let mut owners: BTreeMap<String, String> = BTreeMap::new();
    for product in &manifest.product {
        for crate_name in &product.owned_crates {
            if let Some(existing) = owners.insert(crate_name.clone(), product.id.clone()) {
                diagnostics.push(ArchitectureDiagnostic {
                    kind: ArchitectureDiagnosticKind::DuplicateCrateOwner,
                    message: format!(
                        "crate `{crate_name}` owned by both `{existing}` and `{}`",
                        product.id
                    ),
                    crate_names: vec![crate_name.clone()],
                });
            }
        }
    }

    let owned: BTreeSet<String> = owners.keys().cloned().collect();
    for member in workspace_members {
        let crate_name = member
            .rsplit('/')
            .next()
            .unwrap_or(member.as_str())
            .to_string();
        if !owned.contains(&crate_name) {
            diagnostics.push(ArchitectureDiagnostic {
                kind: ArchitectureDiagnosticKind::UnownedWorkspaceCrate,
                message: format!("workspace crate `{crate_name}` has no product owner"),
                crate_names: vec![crate_name],
            });
        }
    }

    let report = ArchitectureReport {
        owned_crate_count: owned.len(),
        planned_crate_count: manifest.planned_crate.len(),
        workspace_member_count: workspace_members.len(),
        product_count: manifest.product.len(),
    };

    (manifest, diagnostics, report)
}

pub fn validate_architecture_manifest_at(
    _root: &Path,
    manifest_path: &Path,
    workspace_members: &[String],
) -> CargoAllowResult<(
    ArchitectureManifest,
    Vec<ArchitectureDiagnostic>,
    ArchitectureReport,
)> {
    let text = std::fs::read_to_string(manifest_path).map_err(|err| {
        allow_core::CargoAllowError::new(format!(
            "architecture manifest unreadable at {}: {err}",
            manifest_path.display()
        ))
    })?;
    let manifest = parse_architecture_manifest_at(Some(manifest_path), &text)?;
    Ok(validate_architecture_manifest(manifest, workspace_members))
}

pub fn workspace_members_from_manifest(root: &Path) -> CargoAllowResult<Vec<String>> {
    let cargo_toml = root.join("Cargo.toml");
    let text = std::fs::read_to_string(&cargo_toml).map_err(|err| {
        allow_core::CargoAllowError::new(format!(
            "workspace Cargo.toml unreadable at {}: {err}",
            cargo_toml.display()
        ))
    })?;
    let parsed: toml::Value = toml::from_str(&text).map_err(|err| {
        allow_core::CargoAllowError::new(format!("workspace Cargo.toml parse error: {err}"))
    })?;
    let members = parsed
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(|members| members.as_array())
        .ok_or_else(|| CargoAllowError::new("workspace members missing from Cargo.toml"))?;
    let mut paths = Vec::with_capacity(members.len());
    for member in members {
        let Some(path) = member.as_str() else {
            return Err(allow_core::CargoAllowError::new(
                "workspace member entry was not a string",
            ));
        };
        paths.push(path.to_string());
    }
    Ok(paths)
}
