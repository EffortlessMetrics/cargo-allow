//! Parsers for caller-supplied manifest texts and Cargo.lock texts.
//!
//! Parsing is syntax-visible only: manifest `[dependencies]`-family tables and
//! Cargo.lock `[[package]]` records. No Cargo, rustc, network, or resolution
//! is involved. All functions return deterministic results and report
//! malformed input as errors instead of panicking.

use crate::dependency_graph_delta::DependencyGraphEdgeClassV1;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use toml::Value;

/// Requirement constraint shape used for movement classification.
///
/// Complex range semantics are approximated by an operator rank plus the
/// requirement floor triple; the approximation is deterministic and the
/// original requirement text stays attached to every row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RequirementKey {
    pub operator: RequirementOperator,
    pub floor: (u64, u64, u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum RequirementOperator {
    /// `*` or an unconstrained dependency; accepts anything.
    Star,
    /// `>=` or `>` bounds.
    GreaterEqual,
    /// `^` or a bare requirement (the common case).
    Caret,
    /// `~` bounds.
    Tilde,
    /// `=` exact pins.
    Exact,
}

/// One resolved direct requirement site in one manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedRequirement {
    /// Repository-relative manifest path the site was parsed from.
    pub path: String,
    /// Normalized crate identity used for matching (`-` folded to `_`).
    pub name_key: String,
    /// Manifest-facing name as written, including Cargo aliases.
    pub display_name: String,
    /// Normalized identity of the renamed target package (`package = "..."`),
    /// equal to `name_key` when the site has no alias.
    pub package_key: String,
    pub class: DependencyGraphEdgeClassV1,
    pub target: String,
    /// Display requirement texts, sorted and deduplicated.
    pub requirements: Vec<String>,
    /// Classification keys derived from `requirements`, sorted and deduplicated.
    pub canonical: Vec<RequirementKey>,
    pub features: BTreeSet<String>,
    pub default_features: bool,
    pub optional: bool,
    pub source_spec: Option<String>,
    pub workspace_inherited: bool,
}

impl ParsedRequirement {
    /// Single canonical label, e.g. `caret_1_2_0` or `caret_1_0_0+caret_2_0_0`.
    pub(crate) fn canonical_label(&self) -> String {
        canonical_label(&self.canonical)
    }
}

pub(crate) fn canonical_label(canonical: &[RequirementKey]) -> String {
    canonical
        .iter()
        .map(|key| {
            let operator = match key.operator {
                RequirementOperator::Star => "star",
                RequirementOperator::GreaterEqual => "ge",
                RequirementOperator::Caret => "caret",
                RequirementOperator::Tilde => "tilde",
                RequirementOperator::Exact => "exact",
            };
            format!("{operator}_{}_{}_{}", key.floor.0, key.floor.1, key.floor.2)
        })
        .collect::<Vec<String>>()
        .join("+")
}

/// Raw dependency spec as written, before workspace inheritance resolution.
///
/// Accepts the string shorthand (`dep = "1.0"`), the explicit table form
/// including multiple version alternatives (`dep = { version = ["1.0", "0.2"] }`),
/// and a bare requirement array (`dep = ["1.0", "0.2"]`).
#[derive(Debug, Clone, Default)]
pub(crate) struct DependencySpecRaw {
    pub requirements: Vec<String>,
    pub features: BTreeSet<String>,
    pub default_features: Option<bool>,
    pub optional: Option<bool>,
    pub package_rename: Option<String>,
    pub source_spec: Option<String>,
    pub workspace: bool,
}

/// Workspace-inheritance table collected from `[workspace.dependencies]`.
pub(crate) type WorkspaceSpecs = BTreeMap<String, DependencySpecRaw>;

/// Cargo.lock package record with normalized lockfile edges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedLockPackage {
    pub name: String,
    pub name_key: String,
    pub version: String,
    pub source: Option<String>,
    pub checksum: Option<String>,
    /// Normalized dependency edges; edge version qualifiers are intentionally
    /// dropped so transitive version bumps do not masquerade as edge changes.
    pub edges: BTreeSet<LockEdge>,
}

/// Normalized lockfile dependency edge.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct LockEdge {
    pub name_key: String,
    pub feature: Option<String>,
    /// Edge carried the `dep? feature` optional-activation marker.
    pub optional_activation: bool,
}

/// Normalize a Cargo package name for matching purposes.
pub(crate) fn normalize_package_name(name: &str) -> String {
    name.trim().to_lowercase().replace('-', "_")
}

