#![expect(
    clippy::collapsible_if,
    reason = "policy:allow-0517 publish-order topology test keeps invariant parsing readable"
)]
//! Publish order validation: proves V2 topology release_order is a valid
//! topological sort of the actual workspace dependency graph (#3363).
//!
//! Checks:
//! 1. release_order values are unique (deterministic)
//! 2. For every dependency edge A -> B (A depends on B), B has a lower
//!    release_order than A (B publishes before A)
//! 3. No cycles in the dependency graph

use std::collections::BTreeMap;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn release_order_is_unique_and_acyclic() -> Result<(), String> {
    let root = workspace_root();

    // Parse workspace Cargo.toml to get member list
    let ws_manifest = std::fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|e| format!("read workspace Cargo.toml: {e}"))?;

    // Extract workspace members
    let mut members: Vec<String> = Vec::new();
    let mut in_members = false;
    for line in ws_manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("members") {
            in_members = true;
            continue;
        }
        if in_members {
            if trimmed.starts_with(']') {
                break;
            }
            if let Some(name) = trimmed
                .trim_matches(|c: char| c == '"' || c == ',' || c.is_whitespace())
                .strip_prefix("crates/")
            {
                members.push(name.to_string());
            }
        }
    }

    // Parse V2 topology to get release_order per package
    let topo_text = std::fs::read_to_string(root.join("policy/product-package-topology-v2.toml"))
        .map_err(|e| format!("read V2 topology: {e}"))?;

    let mut release_order: BTreeMap<String, u32> = BTreeMap::new();
    let mut current_package = String::new();
    for line in topo_text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("cargo_package_name") {
            if let Some(name) = trimmed.split('"').nth(1) {
                current_package = name.to_string();
            }
        }
        if trimmed.starts_with("release_order") {
            if let Some(value) = trimmed.split('=').nth(1) {
                if let Ok(order) = value.trim().parse::<u32>() {
                    if !current_package.is_empty() {
                        release_order.insert(current_package.clone(), order);
                    }
                }
            }
        }
    }

    // Check uniqueness
    let orders: Vec<u32> = release_order.values().copied().collect();
    let unique: std::collections::BTreeSet<u32> = orders.iter().copied().collect();
    if orders.len() != unique.len() {
        return Err(format!(
            "release_order values are not unique: {} entries, {} unique",
            orders.len(),
            unique.len()
        ));
    }

    // Check acyclic: for each member crate, read its deps and verify
    // that every workspace dep has a lower release_order
    for member in &members {
        let manifest_path = root.join("crates").join(member).join("Cargo.toml");
        let manifest = std::fs::read_to_string(&manifest_path)
            .map_err(|e| format!("read {member}/Cargo.toml: {e}"))?;

        let member_order = *release_order.get(member).unwrap_or(&0);

        // Extract [dependencies] section
        let mut in_deps = false;
        for line in manifest.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                in_deps = trimmed == "[dependencies]";
                continue;
            }
            if in_deps && let Some(dep_name) = trimmed.split('=').next().map(|s| s.trim()) {
                // Check if this dep is a workspace member
                if members.contains(&dep_name.to_string()) {
                    let dep_order = *release_order.get(dep_name).unwrap_or(&0);
                    if dep_order >= member_order {
                        return Err(format!(
                            "{member} (order={member_order}) depends on {dep_name} (order={dep_order}), \
                             but dependency must publish first (lower order)"
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}

#[test]
fn publish_order_covers_all_workspace_members() -> Result<(), String> {
    let root = workspace_root();
    let ws_manifest = std::fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|e| format!("read workspace Cargo.toml: {e}"))?;

    let mut members: Vec<String> = Vec::new();
    let mut in_members = false;
    for line in ws_manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("members") {
            in_members = true;
            continue;
        }
        if in_members {
            if trimmed.starts_with(']') {
                break;
            }
            if let Some(name) = trimmed
                .trim_matches(|c: char| c == '"' || c == ',' || c.is_whitespace())
                .strip_prefix("crates/")
            {
                members.push(name.to_string());
            }
        }
    }

    let topo_text = std::fs::read_to_string(root.join("policy/product-package-topology-v2.toml"))
        .map_err(|e| format!("read V2 topology: {e}"))?;

    let mut topo_packages = Vec::new();
    for line in topo_text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("cargo_package_name = ") {
            let name = rest.trim_matches('"');
            topo_packages.push(name.to_string());
        }
    }

    for member in &members {
        if !topo_packages.contains(member) {
            return Err(format!(
                "workspace member `{member}` missing from V2 topology release_order"
            ));
        }
    }

    Ok(())
}
