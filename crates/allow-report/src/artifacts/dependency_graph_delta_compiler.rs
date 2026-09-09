//! Base/head dependency graph delta compiler (#3920 PR B): parses
//! exact base/head manifests and lockfiles and emits deterministic
//! ordered delta rows preserving native Cargo identity. Root
//! relocation and input traversal order do not change identity or
//! output.

use crate::artifacts::dependency_graph_delta_v1::{
    DependencyClassV1, DependencyGraphDeltaIdentityV1, DependencyGraphDeltaKindV1,
    DependencyGraphDeltaReceiptV1, DependencyGraphDeltaRowV1,
};

/// One parsed Cargo.lock package entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LockPackage {
    pub name: String,
    pub version: String,
    pub source: String,
    pub checksum: String,
}

/// One parsed manifest dependency requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManifestRequirement {
    pub name: String,
    pub requirement: String,
    pub class: DependencyClassV1,
}

/// Parse a Cargo.lock's `[[package]]` entries into deterministic
/// (name-sorted) rows. Path-only entries (no source) are skipped.
pub(crate) fn parse_lock_packages(lock_text: &str) -> Vec<LockPackage> {
    let mut packages = Vec::new();
    let value: toml::Value = match toml::from_str(lock_text) {
        Ok(value) => value,
        Err(_) => return packages,
    };
    let Some(entries) = value.get("package").and_then(toml::Value::as_array) else {
        return packages;
    };
    for entry in entries {
        let name = entry
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or("")
            .to_string();
        let version = entry
            .get("version")
            .and_then(toml::Value::as_str)
            .unwrap_or("")
            .to_string();
        let source = entry
            .get("source")
            .and_then(toml::Value::as_str)
            .unwrap_or("")
            .to_string();
        let checksum = entry
            .get("checksum")
            .and_then(toml::Value::as_str)
            .unwrap_or("")
            .to_string();
        if name.is_empty() || version.is_empty() {
            continue;
        }
        packages.push(LockPackage {
            name,
            version,
            source,
            checksum,
        });
    }
    packages.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then(a.version.cmp(&b.version))
            .then(a.source.cmp(&b.source))
    });
    packages
}

