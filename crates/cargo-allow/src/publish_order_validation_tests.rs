//! Publish-order validation: proves V2 `release_order` is a valid topological
//! sort of the actual workspace dependency graph (#3363).
//!
//! Workspace member path, Cargo package identity, and dependency alias are
//! distinct facts. The validator resolves them through Cargo manifests rather
//! than assuming a directory basename is a package name.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use allow_policy::product_packages::parse_product_package_topology_v2;

#[derive(Debug)]
struct WorkspaceMember {
    workspace_path: String,
    package_name: String,
    manifest: toml::Value,
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn release_order_is_unique_and_acyclic() -> Result<(), String> {
    let root = workspace_root();
    let (root_manifest, members) = load_workspace(&root)?;
    let release_order = topology_release_order(&root)?;

    let orders: Vec<u32> = release_order.values().copied().collect();
    let unique: BTreeSet<u32> = orders.iter().copied().collect();
    if orders.len() != unique.len() {
        return Err(format!(
            "release_order values are not unique: {} entries, {} unique",
            orders.len(),
            unique.len()
        ));
    }

    let workspace_packages: BTreeSet<&str> = members
        .iter()
        .map(|member| member.package_name.as_str())
        .collect();
    let dependency_aliases = workspace_dependency_packages(&root_manifest);

    for member in &members {
        let member_order = release_order
            .get(member.package_name.as_str())
            .ok_or_else(|| {
                format!(
                    "workspace member `{}` (package `{}`) is missing from V2 release order",
                    member.workspace_path, member.package_name
                )
            })?;

        let Some(dependencies) = member
            .manifest
            .get("dependencies")
            .and_then(toml::Value::as_table)
        else {
            continue;
        };

        for (alias, specification) in dependencies {
            let dependency_package =
                dependency_package_name(alias, specification, &dependency_aliases);
            if !workspace_packages.contains(dependency_package.as_str()) {
                continue;
            }
            let dependency_order = release_order
                .get(dependency_package.as_str())
                .ok_or_else(|| {
                    format!(
                        "workspace dependency `{alias}` resolves to package `{dependency_package}` without a V2 release order"
                    )
                })?;
            if dependency_order >= member_order {
                return Err(format!(
                    "{} (package `{}`, order={member_order}) depends on `{alias}` \
                     (package `{dependency_package}`, order={dependency_order}), but dependency must publish first",
                    member.workspace_path, member.package_name
                ));
            }
        }
    }

    Ok(())
}

#[test]
fn publish_order_covers_all_workspace_members() -> Result<(), String> {
    let root = workspace_root();
    let (_, members) = load_workspace(&root)?;
    let release_order = topology_release_order(&root)?;

    for member in &members {
        if !release_order.contains_key(member.package_name.as_str()) {
            return Err(format!(
                "workspace member `{}` (package `{}`) is missing from V2 topology release_order",
                member.workspace_path, member.package_name
            ));
        }
    }
    if release_order.len() != members.len() {
        return Err(format!(
            "V2 topology has {} release-order rows for {} workspace members",
            release_order.len(),
            members.len()
        ));
    }

    Ok(())
}

fn load_workspace(root: &Path) -> Result<(toml::Value, Vec<WorkspaceMember>), String> {
    let root_text = std::fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|err| format!("read workspace Cargo.toml: {err}"))?;
    let root_manifest: toml::Value =
        toml::from_str(&root_text).map_err(|err| format!("parse workspace Cargo.toml: {err}"))?;
    let member_paths = root_manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "workspace.members missing from Cargo.toml".to_string())?;

    let mut members = Vec::with_capacity(member_paths.len());
    for member_path in member_paths {
        let workspace_path = member_path
            .as_str()
            .ok_or_else(|| "workspace member is not a string".to_string())?;
        let manifest_path = root.join(workspace_path).join("Cargo.toml");
        let manifest_text = std::fs::read_to_string(&manifest_path)
            .map_err(|err| format!("read {}: {err}", manifest_path.display()))?;
        let manifest: toml::Value = toml::from_str(&manifest_text)
            .map_err(|err| format!("parse {}: {err}", manifest_path.display()))?;
        let package_name = manifest
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("package.name missing from {}", manifest_path.display()))?;
        members.push(WorkspaceMember {
            workspace_path: workspace_path.to_string(),
            package_name: package_name.to_string(),
            manifest,
        });
    }

    Ok((root_manifest, members))
}

fn topology_release_order(root: &Path) -> Result<BTreeMap<String, u32>, String> {
    let path = root.join("policy/product-package-topology-v2.toml");
    let text =
        std::fs::read_to_string(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let topology = parse_product_package_topology_v2(&text)
        .map_err(|err| format!("parse current V2 topology: {err}"))?;
    Ok(topology
        .package
        .into_iter()
        .map(|entry| (entry.cargo_package_name, entry.release_order))
        .collect())
}

fn workspace_dependency_packages(root_manifest: &toml::Value) -> BTreeMap<String, String> {
    root_manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table)
        .map(|dependencies| {
            dependencies
                .iter()
                .map(|(alias, specification)| {
                    let package = specification
                        .as_table()
                        .and_then(|table| table.get("package"))
                        .and_then(toml::Value::as_str)
                        .unwrap_or(alias);
                    (alias.clone(), package.to_string())
                })
                .collect()
        })
        .unwrap_or_default()
}

fn dependency_package_name(
    alias: &str,
    specification: &toml::Value,
    workspace_dependencies: &BTreeMap<String, String>,
) -> String {
    specification
        .as_table()
        .and_then(|table| table.get("package"))
        .and_then(toml::Value::as_str)
        .map(str::to_string)
        .or_else(|| workspace_dependencies.get(alias).cloned())
        .unwrap_or_else(|| alias.to_string())
}
