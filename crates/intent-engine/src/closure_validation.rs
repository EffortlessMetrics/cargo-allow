//! Exact Cargo metadata closure validation over supplied bounded input
//! (#2942 step 3 / #3329).
//!
//! Consumes a supplied `cargo metadata --format-version 1` JSON artifact
//! (never spawned) plus the V2 governance identities and dependency law,
//! and emits exact observed/target closure diagnostics: forbidden and
//! required edges at the logical-identity level, shortest actionable
//! dependency paths, identity conflicts, and observed/target denominator
//! reconciliation.
//!
//! Boundary: parses supplied text only. No Cargo/rustc/Clippy/build-script/
//! proc-macro invocation, no filesystem writes, no ambient workspace reads.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use intent_model::{
    GovernanceCrateIdentityV2, GovernanceForbiddenEdgeV2, GovernanceRequiredEdgeV2,
};

// ---------------------------------------------------------------------------
// Metadata graph (supplied artifact)
// ---------------------------------------------------------------------------

/// Fine-grained dependency class of an observed edge (#2922 vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObservedDependencyClassV2 {
    Normal,
    Dev,
    Build,
    TargetSpecific,
    Optional,
}

impl ObservedDependencyClassV2 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Dev => "dev",
            Self::Build => "build",
            Self::TargetSpecific => "target_specific",
            Self::Optional => "optional",
        }
    }

    fn from_kind_and_flags(kind: Option<&str>, optional: bool, target: Option<&str>) -> Self {
        if optional {
            return Self::Optional;
        }
        if target.is_some() {
            return Self::TargetSpecific;
        }
        match kind {
            Some("dev") => Self::Dev,
            Some("build") => Self::Build,
            _ => Self::Normal,
        }
    }
}

/// An observed dependency edge in the supplied metadata artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedDependencyEdgeV2 {
    pub from_package: String,
    pub to_package: String,
    pub class: ObservedDependencyClassV2,
}

/// The observed dependency graph parsed from a supplied artifact.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObservedMetadataGraphV2 {
    /// Every package declared in the artifact, including isolated ones.
    pub packages: Vec<String>,
    pub edges: Vec<ObservedDependencyEdgeV2>,
}

/// Parse a supplied `cargo metadata --format-version 1` JSON artifact into
/// the observed graph. Does not invoke Cargo.
pub fn parse_observed_metadata_graph_v2(input: &str) -> Result<ObservedMetadataGraphV2, String> {
    #[derive(serde::Deserialize)]
    struct MetadataJson {
        #[serde(default)]
        packages: Vec<MetadataPackage>,
    }
    #[derive(serde::Deserialize)]
    struct MetadataPackage {
        name: String,
        #[serde(default)]
        dependencies: Vec<MetadataDependency>,
    }
    #[derive(serde::Deserialize)]
    struct MetadataDependency {
        name: String,
        #[serde(default)]
        kind: Option<String>,
        #[serde(default)]
        optional: bool,
        #[serde(default)]
        target: Option<String>,
    }
    let parsed: MetadataJson = serde_json::from_str(input)
        .map_err(|err| format!("failed to parse cargo metadata JSON: {err}"))?;
    let mut packages = Vec::with_capacity(parsed.packages.len());
    let mut edges = Vec::new();
    for package in &parsed.packages {
        packages.push(package.name.clone());
        for dependency in &package.dependencies {
            let class = ObservedDependencyClassV2::from_kind_and_flags(
                dependency.kind.as_deref(),
                dependency.optional,
                dependency.target.as_deref(),
            );
            edges.push(ObservedDependencyEdgeV2 {
                from_package: package.name.clone(),
                to_package: dependency.name.clone(),
                class,
            });
        }
    }
    Ok(ObservedMetadataGraphV2 { packages, edges })
}

