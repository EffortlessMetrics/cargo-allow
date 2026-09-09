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
    pub features: Vec<String>,
    pub class: DependencyClassV1,
}

/// Parse a Cargo.lock's `[[package]]` entries into deterministic
/// (name-sorted) rows. Workspace members and path dependencies carry
/// no source; they are retained with an empty source so first-party
/// movement stays visible.
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
        let (requirement, features) = if spec
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
                Some(ws_spec) => match ws_spec {
                    toml::Value::String(version) => (version.clone(), Vec::new()),
                    table => (
                        table
                            .get("version")
                            .and_then(toml::Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        spec_features(table),
                    ),
                },
                None => (String::new(), Vec::new()),
            }
        } else {
            match spec {
                toml::Value::String(version) => (version.clone(), Vec::new()),
                table => (
                    table
                        .get("version")
                        .and_then(toml::Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    spec_features(table),
                ),
            }
        };
        requirements.push(ManifestRequirement {
            name: name.clone(),
            requirement,
            features,
            class,
        });
    }
    requirements.sort_by(|a, b| a.name.cmp(&b.name));
    requirements
}

/// Parse one manifest text's `[workspace]` table for inherited
/// requirement resolution; unparseable text contributes nothing.
fn parse_workspace_dependencies(manifest: &str) -> Option<toml::Value> {
    let value: toml::Value = toml::from_str(manifest).ok()?;
    value.get("workspace").cloned()
}

/// Extract a sorted, deduplicated feature list from one dependency
/// spec table. String specs carry no features.
fn spec_features(spec: &toml::Value) -> Vec<String> {
    let mut features: Vec<String> = spec
        .get("features")
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    features.sort();
    features.dedup();
    features
}

/// Compile one base/head pair into the typed delta receipt. Pure and
/// deterministic: the same inputs always produce the same output.
/// Workspace-inherited requirements (`workspace = true`) are resolved
/// from each manifest's own `[workspace.dependencies]` table when the
/// manifest text carries one, so a merged member-dep manifest can be
/// compiled with the same entry point.
pub fn compile_dependency_graph_delta(
    identity: &DependencyGraphDeltaIdentityV1,
    base_manifest: &str,
    head_manifest: &str,
    base_lock: &str,
    head_lock: &str,
) -> Result<DependencyGraphDeltaReceiptV1, String> {
    compile_dependency_graph_delta_with_workspace(
        identity,
        base_manifest,
        head_manifest,
        Some(base_manifest),
        Some(head_manifest),
        base_lock,
        head_lock,
    )
}

