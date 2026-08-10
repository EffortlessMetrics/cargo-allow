//! Deterministic current architecture and cargo-allow candidate receipt.

use allow_core::{CargoAllowError, CargoAllowResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

use super::{
    CargoDependencyClass, load_workspace_metadata_graph_v2, parse_architecture_manifest_v2,
    reconcile_v2_denominators_at, validate_v2_alias_map, validate_v2_identity_uniqueness,
};
use crate::product_packages::parse_product_package_topology_v2;

pub const CURRENT_ARCHITECTURE_RECEIPT_V2: &str = "CurrentArchitectureReceiptV2";
pub const CARGO_ALLOW_CANDIDATE_ID_V2: &str = "cargo-allow-0.2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentArchitectureReceiptV2 {
    pub receipt_kind: String,
    pub architecture_manifest_id: String,
    pub topology_id: String,
    pub candidate_identity: String,
    pub workspace_package_count: usize,
    pub architecture_identity_count: usize,
    pub topology_package_count: usize,
    pub candidate_package_count: usize,
    pub workspace_packages: Vec<ArchitecturePackageRowV2>,
    pub candidate_packages: Vec<ArchitecturePackageRowV2>,
    pub claim_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitecturePackageRowV2 {
    pub logical_id: String,
    pub cargo_package_name: String,
    pub workspace_path: String,
    pub product_family: String,
    pub package_version: String,
    pub version_source: String,
    pub publication_state: String,
    pub candidate_inclusion: bool,
    pub release_order: u32,
}

impl CurrentArchitectureReceiptV2 {
    pub fn render_json(&self) -> CargoAllowResult<String> {
        serde_json::to_string_pretty(self)
            .map(|mut json| {
                json.push('\n');
                json
            })
            .map_err(|err| CargoAllowError::new(format!("render architecture receipt: {err}")))
    }
}

pub fn current_architecture_receipt_at(
    root: &Path,
) -> CargoAllowResult<CurrentArchitectureReceiptV2> {
    let architecture =
        parse_architecture_manifest_v2(&read_required(root, "policy/product-crates-v2.toml")?)?;
    let topology = parse_product_package_topology_v2(&read_required(
        root,
        "policy/product-package-topology-v2.toml",
    )?)?;
    let report = reconcile_v2_denominators_at(root)?;
    if !report.is_clean() {
        return Err(CargoAllowError::new(format!(
            "V2 authorities do not reconcile: {:?}",
            report.diagnostics
        )));
    }
    for diagnostics in [
        validate_v2_identity_uniqueness(&architecture),
        validate_v2_alias_map(&architecture),
    ] {
        if !diagnostics.is_empty() {
            return Err(CargoAllowError::new(format!(
                "V2 architecture validation failed: {diagnostics:?}"
            )));
        }
    }

    let workspace_packages = workspace_packages_at(root)?;
    let by_name: BTreeMap<&str, &WorkspacePackage> = workspace_packages
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect();
    let identities: BTreeMap<&str, _> = architecture
        .crate_identity
        .iter()
        .map(|identity| (identity.cargo_package_name.as_str(), identity))
        .collect();

    let mut rows = Vec::with_capacity(topology.package.len());
    for entry in &topology.package {
        let identity = identities
            .get(entry.cargo_package_name.as_str())
            .ok_or_else(|| {
                CargoAllowError::new(format!(
                    "missing V2 identity for `{}`",
                    entry.cargo_package_name
                ))
            })?;
        let workspace = by_name
            .get(entry.cargo_package_name.as_str())
            .ok_or_else(|| {
                CargoAllowError::new(format!(
                    "missing workspace package `{}`",
                    entry.cargo_package_name
                ))
            })?;
        if workspace.version != entry.package_version {
            return Err(CargoAllowError::new(format!(
                "V2 package `{}` version `{}` differs from manifest `{}`",
                entry.cargo_package_name, entry.package_version, workspace.version
            )));
        }
        if workspace.version_source != entry.version_source.as_str() {
            return Err(CargoAllowError::new(format!(
                "V2 package `{}` version source `{}` differs from manifest `{}`",
                entry.cargo_package_name,
                entry.version_source.as_str(),
                workspace.version_source
            )));
        }
        rows.push(ArchitecturePackageRowV2 {
            logical_id: entry.logical_id.clone(),
            cargo_package_name: entry.cargo_package_name.clone(),
            workspace_path: identity.workspace_path.clone(),
            product_family: entry.product_family.clone(),
            package_version: entry.package_version.clone(),
            version_source: entry.version_source.as_str().to_string(),
            publication_state: entry.publication_state.as_str().to_string(),
            candidate_inclusion: entry.candidate_inclusion,
            release_order: entry.release_order,
        });
    }

    rows.sort_by_key(|row| (row.release_order, row.cargo_package_name.clone()));
    let candidate_packages: Vec<_> = rows
        .iter()
        .filter(|row| row.candidate_inclusion)
        .cloned()
        .collect();
    validate_candidate_order(root, &candidate_packages)?;

    Ok(CurrentArchitectureReceiptV2 {
        receipt_kind: CURRENT_ARCHITECTURE_RECEIPT_V2.to_string(),
        architecture_manifest_id: architecture.manifest_id,
        topology_id: topology.topology_id,
        candidate_identity: CARGO_ALLOW_CANDIDATE_ID_V2.to_string(),
        workspace_package_count: workspace_packages.len(),
        architecture_identity_count: architecture.crate_identity.len(),
        topology_package_count: topology.package.len(),
        candidate_package_count: candidate_packages.len(),
        workspace_packages: rows,
        candidate_packages,
        claim_boundary: "Current V2 architecture and derived cargo-allow candidate denominator; no publication or product-support claim.".to_string(),
    })
}

fn validate_candidate_order(
    root: &Path,
    candidate: &[ArchitecturePackageRowV2],
) -> CargoAllowResult<()> {
    let orders: BTreeMap<&str, u32> = candidate
        .iter()
        .map(|row| (row.cargo_package_name.as_str(), row.release_order))
        .collect();
    if orders.len() != candidate.len() {
        return Err(CargoAllowError::new(
            "candidate release_order values are not unique",
        ));
    }
    let graph = load_workspace_metadata_graph_v2(root)?;
    for edge in graph.edges.iter().filter(|edge| {
        matches!(
            edge.class,
            CargoDependencyClass::Normal
                | CargoDependencyClass::Optional
                | CargoDependencyClass::TargetSpecific
                | CargoDependencyClass::FeatureActivated
        )
    }) {
        let (Some(from), Some(to)) = (
            orders.get(edge.from_package.as_str()),
            orders.get(edge.to_package.as_str()),
        ) else {
            continue;
        };
        if from <= to {
            return Err(CargoAllowError::new(format!(
                "candidate release order is not dependency-valid: `{}` ({from}) depends on `{}` ({to})",
                edge.from_package, edge.to_package
            )));
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct WorkspacePackage {
    name: String,
    version: String,
    version_source: String,
}

fn workspace_packages_at(root: &Path) -> CargoAllowResult<Vec<WorkspacePackage>> {
    let root_value: toml::Value = toml::from_str(&read_required(root, "Cargo.toml")?)
        .map_err(|err| CargoAllowError::new(format!("parse workspace Cargo.toml: {err}")))?;
    let workspace = root_value
        .get("workspace")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| CargoAllowError::new("workspace table missing from Cargo.toml"))?;
    let workspace_package = workspace
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| CargoAllowError::new("workspace.package missing from Cargo.toml"))?;
    let workspace_version = workspace_package
        .get("version")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| CargoAllowError::new("workspace.package.version missing"))?;
    let members = workspace
        .get("members")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| CargoAllowError::new("workspace.members missing"))?;

    let mut packages = Vec::with_capacity(members.len());
    for member in members {
        let member_path = member
            .as_str()
            .ok_or_else(|| CargoAllowError::new("workspace member is not a string"))?;
        let manifest_path = root.join(member_path).join("Cargo.toml");
        let value: toml::Value = toml::from_str(&read_file(&manifest_path)?).map_err(|err| {
            CargoAllowError::new(format!("parse {}: {err}", manifest_path.display()))
        })?;
        let package = value
            .get("package")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| {
                CargoAllowError::new(format!("package table missing in {member_path}"))
            })?;
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| {
                CargoAllowError::new(format!("package.name missing in {member_path}"))
            })?;
        let version_value = package.get("version").ok_or_else(|| {
            CargoAllowError::new(format!(
                "package `{name}` must declare version or version.workspace = true"
            ))
        })?;
        let (version, version_source) = if let Some(version) = version_value.as_str() {
            (version.to_string(), "Explicit".to_string())
        } else if version_value
            .get("workspace")
            .and_then(toml::Value::as_bool)
            == Some(true)
        {
            (
                workspace_version.to_string(),
                "WorkspaceProduct".to_string(),
            )
        } else {
            return Err(CargoAllowError::new(format!(
                "package `{name}` version must be a string or version.workspace = true"
            )));
        };
        packages.push(WorkspacePackage {
            name: name.to_string(),
            version,
            version_source,
        });
    }
    Ok(packages)
}