/// Compute the shortest dependency path between two packages using BFS.
///
/// Deterministic regardless of input traversal order: adjacency and visited
/// sets are ordered, and neighbors are visited in sorted order.
pub fn shortest_observed_path(
    graph: &ObservedMetadataGraphV2,
    from: &str,
    to: &str,
) -> Option<Vec<String>> {
    if from == to {
        return Some(vec![from.to_string()]);
    }
    let mut adjacency: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for edge in &graph.edges {
        adjacency
            .entry(edge.from_package.as_str())
            .or_default()
            .insert(edge.to_package.as_str());
    }
    let mut queue = VecDeque::from([(from, vec![from.to_string()])]);
    let mut visited: BTreeSet<&str> = BTreeSet::new();
    visited.insert(from);
    while let Some((current, path)) = queue.pop_front() {
        let Some(neighbors) = adjacency.get(current) else {
            continue;
        };
        for next in neighbors {
            if visited.contains(next) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push((*next).to_string());
            if *next == to {
                return Some(next_path);
            }
            visited.insert(next);
            queue.push_back((next, next_path));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Closure validation
// ---------------------------------------------------------------------------

/// Explicit bounded closure-validation input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureValidationInputV2<'a> {
    /// Supplied observed metadata graph (already parsed; never spawned).
    pub observed: &'a ObservedMetadataGraphV2,
    /// V2 governance crate identities (target topology).
    pub identities: &'a [GovernanceCrateIdentityV2],
    /// V2 authored dependency law (forbidden edges).
    pub forbidden_edges: &'a [GovernanceForbiddenEdgeV2],
    /// V2 authored dependency law (required edges).
    pub required_edges: &'a [GovernanceRequiredEdgeV2],
}

/// Kind of closure diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClosureFindingKindV2 {
    UnclassifiedPackage,
    IdentityConflict,
    ForbiddenDependency,
    MissingRequiredDependency,
    ObservedPackageWithoutIdentity,
    IdentityWithoutObservedPackage,
}

impl ClosureFindingKindV2 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnclassifiedPackage => "unclassified_package",
            Self::IdentityConflict => "identity_conflict",
            Self::ForbiddenDependency => "forbidden_dependency",
            Self::MissingRequiredDependency => "missing_required_dependency",
            Self::ObservedPackageWithoutIdentity => "observed_package_without_identity",
            Self::IdentityWithoutObservedPackage => "identity_without_observed_package",
        }
    }
}

/// One closure diagnostic, mirroring #2922's shape: kind, message, package
/// names, and the shortest actionable dependency path when one exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureFindingV2 {
    pub kind: ClosureFindingKindV2,
    pub message: String,
    pub package_names: Vec<String>,
    pub dependency_path: Vec<String>,
}

/// Full closure validation report.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClosureValidationReportV2 {
    pub findings: Vec<ClosureFindingV2>,
}

impl ClosureValidationReportV2 {
    pub fn has_blocking(&self) -> bool {
        !self.findings.is_empty()
    }
}

/// Validate the observed closure against the target V2 authority.
pub fn validate_observed_closure(
    input: &ClosureValidationInputV2<'_>,
) -> ClosureValidationReportV2 {
    let package_to_logical = identity_map(input.identities);
    let mut findings = Vec::new();
    findings.extend(validate_identity_uniqueness(input));
    findings.extend(reconcile_denominator(input, &package_to_logical));
    findings.extend(detect_forbidden_edges(input, &package_to_logical));
    findings.extend(detect_required_edges(input, &package_to_logical));
    ClosureValidationReportV2 { findings }
}

type PackageToLogical = BTreeMap<String, String>;

/// Map cargo package names (and their workspace aliases) to logical IDs.
fn identity_map(identities: &[GovernanceCrateIdentityV2]) -> PackageToLogical {
    let mut map = BTreeMap::new();
    for identity in identities {
        map.insert(
            identity.cargo_package_name.clone(),
            identity.logical_id.clone(),
        );
        for alias in &identity.workspace_dependency_aliases {
            map.entry(alias.clone())
                .or_insert_with(|| identity.logical_id.clone());
        }
    }
    map
}

/// Every identity logical id must be unique; duplicate logical ids are
/// identity conflicts.
fn validate_identity_uniqueness(input: &ClosureValidationInputV2<'_>) -> Vec<ClosureFindingV2> {
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for identity in input.identities {
        *seen.entry(identity.logical_id.as_str()).or_default() += 1;
    }
    seen.iter()
        .filter(|(_, count)| **count > 1)
        .map(|(logical_id, count)| ClosureFindingV2 {
            kind: ClosureFindingKindV2::IdentityConflict,
            message: format!("logical id `{logical_id}` declared {count} times"),
            package_names: Vec::new(),
            dependency_path: Vec::new(),
        })
        .collect()
}

