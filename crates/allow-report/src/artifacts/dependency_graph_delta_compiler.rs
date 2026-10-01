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
            // Cargo unions member-local features with the inherited
            // spec's features; the same union is what the delta must
            // see or a member-only feature change disappears.
            let member_features = match spec {
                toml::Value::String(_) => Vec::new(),
                table => spec_features(table),
            };
            let mut features = match ws_deps.and_then(|table| table.get(name.as_str())) {
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
            };
            features.1.extend(member_features);
            features.1.sort();
            features.1.dedup();
            features
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

/// Validate one manifest document strictly for the producer
/// boundary: parse errors are malformed input, not an empty set.
pub fn validate_manifest_document(text: &str) -> Result<(), String> {
    toml::from_str::<toml::Value>(text)
        .map(|_| ())
        .map_err(|error| format!("manifest document does not parse: {error}"))
}

/// Validate one lockfile document strictly for the producer boundary:
/// a lock parses as TOML and carries at least one `[[package]]` entry.
pub fn validate_lock_document(text: &str) -> Result<(), String> {
    let value: toml::Value =
        toml::from_str(text).map_err(|error| format!("lock document does not parse: {error}"))?;
    let empty = value
        .get("package")
        .and_then(toml::Value::as_array)
        .map(|entries| entries.is_empty())
        .unwrap_or(true);
    if empty {
        return Err("lock document carries no [[package]] entries".to_string());
    }
    Ok(())
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

    // Parse both lockfiles for resolved package movement. Rows are
    // grouped per name and keyed by full Cargo identity (version,
    // source) so that legitimate duplicate-version lock entries stay
    // distinct (#4243): a name-keyed map silently kept only the last
    // occurrence, hiding movement of the shadowed version.
    let base_packages = parse_lock_packages(base_lock);
    let head_packages = parse_lock_packages(head_lock);
    let mut base_by_name: std::collections::BTreeMap<&str, Vec<&LockPackage>> =
        std::collections::BTreeMap::new();
    for package in &base_packages {
        base_by_name
            .entry(package.name.as_str())
            .or_default()
            .push(package);
    }
    let mut head_by_name: std::collections::BTreeMap<&str, Vec<&LockPackage>> =
        std::collections::BTreeMap::new();
    for package in &head_packages {
        head_by_name
            .entry(package.name.as_str())
            .or_default()
            .push(package);
    }

    let all_lock_names: std::collections::BTreeSet<&str> = base_by_name
        .keys()
        .chain(head_by_name.keys())
        .copied()
        .collect();

    for name in &all_lock_names {
        let empty: Vec<&LockPackage> = Vec::new();
        let base_rows = base_by_name.get(name).unwrap_or(&empty);
        let head_rows = head_by_name.get(name).unwrap_or(&empty);

        let requirement_unchanged = match (base_req_map.get(*name), head_req_map.get(*name)) {
            (Some(base), Some(head)) => base == head,
            (None, None) => true,
            _ => false,
        };

        // Exact identity matches (same version and source): only a
        // checksum change is reportable, on that exact identity pair.
        // The remaining identities pair deterministically — both sides
        // sorted by version precedence — so a moved duplicate version
        // surfaces as the upgrade/downgrade row for its exact identity
        // pair, and a count change surfaces as DuplicateVersionMovement
        // with PackageAdded/PackageRemoved rows for the surplus
        // identities. Movement is never silent and never misattributed
        // to the unshadowed pair (#4243).
        let mut unmatched_base: Vec<&LockPackage> = base_rows.to_vec();
        let mut unmatched_head: Vec<&LockPackage> = head_rows.to_vec();
        for base in base_rows {
            let Some(position) = unmatched_head
                .iter()
                .position(|head| head.version == base.version && head.source == base.source)
            else {
                continue;
            };
            let head = unmatched_head.remove(position);
            unmatched_base.retain(|candidate| {
                candidate.version != base.version || candidate.source != base.source
            });
            let checksum_changed = !base.checksum.is_empty()
                && !head.checksum.is_empty()
                && base.checksum != head.checksum;
            if checksum_changed {
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

        unmatched_base
            .sort_by(|a, b| compare_versions(&a.version, &b.version).then(a.source.cmp(&b.source)));
        unmatched_head
            .sort_by(|a, b| compare_versions(&a.version, &b.version).then(a.source.cmp(&b.source)));
        let pair_count = unmatched_base.len().min(unmatched_head.len());
        for (base, head) in unmatched_base.iter().zip(unmatched_head.iter()) {
            let version_up = is_version_up(&base.version, &head.version);
            let version_down = is_version_down(&base.version, &head.version);
            if version_up || version_down {
                // Version movement is its own row (an upgrade or a
                // downgrade, never a "compatible update"), and when the
                // manifest requirement did not move, an additional row
                // marks the movement as lock-only resolution. A version
                // bump legitimately rotates the checksum, so it is not
                // classified as a source/checksum identity change.
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
            } else if base.source != head.source || base.checksum != head.checksum {
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
        for surplus_base in unmatched_base.iter().skip(pair_count) {
            rows.push(DependencyGraphDeltaRowV1 {
                kind: DependencyGraphDeltaKindV1::DuplicateVersionMovement,
                class: DependencyClassV1::Normal,
                package_name: name.to_string(),
                base_version: surplus_base.version.clone(),
                head_version: String::new(),
                base_requirement: String::new(),
                head_requirement: String::new(),
                base_source: surplus_base.source.clone(),
                head_source: String::new(),
                base_checksum: surplus_base.checksum.clone(),
                head_checksum: String::new(),
            });
            rows.push(DependencyGraphDeltaRowV1 {
                kind: DependencyGraphDeltaKindV1::PackageRemoved,
                class: DependencyClassV1::Normal,
                package_name: name.to_string(),
                base_version: surplus_base.version.clone(),
                head_version: String::new(),
                base_requirement: String::new(),
                head_requirement: String::new(),
                base_source: surplus_base.source.clone(),
                head_source: String::new(),
                base_checksum: surplus_base.checksum.clone(),
                head_checksum: String::new(),
            });
        }
        for surplus_head in unmatched_head.iter().skip(pair_count) {
            rows.push(DependencyGraphDeltaRowV1 {
                kind: DependencyGraphDeltaKindV1::DuplicateVersionMovement,
                class: DependencyClassV1::Normal,
                package_name: name.to_string(),
                base_version: String::new(),
                head_version: surplus_head.version.clone(),
                base_requirement: String::new(),
                head_requirement: String::new(),
                base_source: String::new(),
                head_source: surplus_head.source.clone(),
                base_checksum: String::new(),
                head_checksum: surplus_head.checksum.clone(),
            });
            rows.push(DependencyGraphDeltaRowV1 {
                kind: DependencyGraphDeltaKindV1::PackageAdded,
                class: DependencyClassV1::Normal,
                package_name: name.to_string(),
                base_version: String::new(),
                head_version: surplus_head.version.clone(),
                base_requirement: String::new(),
                head_requirement: String::new(),
                base_source: String::new(),
                head_source: surplus_head.source.clone(),
                base_checksum: String::new(),
                head_checksum: surplus_head.checksum.clone(),
            });
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
            "duplicate-version rows are distinguished by (version, source) identity, but the dependency edge that pulled a specific resolved row is not expressible in the row model; edge provenance remains a separate member-provenance follow-up (#4244)".to_string(),
        ],
        claim_boundary: "Typed dependency graph delta for one exact base/head pair: every row preserves the native Cargo identity (package name, version, source, checksum) and the delta kind. Count parity does not establish graph identity. A downgrade is never described as a compatible update. Missing, malformed, or partial analyses are instrument failures, not NoSemanticGraphChange.".to_string(),
    })
}

/// Compare two semver-like version strings for ordering.
///
/// Cargo.lock versions are usually plain `x.y.z`, but prerelease
/// resolutions (`1.0.0-rc.1`, `-alpha`, build metadata after `+`) are
/// real lockfile content (#4245). Comparison follows semver
/// precedence: build metadata is ignored, the numeric dot segments of
/// the core version compare first, a version with a prerelease
/// identifier is less than the same core version without one, and two
/// prerelease identifier lists compare identifier by identifier —
/// numeric identifiers numerically, numeric below alphanumeric, the
/// rest ASCII-lexically — with the longer list greater when the shared
/// prefix is equal. A core segment that is not a plain integer cannot
/// be ordered and is ignored for the numeric comparison, preserving
/// the previous lenient behavior for the resolved-version surface
/// (requirements, unlike resolved versions, fail closed in
/// `classify_requirement_movement`).
pub(crate) fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    fn parse(v: &str) -> (Vec<u64>, Option<&str>) {
        let no_build = v.split('+').next().unwrap_or(v);
        match no_build.split_once('-') {
            Some((core, pre)) => (
                core.split('.')
                    .filter_map(|part| part.parse::<u64>().ok())
                    .collect(),
                Some(pre),
            ),
            None => (
                no_build
                    .split('.')
                    .filter_map(|part| part.parse::<u64>().ok())
                    .collect(),
                None,
            ),
        }
    }
    let (a_parts, a_pre) = parse(a);
    let (b_parts, b_pre) = parse(b);
    for i in 0..a_parts.len().max(b_parts.len()) {
        let a_val = a_parts.get(i).copied().unwrap_or(0);
        let b_val = b_parts.get(i).copied().unwrap_or(0);
        match a_val.cmp(&b_val) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    match (a_pre, b_pre) {
        (None, None) => std::cmp::Ordering::Equal,
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(a_pre), Some(b_pre)) => compare_prerelease_identifiers(a_pre, b_pre),
    }
}

/// Compare two non-empty prerelease identifier lists (`rc.1`, `alpha`)
/// per semver precedence: numeric identifiers compare numerically and
/// rank below alphanumeric identifiers, alphanumeric identifiers
/// compare ASCII-lexically, and the longer list wins when the shared
/// prefix is equal.
fn compare_prerelease_identifiers(a: &str, b: &str) -> std::cmp::Ordering {
    let a_ids: Vec<&str> = a.split('.').collect();
    let b_ids: Vec<&str> = b.split('.').collect();
    for (&a_id, &b_id) in a_ids.iter().zip(b_ids.iter()) {
        let ordering = match (a_id.parse::<u64>().ok(), b_id.parse::<u64>().ok()) {
            (Some(a_num), Some(b_num)) => a_num.cmp(&b_num),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a_id.cmp(b_id),
        };
        if ordering != std::cmp::Ordering::Equal {
            return ordering;
        }
    }
    a_ids.len().cmp(&b_ids.len())
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