/// Parse one manifest text into its direct requirement sites.
///
/// `path` is bound onto every parsed requirement for row provenance.
pub(crate) fn parse_manifest(
    path: &str,
    text: &str,
    workspace_specs: &WorkspaceSpecs,
) -> Result<Vec<ParsedRequirement>, String> {
    let root: Value =
        toml::from_str(text).map_err(|err| format!("manifest_parse_error:{path}:{err}"))?;
    let mut requirements = Vec::new();
    collect_class_tables(&root, "", workspace_specs, &mut requirements);
    if let Some(targets) = root.get("target").and_then(Value::as_table) {
        for (target_name, spec) in targets {
            collect_class_tables(spec, target_name, workspace_specs, &mut requirements);
        }
    }
    for requirement in &mut requirements {
        requirement.path = path.to_string();
    }
    Ok(requirements)
}

/// Collect `[workspace.dependencies]` entries for inheritance resolution.
///
/// Entries are first-wins over sorted manifest order; callers feed manifests
/// in sorted path order so collection is deterministic.
pub(crate) fn collect_workspace_specs(
    text: &str,
    specs: &mut WorkspaceSpecs,
) -> Result<(), String> {
    let root: Value = toml::from_str(text).map_err(|err| format!("manifest_parse_error:{err}"))?;
    let Some(workspace_deps) = root
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(Value::as_table)
    else {
        return Ok(());
    };
    for (name, value) in workspace_deps {
        let name_key = normalize_package_name(name);
        specs
            .entry(name_key)
            .or_insert_with(|| parse_dependency_raw(value));
    }
    Ok(())
}

fn collect_class_tables(
    root: &Value,
    target: &str,
    workspace_specs: &WorkspaceSpecs,
    out: &mut Vec<ParsedRequirement>,
) {
    for (key, class) in [
        ("dependencies", DependencyGraphEdgeClassV1::Normal),
        ("dev-dependencies", DependencyGraphEdgeClassV1::Dev),
        ("build-dependencies", DependencyGraphEdgeClassV1::Build),
    ] {
        if let Some(table) = root.get(key).and_then(Value::as_table) {
            for (name, value) in table {
                out.push(build_requirement(
                    name,
                    value,
                    class,
                    target,
                    workspace_specs,
                ));
            }
        }
    }
}

fn build_requirement(
    name: &str,
    value: &Value,
    class: DependencyGraphEdgeClassV1,
    target: &str,
    workspace_specs: &WorkspaceSpecs,
) -> ParsedRequirement {
    let raw = parse_dependency_raw(value);
    let resolved = resolve_workspace_inheritance(&raw, name, workspace_specs);
    let name_key = normalize_package_name(name);
    let package_key = resolved
        .package_rename
        .as_ref()
        .map(|renamed| normalize_package_name(renamed))
        .unwrap_or_else(|| name_key.clone());
    let mut requirements = resolved.requirements;
    requirements.sort();
    requirements.dedup();
    let mut canonical: Vec<RequirementKey> = requirements
        .iter()
        .map(|requirement| requirement_key(requirement))
        .collect();
    if canonical.is_empty() {
        canonical.push(RequirementKey {
            operator: RequirementOperator::Star,
            floor: (0, 0, 0),
        });
    }
    canonical.sort();
    canonical.dedup();
    ParsedRequirement {
        path: String::new(),
        name_key,
        display_name: name.to_string(),
        package_key,
        class,
        target: target.to_string(),
        requirements,
        canonical,
        features: resolved.features,
        default_features: resolved.default_features.unwrap_or(true),
        optional: resolved.optional.unwrap_or(false),
        source_spec: resolved.source_spec,
        workspace_inherited: resolved.workspace,
    }
}

fn parse_dependency_raw(value: &Value) -> DependencySpecRaw {
    let mut raw = DependencySpecRaw::default();
    match value {
        Value::String(requirement) => {
            raw.requirements.push(requirement.clone());
        }
        Value::Array(alternatives) => {
            for alternative in alternatives {
                if let Some(requirement) = alternative.as_str() {
                    raw.requirements.push(requirement.to_string());
                }
            }
        }
        Value::Table(table) => {
            if table.get("workspace").and_then(Value::as_bool) == Some(true) {
                raw.workspace = true;
            }
            match table.get("version") {
                Some(Value::String(requirement)) => {
                    raw.requirements.push(requirement.clone());
                }
                Some(Value::Array(alternatives)) => {
                    for alternative in alternatives {
                        if let Some(requirement) = alternative.as_str() {
                            raw.requirements.push(requirement.to_string());
                        }
                    }
                }
                _ => {}
            }
            if let Some(features) = table.get("features").and_then(Value::as_array) {
                for feature in features {
                    if let Some(name) = feature.as_str() {
                        raw.features.insert(name.to_string());
                    }
                }
            }
            raw.default_features = table
                .get("default-features")
                .and_then(Value::as_bool)
                .or_else(|| table.get("default_features").and_then(Value::as_bool));
            raw.optional = table.get("optional").and_then(Value::as_bool);
            raw.package_rename = table
                .get("package")
                .and_then(Value::as_str)
                .map(str::to_string);
            raw.source_spec = source_spec_of(table);
        }
        _ => {}
    }
    if raw.requirements.is_empty() && !raw.workspace {
        raw.requirements.push("*".to_string());
    }
    raw
}