/// Compile one base/head pair with explicit workspace manifest texts
/// for `workspace = true` requirement resolution. `None` leaves
/// inherited entries unresolved (empty requirement).
pub fn compile_dependency_graph_delta_with_workspace(
    identity: &DependencyGraphDeltaIdentityV1,
    base_manifest: &str,
    head_manifest: &str,
    base_workspace_manifest: Option<&str>,
    head_workspace_manifest: Option<&str>,
    base_lock: &str,
    head_lock: &str,
) -> Result<DependencyGraphDeltaReceiptV1, String> {
    let mut rows = Vec::new();

    // Parse both manifests for direct requirements, resolving
    // `workspace = true` entries against each side's own
    // [workspace.dependencies] table.
    let base_workspace = base_workspace_manifest.and_then(parse_workspace_dependencies);
    let head_workspace = head_workspace_manifest.and_then(parse_workspace_dependencies);
    let base_reqs = parse_manifest_requirements(base_manifest, base_workspace.as_ref());
    let head_reqs = parse_manifest_requirements(head_manifest, head_workspace.as_ref());
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

    let base_req_by_name: std::collections::BTreeMap<&str, &ManifestRequirement> = base_reqs
        .iter()
        .map(|req| (req.name.as_str(), req))
        .collect();
    let head_req_by_name: std::collections::BTreeMap<&str, &ManifestRequirement> = head_reqs
        .iter()
        .map(|req| (req.name.as_str(), req))
        .collect();

    // Classify direct requirement changes.
    for name in &all_names {
        let base_req = base_req_map.get(name);
        let head_req = head_req_map.get(name);
        match (base_req, head_req) {
            (Some(base), Some(head)) => {
                // A spec whose kind changed (e.g. version -> git) leaves
                // one side empty; the lock-level source row carries that
                // signal, so requirement polarity is only classified
                // between two parseable version requirements.
                if !base.is_empty() && !head.is_empty() && base != head {
                    let kind = classify_requirement_movement(base, head);
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
                // Feature activation is manifest-visible: an added or
                // removed feature list is emitted even when the version
                // requirement text is unchanged.
                let base_features = base_req_by_name
                    .get(name)
                    .map(|req| req.features.clone())
                    .unwrap_or_default();
                let head_features = head_req_by_name
                    .get(name)
                    .map(|req| req.features.clone())
                    .unwrap_or_default();
                if base_features != head_features {
                    rows.push(DependencyGraphDeltaRowV1 {
                        kind: DependencyGraphDeltaKindV1::FeatureActivationChanged,
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
                // The direct requirement is unchanged when both manifests
                // name the package with the same parseable requirement;
                // packages absent from both manifests (transitive) also
                // carry no requirement movement.
                let requirement_unchanged = match (base_req_map.get(*name), head_req_map.get(*name))
                {
                    (Some(b), Some(h)) => b == h,
                    (None, None) => true,
                    _ => false,
                };
                if version_up || version_down {
                    // Version movement is its own row (an upgrade or a
                    // downgrade, never a "compatible update"), and when
                    // the manifest requirement did not move, an
                    // additional row marks the movement as lock-only
                    // resolution. A version bump legitimately rotates
                    // the checksum, so it is not classified as a
                    // source/checksum identity change.
                    let movement_kind = if version_up {
                        DependencyGraphDeltaKindV1::PackageUpgraded
                    } else {
                        DependencyGraphDeltaKindV1::PackageDowngraded
                    };
                    rows.push(DependencyGraphDeltaRowV1 {
                        kind: movement_kind,
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
                    if requirement_unchanged {
                        rows.push(DependencyGraphDeltaRowV1 {
                            kind: DependencyGraphDeltaKindV1::LockOnlyResolutionChanged,
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
                } else if source_changed || checksum_changed {
                    // Same resolved version but a different origin or
                    // content identity: count parity does not establish
                    // graph identity.
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

/// Classify one direct requirement movement between two non-empty,
/// differing version requirements. A major-version floor change is a
/// raise or a lowering; movement within the same major that adds
/// segment precision to the floor narrows the accepted range, and
/// removing precision broadens it. Requirements whose floor cannot be
/// read as a plain integer (comparator prefixes such as `^` or `>=`,
/// non-numeric floors) cannot be ordered from syntax alone and fail
/// closed instead of guessing a polarity.
fn classify_requirement_movement(base: &str, head: &str) -> DependencyGraphDeltaKindV1 {
    let floor_major = |requirement: &str| -> Option<u64> {
        requirement
            .split('.')
            .next()
            .and_then(|part| part.parse::<u64>().ok())
    };
    let (Some(base_major), Some(head_major)) = (floor_major(base), floor_major(head)) else {
        return DependencyGraphDeltaKindV1::UnsupportedOrInstrumentFailure;
    };
    let segments = |requirement: &str| -> usize { requirement.split('.').count() };
    if head_major > base_major {
        return DependencyGraphDeltaKindV1::DirectRequirementRaised;
    }
    if head_major < base_major {
        return DependencyGraphDeltaKindV1::DirectRequirementLowered;
    }
    match segments(head).cmp(&segments(base)) {
        std::cmp::Ordering::Greater => DependencyGraphDeltaKindV1::RequirementRangeNarrowed,
        std::cmp::Ordering::Less => DependencyGraphDeltaKindV1::RequirementRangeBroadened,
        std::cmp::Ordering::Equal => {
            if is_version_up(base, head) {
                DependencyGraphDeltaKindV1::DirectRequirementRaised
            } else if is_version_down(base, head) {
                DependencyGraphDeltaKindV1::DirectRequirementLowered
            } else {
                // Textually different but numerically identical floors:
                // no requirement boundary moved.
                DependencyGraphDeltaKindV1::NoSemanticGraphChange
            }
        }
    }
}