fn read_required(root: &Path, relative: &str) -> CargoAllowResult<String> {
    read_file(&root.join(relative))
}

fn read_file(path: &Path) -> CargoAllowResult<String> {
    std::fs::read_to_string(path)
        .map_err(|err| CargoAllowError::new(format!("read {}: {err}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::{
        ArchitecturePackageRowV2, current_architecture_receipt_at, validate_candidate_order,
        workspace_packages_at,
    };
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn current_receipt_is_deterministic_and_has_expected_denominators() -> Result<(), String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let receipt = current_architecture_receipt_at(&root)
            .map_err(|err| format!("build current receipt: {err}"))?;
        let first = receipt
            .render_json()
            .map_err(|err| format!("render receipt: {err}"))?;
        let second = receipt
            .render_json()
            .map_err(|err| format!("render receipt: {err}"))?;
        if first != second
            || !first.ends_with('\n')
            || receipt.workspace_package_count != 22
            || receipt.candidate_package_count != 13
        {
            return Err("receipt is not deterministic or has unexpected denominators".to_string());
        }
        Ok(())
    }

    #[test]
    fn missing_authority_root_is_rejected() -> Result<(), String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("does-not-exist");
        if current_architecture_receipt_at(&root).is_ok() {
            return Err("missing authority root unexpectedly reconciled".to_string());
        }
        Ok(())
    }

    #[test]
    fn candidate_order_rejects_duplicate_and_dependency_invalid_orders() -> Result<(), String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let duplicate = vec![row("allow-diff", 1), row("effortless-repo-snapshot", 1)];
        if validate_candidate_order(&root, &duplicate).is_ok() {
            return Err("duplicate candidate release order unexpectedly accepted".to_string());
        }
        let invalid = vec![row("allow-diff", 1), row("effortless-repo-snapshot", 2)];
        if validate_candidate_order(&root, &invalid).is_ok() {
            return Err("dependency-invalid candidate order unexpectedly accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn workspace_reader_preserves_explicit_and_inherited_sources() -> Result<(), String> {
        let root = fixture_root("sources");
        make_workspace(&root, "[\"crates/explicit\", \"crates/inherited\"]", true)?;
        write_manifest(
            &root,
            "explicit",
            "[package]\nname = \"explicit\"\nversion = \"0.1.0\"\n",
        )?;
        write_manifest(
            &root,
            "inherited",
            "[package]\nname = \"inherited\"\nversion.workspace = true\n",
        )?;
        let packages = workspace_packages_at(&root).map_err(|err| err.to_string())?;
        if packages.len() != 2
            || packages
                .first()
                .map(|package| package.version_source.as_str())
                != Some("Explicit")
            || packages
                .get(1)
                .map(|package| package.version_source.as_str())
                != Some("WorkspaceProduct")
            || packages.get(1).map(|package| package.version.as_str()) != Some("0.2.0")
        {
            return Err(format!("unexpected workspace packages: {packages:?}"));
        }
        remove_fixture(&root)
    }

    #[test]
    fn workspace_reader_rejects_malformed_members_and_versions() -> Result<(), String> {
        let cases = [
            (
                "no-workspace",
                "[package]\nversion = \"0.2.0\"\n",
                "workspace",
            ),
            (
                "no-package",
                "[workspace]\nmembers = []\n",
                "workspace.package",
            ),
            (
                "no-version",
                "[workspace]\nmembers = []\n[workspace.package]\n",
                "version",
            ),
            (
                "no-members",
                "[workspace]\n[workspace.package]\nversion = \"0.2.0\"\n",
                "members",
            ),
            (
                "bad-member",
                "[workspace]\nmembers = [1]\n[workspace.package]\nversion = \"0.2.0\"\n",
                "member",
            ),
        ];
        for (label, cargo, expected) in cases {
            let root = fixture_root(label);
            fs::write(root.join("Cargo.toml"), cargo).map_err(|err| err.to_string())?;
            let error = workspace_packages_at(&root)
                .err()
                .ok_or_else(|| format!("{label} unexpectedly succeeded"))?;
            if !error.to_string().contains(expected) {
                return Err(format!("{label} error lacked `{expected}`: {error}"));
            }
            remove_fixture(&root)?;
        }
        Ok(())
    }

    #[test]
    fn workspace_reader_rejects_bad_member_manifests() -> Result<(), String> {
        let cases = [
            (
                "missing-package",
                "[workspace]\nmembers = [\"crates/member\"]\n[workspace.package]\nversion = \"0.2.0\"\n",
                "package table",
            ),
            (
                "missing-name",
                "[package]\nversion = \"0.1.0\"\n",
                "package.name",
            ),
            (
                "missing-version",
                "[package]\nname = \"member\"\n",
                "declare version",
            ),
            (
                "invalid-version",
                "[package]\nname = \"member\"\nversion = 1\n",
                "version must",
            ),
        ];
        for (label, manifest, expected) in cases {
            let root = fixture_root(label);
            make_workspace(&root, "[\"crates/member\"]", true)?;
            write_manifest_at(&root, "member", manifest)?;
            let error = workspace_packages_at(&root)
                .err()
                .ok_or_else(|| format!("{label} unexpectedly succeeded"))?;
            if !error.to_string().contains(expected) {
                return Err(format!("{label} error lacked `{expected}`: {error}"));
            }
            remove_fixture(&root)?;
        }
        Ok(())
    }

    fn row(name: &str, release_order: u32) -> ArchitecturePackageRowV2 {
        ArchitecturePackageRowV2 {
            logical_id: name.to_string(),
            cargo_package_name: name.to_string(),
            workspace_path: format!("crates/{name}"),
            product_family: "test".to_string(),
            package_version: "0.1.0".to_string(),
            version_source: "Explicit".to_string(),
            publication_state: "UnpublishedInternal".to_string(),
            candidate_inclusion: true,
            release_order,
        }
    }

    fn fixture_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "cargo-allow-receipt-{label}-{}",
            std::process::id()
        ))
    }

    fn make_workspace(root: &Path, members: &str, with_version: bool) -> Result<(), String> {
        fs::create_dir_all(root.join("crates")).map_err(|err| err.to_string())?;
        let version = if with_version {
            "\n[workspace.package]\nversion = \"0.2.0\"\n"
        } else {
            ""
        };
        fs::write(
            root.join("Cargo.toml"),
            format!("[workspace]\nmembers = {members}{version}"),
        )
        .map_err(|err| err.to_string())
    }

    fn write_manifest(root: &Path, name: &str, manifest: &str) -> Result<(), String> {
        write_manifest_at(root, name, manifest)
    }

    fn write_manifest_at(root: &Path, name: &str, manifest: &str) -> Result<(), String> {
        let path = root.join("crates").join(name);
        fs::create_dir_all(&path).map_err(|err| err.to_string())?;
        fs::write(path.join("Cargo.toml"), manifest).map_err(|err| err.to_string())
    }

    fn remove_fixture(root: &Path) -> Result<(), String> {
        fs::remove_dir_all(root).map_err(|err| err.to_string())
    }
}