fn source_spec_of(table: &toml::map::Map<String, Value>) -> Option<String> {
    if let Some(git) = table.get("git").and_then(Value::as_str) {
        let mut spec = format!("git+{git}");
        if let Some(branch) = table.get("branch").and_then(Value::as_str) {
            spec.push_str(&format!("+branch={branch}"));
        }
        if let Some(tag) = table.get("tag").and_then(Value::as_str) {
            spec.push_str(&format!("+tag={tag}"));
        }
        if let Some(rev) = table.get("rev").and_then(Value::as_str) {
            spec.push_str(&format!("+rev={rev}"));
        }
        return Some(spec);
    }
    if let Some(path) = table.get("path").and_then(Value::as_str) {
        return Some(format!("path+{path}"));
    }
    table
        .get("registry")
        .and_then(Value::as_str)
        .map(|registry| format!("registry+{registry}"))
}

fn resolve_workspace_inheritance(
    raw: &DependencySpecRaw,
    name: &str,
    workspace_specs: &WorkspaceSpecs,
) -> DependencySpecRaw {
    if !raw.workspace {
        return raw.clone();
    }
    let name_key = normalize_package_name(name);
    let Some(inherited) = workspace_specs.get(&name_key) else {
        // Unresolvable inheritance degrades to an unconstrained requirement;
        // movement remains visible through the display requirement text.
        let mut degraded = raw.clone();
        degraded.workspace = false;
        if degraded.requirements.is_empty() {
            degraded.requirements.push("*".to_string());
        }
        return degraded;
    };
    DependencySpecRaw {
        requirements: inherited.requirements.clone(),
        features: raw.features.union(&inherited.features).cloned().collect(),
        default_features: raw.default_features.or(inherited.default_features),
        optional: raw.optional,
        package_rename: inherited.package_rename.clone(),
        source_spec: inherited.source_spec.clone(),
        workspace: true,
    }
}

/// Classification key for one requirement text.
pub(crate) fn requirement_key(requirement: &str) -> RequirementKey {
    let trimmed = requirement.trim();
    let body = trimmed.trim_start_matches('=');
    let (operator, version_body) = if let Some(rest) = trimmed.strip_prefix(">=") {
        (RequirementOperator::GreaterEqual, rest)
    } else if trimmed.starts_with('>') {
        (RequirementOperator::GreaterEqual, body)
    } else if trimmed.starts_with('<') {
        // Upper-bound-only constraints have no floor; they stay visible
        // through their display text and compare as unconstrained floors.
        (RequirementOperator::Star, trimmed)
    } else if let Some(rest) = trimmed.strip_prefix('~') {
        (RequirementOperator::Tilde, rest)
    } else if trimmed.starts_with('=') {
        (RequirementOperator::Exact, body)
    } else if let Some(rest) = trimmed.strip_prefix('^') {
        (RequirementOperator::Caret, rest)
    } else {
        (RequirementOperator::Caret, body)
    };
    if version_body.trim() == "*" || version_body.trim().is_empty() {
        return RequirementKey {
            operator: RequirementOperator::Star,
            floor: (0, 0, 0),
        };
    }
    // Conjunction segments (">=0.8, <1.0") keep their first bound as the
    // floor; the full text stays attached to rows for exact review.
    let first_bound = version_body.split(',').next().unwrap_or(version_body);
    RequirementKey {
        operator,
        floor: version_floor(first_bound),
    }
}

fn version_floor(text: &str) -> (u64, u64, u64) {
    let core = text.trim().split(['-', '+']).next().unwrap_or(text);
    let mut parts = core.trim().split('.');
    let mut floor = (0u64, 0u64, 0u64);
    let Some(major) = parts
        .next()
        .and_then(|part| part.trim().parse::<u64>().ok())
    else {
        return floor;
    };
    floor.0 = major;
    let Some(minor) = parts
        .next()
        .and_then(|part| part.trim().parse::<u64>().ok())
    else {
        return floor;
    };
    floor.1 = minor;
    if let Some(patch) = parts
        .next()
        .and_then(|part| part.trim().parse::<u64>().ok())
    {
        floor.2 = patch;
    }
    floor
}