/// Denominator reconciliation between observed packages and target
/// identities, in both directions.
fn reconcile_denominator(
    input: &ClosureValidationInputV2<'_>,
    package_to_logical: &PackageToLogical,
) -> Vec<ClosureFindingV2> {
    let mut findings = Vec::new();
    let mut observed_packages = BTreeSet::new();
    for package in &input.observed.packages {
        observed_packages.insert(package.as_str());
    }
    for edge in &input.observed.edges {
        observed_packages.insert(edge.from_package.as_str());
        observed_packages.insert(edge.to_package.as_str());
    }
    for package in &observed_packages {
        if !package_to_logical.contains_key(*package) {
            findings.push(ClosureFindingV2 {
                kind: ClosureFindingKindV2::UnclassifiedPackage,
                message: format!("observed package `{package}` has no V2 identity"),
                package_names: vec![(*package).to_string()],
                dependency_path: Vec::new(),
            });
        }
    }
    for identity in input.identities {
        let observed = observed_packages.contains(identity.cargo_package_name.as_str())
            || identity
                .workspace_dependency_aliases
                .iter()
                .any(|alias| observed_packages.contains(alias.as_str()));
        if !observed {
            findings.push(ClosureFindingV2 {
                kind: ClosureFindingKindV2::IdentityWithoutObservedPackage,
                message: format!(
                    "identity `{}` ({}) is absent from the observed closure",
                    identity.logical_id, identity.cargo_package_name
                ),
                package_names: vec![identity.cargo_package_name.clone()],
                dependency_path: Vec::new(),
            });
        }
    }
    findings
}

/// Detect observed edges whose resolved logical endpoints are forbidden by
/// the V2 authority, reporting the shortest actionable route.
fn detect_forbidden_edges(
    input: &ClosureValidationInputV2<'_>,
    package_to_logical: &PackageToLogical,
) -> Vec<ClosureFindingV2> {
    let mut findings = Vec::new();
    let mut reported: BTreeSet<(String, String)> = BTreeSet::new();
    for edge in &input.observed.edges {
        let Some(from_logical) = package_to_logical.get(&edge.from_package) else {
            continue;
        };
        let Some(to_logical) = package_to_logical.get(&edge.to_package) else {
            continue;
        };
        let forbidden = input
            .forbidden_edges
            .iter()
            .any(|law| &law.from_logical_id == from_logical && &law.to_logical_id == to_logical);
        if forbidden && reported.insert((from_logical.clone(), to_logical.clone())) {
            let path = shortest_observed_path(input.observed, &edge.from_package, &edge.to_package)
                .unwrap_or_else(|| vec![edge.from_package.clone(), edge.to_package.clone()]);
            findings.push(ClosureFindingV2 {
                kind: ClosureFindingKindV2::ForbiddenDependency,
                message: format!(
                    "forbidden dependency `{from_logical}` -> `{to_logical}` observed as `{}` -> `{}`",
                    edge.from_package, edge.to_package
                ),
                package_names: vec![edge.from_package.clone(), edge.to_package.clone()],
                dependency_path: path,
            });
        }
    }
    findings
}