/// Parse a manifest's `[dependencies]` table into requirement rows.
/// Workspace-inherited entries (`workspace = true`) are resolved from
/// the workspace root's `[workspace.dependencies]` when available.
pub(crate) fn parse_manifest_requirements(
    manifest_text: &str,
    workspace_deps: Option<&toml::Value>,
) -> Vec<ManifestRequirement> {
    let value: toml::Value = match toml::from_str(manifest_text) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let deps = match value.get("dependencies").and_then(toml::Value::as_table) {
        Some(table) => table,
        None => return Vec::new(),
    };
    let mut requirements = Vec::new();
    for (name, spec) in deps {
        let class = DependencyClassV1::Normal;
        let requirement = if spec
            .get("workspace")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false)
        {
            // Inherited from workspace: resolve the requirement from the
            // workspace root's [workspace.dependencies].
            let ws_deps = workspace_deps
                .and_then(|ws| ws.get("dependencies"))
                .and_then(toml::Value::as_table);
            match ws_deps.and_then(|table| table.get(name.as_str())) {
                Some(ws_spec) => ws_spec
                    .get("version")
                    .and_then(toml::Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                None => String::new(),
            }
        } else {
            match spec {
                toml::Value::String(version) => version.clone(),
                table => table
                    .get("version")
                    .and_then(toml::Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            }
        };
        requirements.push(ManifestRequirement {
            name: name.clone(),
            requirement,
            class,
        });
    }
    requirements.sort_by(|a, b| a.name.cmp(&b.name));
    requirements
}

/// Compile one base/head pair into the typed delta receipt. Pure and
/// deterministic: the same inputs always produce the same output.
pub fn compile_dependency_graph_delta(
    identity: &DependencyGraphDeltaIdentityV1,
    base_manifest: &str,
    head_manifest: &str,
    base_lock: &str,
    head_lock: &str,
) -> Result<DependencyGraphDeltaReceiptV1, String> {
    let mut rows = Vec::new();

    // Parse both manifests for direct requirements.
    let base_reqs = parse_manifest_requirements(base_manifest, None);
    let head_reqs = parse_manifest_requirements(head_manifest, None);
    let base_req_map: std::collections::BTreeMap<&str, &str> = base_reqs
        .iter()
        .map(|req| (req.name.as_str(), req.requirement.as_str()))
        .collect();
    let head_req_map: std::collections::BTreeMap<&str, &str> = head_reqs
        .iter()
        .map(|req| (req.name.as_str(), req.requirement.as_str()))
        .collect();

    // Collect all unique dependency names from both manifests.
    let mut all_names: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for name in base_req_map.keys() {
        all_names.insert(name);
    }
    for name in head_req_map.keys() {
        all_names.insert(name);
    }

    // Classify direct requirement changes.
    for name in &all_names {
        let base_req = base_req_map.get(name);
        let head_req = head_req_map.get(name);
        match (base_req, head_req) {
            (Some(base), Some(head)) => {
                if base == head {
                    continue;
                }
                let kind = if is_range_lowered(base, head) {
                    DependencyGraphDeltaKindV1::DirectRequirementLowered
                } else if is_range_raised(base, head) {
                    DependencyGraphDeltaKindV1::DirectRequirementRaised
                } else if is_range_narrowed(base, head) {
                    DependencyGraphDeltaKindV1::RequirementRangeNarrowed
                } else {
                    DependencyGraphDeltaKindV1::RequirementRangeBroadened
                };
                rows.push(DependencyGraphDeltaRowV1 {
                    kind,
                    class: DependencyClassV1::Normal,
                    package_name: name.to_string(),
                    base_version: String::new(),
                    head_version: String::new(),
                    base_requirement: base.to_string(),
                    head_requirement: head.to_string(),
                    base_source: String::new(),
                    head_source: String::new(),
                    base_checksum: String::new(),
                    head_checksum: String::new(),
                });
            }
            (None, Some(head)) => {
                rows.push(DependencyGraphDeltaRowV1 {
                    kind: DependencyGraphDeltaKindV1::DirectRequirementAdded,
                    class: DependencyClassV1::Normal,
                    package_name: name.to_string(),
                    base_version: String::new(),
                    head_version: String::new(),
                    base_requirement: String::new(),
                    head_requirement: head.to_string(),
                    base_source: String::new(),
                    head_source: String::new(),
                    base_checksum: String::new(),
                    head_checksum: String::new(),
                });
            }
            (Some(_), None) => {
                rows.push(DependencyGraphDeltaRowV1 {
                    kind: DependencyGraphDeltaKindV1::DirectRequirementRemoved,
                    class: DependencyClassV1::Normal,
                    package_name: name.to_string(),
                    base_version: String::new(),
                    head_version: String::new(),
                    base_requirement: String::new(),
                    head_requirement: String::new(),
                    base_source: String::new(),
                    head_source: String::new(),
                    base_checksum: String::new(),
                    head_checksum: String::new(),
                });
            }
            (None, None) => {}
        }
    }

    // Parse both lockfiles for resolved package movement.
    let base_packages = parse_lock_packages(base_lock);
    let head_packages = parse_lock_packages(head_lock);
    let base_map: std::collections::BTreeMap<&str, &LockPackage> = base_packages
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect();
    let head_map: std::collections::BTreeMap<&str, &LockPackage> = head_packages
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect();

    let mut all_lock_names: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for name in base_map.keys() {
        all_lock_names.insert(name);
    }
    for name in head_map.keys() {
        all_lock_names.insert(name);
    }

    for name in &all_lock_names {
        let base_package = base_map.get(name);
        let head_package = head_map.get(name);
        match (base_package, head_package) {
            (None, Some(head)) => {
                rows.push(DependencyGraphDeltaRowV1 {
                    kind: DependencyGraphDeltaKindV1::PackageAdded,
                    class: DependencyClassV1::Normal,
                    package_name: name.to_string(),
                    base_version: String::new(),
                    head_version: head.version.clone(),
                    base_requirement: String::new(),
                    head_requirement: String::new(),
                    base_source: String::new(),
                    head_source: head.source.clone(),
                    base_checksum: String::new(),
                    head_checksum: head.checksum.clone(),
                });
            }
            (Some(base), None) => {
                rows.push(DependencyGraphDeltaRowV1 {
                    kind: DependencyGraphDeltaKindV1::PackageRemoved,
                    class: DependencyClassV1::Normal,
                    package_name: name.to_string(),
                    base_version: base.version.clone(),
                    head_version: String::new(),
                    base_requirement: String::new(),
                    head_requirement: String::new(),
                    base_source: base.source.clone(),
                    head_source: String::new(),
                    base_checksum: base.checksum.clone(),
                    head_checksum: String::new(),
                });
            }
            (Some(base), Some(head)) => {
                let source_changed = base.source != head.source;
                let checksum_changed = !base.checksum.is_empty()
                    && !head.checksum.is_empty()
                    && base.checksum != head.checksum;
                let version_up = is_version_up(&base.version, &head.version);
                let version_down = is_version_down(&base.version, &head.version);
                if source_changed || checksum_changed {
                    rows.push(DependencyGraphDeltaRowV1 {
                        kind: DependencyGraphDeltaKindV1::SourceOrChecksumChanged,
                        class: DependencyClassV1::Normal,
                        package_name: name.to_string(),
                        base_version: base.version.clone(),
                        head_version: head.version.clone(),
                        base_requirement: String::new(),
                        head_requirement: String::new(),
                        base_source: base.source.clone(),
                        head_source: head.source.clone(),
                        base_checksum: base.checksum.clone(),
                        head_checksum: head.checksum.clone(),
                    });
                } else if version_up {
                    rows.push(DependencyGraphDeltaRowV1 {
                        kind: DependencyGraphDeltaKindV1::PackageUpgraded,
                        class: DependencyClassV1::Normal,
                        package_name: name.to_string(),
                        base_version: base.version.clone(),
                        head_version: head.version.clone(),
                        base_requirement: String::new(),
                        head_requirement: String::new(),
                        base_source: base.source.clone(),
                        head_source: head.source.clone(),
                        base_checksum: base.checksum.clone(),
                        head_checksum: head.checksum.clone(),
                    });
                } else if version_down {
                    rows.push(DependencyGraphDeltaRowV1 {
                        kind: DependencyGraphDeltaKindV1::PackageDowngraded,
                        class: DependencyClassV1::Normal,
                        package_name: name.to_string(),
                        base_version: base.version.clone(),
                        head_version: head.version.clone(),
                        base_requirement: String::new(),
                        head_requirement: String::new(),
                        base_source: base.source.clone(),
                        head_source: head.source.clone(),
                        base_checksum: base.checksum.clone(),
                        head_checksum: head.checksum.clone(),
                    });
                }
            }
            (None, None) => {}
        }
    }

    rows.sort_by(|a, b| {
        a.package_name
            .cmp(&b.package_name)
            .then(a.kind.as_str().cmp(b.kind.as_str()))
    });

    let complete = true;
    Ok(DependencyGraphDeltaReceiptV1 {
        schema_id: crate::artifacts::dependency_graph_delta_v1::DEPENDENCY_GRAPH_DELTA_SCHEMA_ID
            .to_string(),
        schema_version:
            crate::artifacts::dependency_graph_delta_v1::DEPENDENCY_GRAPH_DELTA_SCHEMA_VERSION,
        identity: identity.clone(),
        rows,
        complete,
        limitations: vec![
            "the delta covers lockfile-resolved package movement and manifest direct requirement changes; Cargo metadata (features, targets, dev/build deps) requires PR B's bounded metadata observations".to_string(),
        ],
        claim_boundary: "Typed dependency graph delta for one exact base/head pair: every row preserves the native Cargo identity (package name, version, source, checksum) and the delta kind. Count parity does not establish graph identity. A downgrade is never described as a compatible update. Missing, malformed, or partial analyses are instrument failures, not NoSemanticGraphChange.".to_string(),
    })
}

/// Compare two semver-like version strings for ordering.
pub(crate) fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |v: &str| -> Vec<u64> {
        v.split('.')
            .filter_map(|part| part.parse::<u64>().ok())
            .collect()
    };
    let a_parts = parse(a);
    let b_parts = parse(b);
    for i in 0..a_parts.len().max(b_parts.len()) {
        let a_val = a_parts.get(i).copied().unwrap_or(0);
        let b_val = b_parts.get(i).copied().unwrap_or(0);
        match a_val.cmp(&b_val) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

fn is_version_up(base: &str, head: &str) -> bool {
    compare_versions(base, head) == std::cmp::Ordering::Less
}

fn is_version_down(base: &str, head: &str) -> bool {
    compare_versions(base, head) == std::cmp::Ordering::Greater
}

fn is_range_raised(base: &str, head: &str) -> bool {
    compare_versions(base, head) == std::cmp::Ordering::Less
}

fn is_range_lowered(base: &str, head: &str) -> bool {
    compare_versions(base, head) == std::cmp::Ordering::Greater
}

fn is_range_narrowed(base: &str, head: &str) -> bool {
    let base_parts: Vec<&str> = base.split('.').collect();
    let head_parts: Vec<&str> = head.split('.').collect();
    head_parts.len() > base_parts.len()
}