/// Full semver triple for lockfile versions. Lockfile versions are always
/// complete triples; anything else is treated as malformed input.
pub(crate) fn parse_semver_triple(version: &str) -> Option<(u64, u64, u64)> {
    let core = version.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.trim().parse::<u64>().ok()?;
    let minor = parts.next()?.trim().parse::<u64>().ok()?;
    let patch = parts.next()?.trim().parse::<u64>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

pub(crate) fn version_has_prerelease(version: &str) -> bool {
    let core_end = version.split('+').next().unwrap_or(version);
    core_end.split('-').nth(1).is_some()
}

/// Deterministic lockfile version ordering with prerelease awareness.
pub(crate) fn compare_lock_versions(left: &str, right: &str) -> std::cmp::Ordering {
    match (parse_semver_triple(left), parse_semver_triple(right)) {
        (Some(left_triple), Some(right_triple)) => left_triple.cmp(&right_triple).then_with(|| {
            match (version_has_prerelease(left), version_has_prerelease(right)) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => left.cmp(right),
            }
        }),
        _ => left.cmp(right),
    }
}

/// Simplified requirement satisfaction used for manifest/lock agreement.
///
/// Full range semantics arrive with bounded Cargo metadata; the check is
/// deliberately conservative about caret, tilde, exact, and floor bounds so
/// stale-lockfile shapes (manifest moved, lock untouched) fail visibly.
pub(crate) fn requirement_satisfied(key: &RequirementKey, version: &str) -> bool {
    let Some(triple) = parse_semver_triple(version) else {
        return false;
    };
    let at_least = triple >= key.floor;
    match key.operator {
        RequirementOperator::Star => true,
        RequirementOperator::GreaterEqual => at_least,
        RequirementOperator::Caret => {
            let (major, minor, _patch) = key.floor;
            if major >= 1 {
                triple.0 == major && at_least
            } else if minor >= 1 {
                triple.0 == 0 && triple.1 == minor && at_least
            } else {
                triple.0 == 0 && triple.1 == 0 && at_least
            }
        }
        RequirementOperator::Tilde => triple.0 == key.floor.0 && at_least,
        RequirementOperator::Exact => triple == key.floor,
    }
}

#[derive(Deserialize)]
struct LockfileDocumentRaw {
    #[serde(default)]
    package: Vec<LockfilePackageRaw>,
}

#[derive(Deserialize)]
struct LockfilePackageRaw {
    name: String,
    version: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    checksum: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
}

/// Parse one Cargo.lock text into sorted package records.
pub(crate) fn parse_lockfile(text: &str) -> Result<Vec<ParsedLockPackage>, String> {
    let document: LockfileDocumentRaw =
        toml::from_str(text).map_err(|err| format!("lockfile_parse_error:{err}"))?;
    let mut packages = Vec::new();
    for package in document.package {
        if parse_semver_triple(&package.version).is_none() {
            return Err(format!(
                "lockfile_version_unparseable:{}:{}",
                package.name, package.version
            ));
        }
        let mut edges = BTreeSet::new();
        for raw_edge in &package.dependencies {
            if let Some(edge) = parse_lock_edge(raw_edge) {
                edges.insert(edge);
            }
        }
        packages.push(ParsedLockPackage {
            name_key: normalize_package_name(&package.name),
            name: package.name,
            version: package.version,
            source: package.source,
            checksum: package.checksum,
            edges,
        });
    }
    packages.sort_by(|left, right| {
        left.name_key
            .cmp(&right.name_key)
            .then_with(|| compare_lock_versions(&left.version, &right.version))
            .then_with(|| left.source.cmp(&right.source))
    });
    Ok(packages)
}

fn parse_lock_edge(raw: &str) -> Option<LockEdge> {
    let mut segments = raw.split_whitespace();
    let head = segments.next()?;
    let (name, optional_activation) = match head.strip_suffix('?') {
        Some(stripped) => (stripped, true),
        None => (head, false),
    };
    let mut feature = None;
    for segment in segments {
        if parse_semver_triple(segment).is_some() {
            continue;
        }
        feature = Some(segment.to_string());
    }
    Some(LockEdge {
        name_key: normalize_package_name(name),
        feature,
        optional_activation,
    })
}