/// Detect required logical edges missing from the observed closure.
fn detect_required_edges(
    input: &ClosureValidationInputV2<'_>,
    package_to_logical: &PackageToLogical,
) -> Vec<ClosureFindingV2> {
    let mut findings = Vec::new();
    let mut observed_logical_edges: BTreeSet<(&str, &str)> = BTreeSet::new();
    for edge in &input.observed.edges {
        if let (Some(from), Some(to)) = (
            package_to_logical.get(&edge.from_package),
            package_to_logical.get(&edge.to_package),
        ) {
            observed_logical_edges.insert((from.as_str(), to.as_str()));
        }
    }
    for law in input.required_edges {
        let present = observed_logical_edges
            .iter()
            .any(|(from, to)| *from == law.from_logical_id && *to == law.to_logical_id);
        if !present {
            findings.push(ClosureFindingV2 {
                kind: ClosureFindingKindV2::MissingRequiredDependency,
                message: format!(
                    "required dependency `{}` -> `{}` is not observed",
                    law.from_logical_id, law.to_logical_id
                ),
                package_names: vec![law.from_logical_id.clone(), law.to_logical_id.clone()],
                dependency_path: Vec::new(),
            });
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use intent_model::{GovernanceCrateRoleV2, GovernanceOwnerV2};

    fn identity(logical_id: &str, package: &str) -> GovernanceCrateIdentityV2 {
        GovernanceCrateIdentityV2 {
            logical_id: logical_id.to_string(),
            workspace_path: format!("crates/{logical_id}"),
            workspace_dependency_aliases: vec![package.to_string()],
            cargo_package_name: package.to_string(),
            rust_library_name: logical_id.replace('-', "_"),
            owner: GovernanceOwnerV2::CargoProof,
            role: GovernanceCrateRoleV2::CargoProof,
        }
    }

    fn metadata(pairs: &[(&str, &[&str])]) -> String {
        let packages = pairs
            .iter()
            .map(|(name, deps)| {
                let deps = deps
                    .iter()
                    .map(|d| format!("{{\"name\": \"{d}\"}}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{\"name\": \"{name}\", \"dependencies\": [{deps}]}}")
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("{{\"packages\": [{packages}]}}")
    }

    fn forbidden(from: &str, to: &str) -> GovernanceForbiddenEdgeV2 {
        GovernanceForbiddenEdgeV2 {
            from_logical_id: from.to_string(),
            to_logical_id: to.to_string(),
            repair_hint: Some("intent-protocol".to_string()),
        }
    }

    fn required(from: &str, to: &str) -> GovernanceRequiredEdgeV2 {
        GovernanceRequiredEdgeV2 {
            from_logical_id: from.to_string(),
            to_logical_id: to.to_string(),
            rationale_issue: Some(2936),
        }
    }

    #[test]
    fn forbidden_edge_is_caught_with_shortest_actionable_path() -> Result<(), String> {
        let graph = parse_observed_metadata_graph_v2(&metadata(&[
            (
                "proof-orchestrator",
                &["intent-compiler", "intent-protocol"],
            ),
            ("intent-compiler", &[]),
            ("intent-protocol", &[]),
        ]))?;
        let identities = vec![
            identity("proof-engine", "proof-orchestrator"),
            identity("intent-engine", "intent-compiler"),
            identity("intent-protocol", "intent-protocol"),
        ];
        let law = vec![forbidden("proof-engine", "intent-engine")];
        let input = ClosureValidationInputV2 {
            observed: &graph,
            identities: &identities,
            forbidden_edges: &law,
            required_edges: &[],
        };
        let report = validate_observed_closure(&input);
        let finding = report
            .findings
            .iter()
            .find(|f| f.kind == ClosureFindingKindV2::ForbiddenDependency)
            .ok_or("forbidden edge must be caught")?;
        if finding.dependency_path
            != vec![
                "proof-orchestrator".to_string(),
                "intent-compiler".to_string(),
            ]
        {
            return Err(format!(
                "shortest actionable path must be attached: {:?}",
                finding.dependency_path
            ));
        }
        if report
            .findings
            .iter()
            .filter(|f| f.kind == ClosureFindingKindV2::ForbiddenDependency)
            .count()
            != 1
        {
            return Err("each forbidden logical edge reports exactly once".into());
        }
        Ok(())
    }

    #[test]
    fn transitive_forbidden_path_reports_shortest_route() -> Result<(), String> {
        // a -> b -> forbidden-target, plus a longer route a -> c -> d -> b;
        // the shortest actionable route from a to the target is direct.
        let graph = parse_observed_metadata_graph_v2(&metadata(&[
            ("pkg-a", &["pkg-b", "pkg-c"]),
            ("pkg-b", &["pkg-target"]),
            ("pkg-c", &["pkg-d"]),
            ("pkg-d", &["pkg-b"]),
            ("pkg-target", &[]),
        ]))?;
        let identities = vec![
            identity("logical-a", "pkg-a"),
            identity("logical-b", "pkg-b"),
            identity("logical-c", "pkg-c"),
            identity("logical-d", "pkg-d"),
            identity("logical-target", "pkg-target"),
        ];
        let law = vec![forbidden("logical-b", "logical-target")];
        let input = ClosureValidationInputV2 {
            observed: &graph,
            identities: &identities,
            forbidden_edges: &law,
            required_edges: &[],
        };
        let report = validate_observed_closure(&input);
        let finding = report
            .findings
            .iter()
            .find(|f| f.kind == ClosureFindingKindV2::ForbiddenDependency)
            .ok_or("transitive forbidden edge must be caught")?;
        if finding.dependency_path != vec!["pkg-b".to_string(), "pkg-target".to_string()] {
            return Err(format!(
                "shortest route must be pkg-b -> pkg-target: {:?}",
                finding.dependency_path
            ));
        }
        Ok(())
    }

    #[test]
    fn shortest_path_is_deterministic_on_diamond_graph() -> Result<(), String> {
        let graph = parse_observed_metadata_graph_v2(&metadata(&[
            ("root", &["left", "right"]),
            ("left", &["sink"]),
            ("right", &["sink"]),
            ("sink", &[]),
        ]))?;
        let path = shortest_observed_path(&graph, "root", "sink")
            .ok_or("path must exist through the diamond")?;
        if path != vec!["root".to_string(), "left".to_string(), "sink".to_string()] {
            return Err(format!("BFS must pick the sorted-first route: {path:?}"));
        }
        if shortest_observed_path(&graph, "sink", "root").is_some() {
            return Err("no reverse path exists in a DAG".into());
        }
        Ok(())
    }

    #[test]
    fn unclassified_observed_package_is_flagged() -> Result<(), String> {
        let graph = parse_observed_metadata_graph_v2(&metadata(&[
            ("known", &["stranger"]),
            ("stranger", &[]),
        ]))?;
        let identities = vec![identity("known", "known")];
        let input = ClosureValidationInputV2 {
            observed: &graph,
            identities: &identities,
            forbidden_edges: &[],
            required_edges: &[],
        };
        let report = validate_observed_closure(&input);
        if !report.findings.iter().any(|f| {
            f.kind == ClosureFindingKindV2::UnclassifiedPackage && f.message.contains("stranger")
        }) {
            return Err(format!(
                "stranger must be unclassified: {:?}",
                report.findings
            ));
        }
        Ok(())
    }

    #[test]
    fn duplicate_logical_id_is_identity_conflict() -> Result<(), String> {
        let graph = parse_observed_metadata_graph_v2(&metadata(&[("pkg", &[])]))?;
        let identities = vec![identity("dup", "pkg"), identity("dup", "pkg-2")];
        let input = ClosureValidationInputV2 {
            observed: &graph,
            identities: &identities,
            forbidden_edges: &[],
            required_edges: &[],
        };
        let report = validate_observed_closure(&input);
        if !report
            .findings
            .iter()
            .any(|f| f.kind == ClosureFindingKindV2::IdentityConflict)
        {
            return Err("duplicate logical id must conflict".into());
        }
        Ok(())
    }

    #[test]
    fn denominator_reconciles_both_directions() -> Result<(), String> {
        let graph = parse_observed_metadata_graph_v2(&metadata(&[
            ("observed-only", &[]),
            ("proof-orchestrator", &[]),
        ]))?;
        let identities = vec![
            identity("proof-engine", "proof-orchestrator"),
            identity("ghost", "ghost-pkg"),
        ];
        let input = ClosureValidationInputV2 {
            observed: &graph,
            identities: &identities,
            forbidden_edges: &[],
            required_edges: &[],
        };
        let report = validate_observed_closure(&input);
        if !report.findings.iter().any(|f| {
            f.kind == ClosureFindingKindV2::UnclassifiedPackage
                && f.message.contains("observed-only")
        }) {
            return Err("observed package without identity must be flagged".into());
        }
        if !report.findings.iter().any(|f| {
            f.kind == ClosureFindingKindV2::IdentityWithoutObservedPackage
                && f.message.contains("ghost")
        }) {
            return Err("identity without observed package must be flagged".into());
        }
        Ok(())
    }

    #[test]
    fn required_edge_missing_is_flagged_and_present_passes() -> Result<(), String> {
        let identities = vec![
            identity("proof-engine", "proof-orchestrator"),
            identity("intent-protocol", "intent-protocol"),
        ];
        let law_required = vec![required("proof-engine", "intent-protocol")];

        let missing_graph = parse_observed_metadata_graph_v2(&metadata(&[
            ("proof-orchestrator", &[]),
            ("intent-protocol", &[]),
        ]))?;
        let missing_input = ClosureValidationInputV2 {
            observed: &missing_graph,
            identities: &identities,
            forbidden_edges: &[],
            required_edges: &law_required,
        };
        let report = validate_observed_closure(&missing_input);
        if !report
            .findings
            .iter()
            .any(|f| f.kind == ClosureFindingKindV2::MissingRequiredDependency)
        {
            return Err("missing required edge must be flagged".into());
        }

        let present_graph = parse_observed_metadata_graph_v2(&metadata(&[
            ("proof-orchestrator", &["intent-protocol"]),
            ("intent-protocol", &[]),
        ]))?;
        let present_input = ClosureValidationInputV2 {
            observed: &present_graph,
            identities: &identities,
            forbidden_edges: &[],
            required_edges: &law_required,
        };
        let report = validate_observed_closure(&present_input);
        if report
            .findings
            .iter()
            .any(|f| f.kind == ClosureFindingKindV2::MissingRequiredDependency)
        {
            return Err("observed required edge must pass".into());
        }
        if report.has_blocking() {
            return Err(format!(
                "satisfied closure must be clean: {:?}",
                report.findings
            ));
        }
        Ok(())
    }

    #[test]
    fn alias_resolves_to_logical_identity() -> Result<(), String> {
        let graph = parse_observed_metadata_graph_v2(&metadata(&[
            ("cargo-proof", &["proof-orchestrator"]),
            ("proof-orchestrator", &["intent-compiler"]),
            ("intent-compiler", &[]),
        ]))?;
        let mut engine = identity("proof-engine", "proof-orchestrator");
        engine.workspace_dependency_aliases = vec!["proof-orchestrator".to_string()];
        let identities = vec![
            identity("cargo-proof", "cargo-proof"),
            engine,
            identity("intent-engine", "intent-compiler"),
        ];
        let law = vec![forbidden("proof-engine", "intent-engine")];
        let input = ClosureValidationInputV2 {
            observed: &graph,
            identities: &identities,
            forbidden_edges: &law,
            required_edges: &[],
        };
        let report = validate_observed_closure(&input);
        if !report
            .findings
            .iter()
            .any(|f| f.kind == ClosureFindingKindV2::ForbiddenDependency)
        {
            return Err(format!(
                "alias-resolved forbidden edge must be caught: {:?}",
                report.findings
            ));
        }
        Ok(())
    }

    #[test]
    fn malformed_metadata_artifact_fails_loudly() -> Result<(), String> {
        if parse_observed_metadata_graph_v2("not json").is_ok() {
            return Err("malformed artifact must fail".into());
        }
        Ok(())
    }

    #[test]
    fn dependency_class_parses_optional_and_target_first() -> Result<(), String> {
        #[derive(serde::Deserialize)]
        struct DependencyFlags {
            kind: Option<String>,
            optional: bool,
            target: Option<String>,
        }
        let optional: DependencyFlags =
            serde_json::from_str(r#"{"kind":null,"optional":true,"target":null}"#)
                .map_err(|err| err.to_string())?;
        let class = ObservedDependencyClassV2::from_kind_and_flags(
            optional.kind.as_deref(),
            optional.optional,
            optional.target.as_deref(),
        );
        if class != ObservedDependencyClassV2::Optional {
            return Err("optional must take precedence".into());
        }
        let target: DependencyFlags =
            serde_json::from_str(r#"{"kind":"dev","optional":false,"target":"wasm32"}"#)
                .map_err(|err| err.to_string())?;
        let class = ObservedDependencyClassV2::from_kind_and_flags(
            target.kind.as_deref(),
            target.optional,
            target.target.as_deref(),
        );
        if class != ObservedDependencyClassV2::TargetSpecific {
            return Err("target predicate must take precedence over kind".into());
        }
        Ok(())
    }

    #[test]
    fn live_authority_forbidden_edges_catch_seeded_metadata() -> Result<(), String> {
        // The V2 law is parsed from the live authority (not static arrays)
        // and must catch a seeded violation identically.
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let law_text = std::fs::read_to_string(root.join("policy/product-crates.toml"))
            .map_err(|err| format!("read authority: {err}"))?;
        let identities_text = std::fs::read_to_string(root.join("policy/product-crates-v2.toml"))
            .map_err(|err| format!("read identities: {err}"))?;
        let (forbidden, required) = intent_model::parse_dependency_law_v1(&law_text)?;
        let identities = intent_model::parse_crate_identities_v1(&identities_text)?;

        let graph = parse_observed_metadata_graph_v2(&metadata(&[
            (
                "proof-orchestrator",
                &["intent-compiler", "intent-protocol"],
            ),
            ("intent-compiler", &[]),
            ("intent-protocol", &[]),
        ]))?;
        let input = ClosureValidationInputV2 {
            observed: &graph,
            identities: &identities,
            forbidden_edges: &forbidden,
            required_edges: &required,
        };
        let report = validate_observed_closure(&input);
        if !report.findings.iter().any(|f| {
            f.kind == ClosureFindingKindV2::ForbiddenDependency
                && f.message.contains("intent-engine")
        }) {
            return Err(format!(
                "live authority must catch proof-engine -> intent-engine: {:?}",
                report.findings
            ));
        }
        if report.findings.iter().any(|f| {
            f.kind == ClosureFindingKindV2::MissingRequiredDependency
                && f.message.contains("intent-protocol")
        }) {
            return Err(format!(
                "the seeded graph satisfies the live required edge; it must pass: {:?}",
                report.findings
            ));
        }
        Ok(())
    }
}
