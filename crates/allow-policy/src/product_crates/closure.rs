//! Alias-, feature-, target-, and dependency-class-aware product closure
//! validation from bounded Cargo metadata (#2922).
//!
//! This module enhances the V1 dependency graph with finer-grained dependency
//! classes (optional, target-specific, feature-activated, workspace/path,
//! registry) and integrates with the V2 source-controlled identity map (#2921)
//! to map Cargo package IDs back to stable logical ownership.
//!
//! Rust code parses only committed/test fixture or workflow-produced JSON.
//! Tests use committed positive/negative metadata fixtures — they do not
//! invoke Cargo.

use allow_core::{CargoAllowError, CargoAllowResult};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

use crate::product_crates::v2::{ArchitectureManifestV2, CrateIdentityV2};

// ---------------------------------------------------------------------------
// Enhanced dependency class vocabulary (#2922)
// ---------------------------------------------------------------------------

/// Fine-grained dependency class retaining at least: normal, dev, build,
/// target-specific, optional, feature-activated, workspace/path, registry,
/// and process/CLI compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CargoDependencyClass {
    Normal,
    Dev,
    Build,
    TargetSpecific,
    Optional,
    FeatureActivated,
    WorkspacePath,
    Registry,
    ProcessCli,
}

impl CargoDependencyClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Dev => "dev",
            Self::Build => "build",
            Self::TargetSpecific => "target_specific",
            Self::Optional => "optional",
            Self::FeatureActivated => "feature_activated",
            Self::WorkspacePath => "workspace_path",
            Self::Registry => "registry",
            Self::ProcessCli => "process_cli",
        }
    }

    /// Parse the `kind` field from Cargo metadata dependency entries.
    pub(crate) fn from_kind_and_flags(
        kind: Option<&str>,
        optional: bool,
        target: Option<&str>,
    ) -> Self {
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

// ---------------------------------------------------------------------------
// Enhanced dependency edge (#2922)
// ---------------------------------------------------------------------------

/// A dependency edge with full class, feature activation, target predicate,
/// and source/resolution information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoDependencyEdge {
    pub from_package: String,
    pub to_package: String,
    pub dependency_alias: Option<String>,
    pub class: CargoDependencyClass,
    pub activating_feature: Option<String>,
    pub target_predicate: Option<String>,
    pub resolved: bool,
}

// ---------------------------------------------------------------------------
// Cargo package ID → V2 logical ID resolver (#2922)
// ---------------------------------------------------------------------------

/// Maps Cargo package names to V2 logical IDs through the identity map (#2921).
///
/// Resolves dependency aliases to canonical package names, then maps package
/// names to logical IDs. Ambiguous aliases (one alias → multiple packages)
/// produce an error.
#[derive(Debug, Clone)]
pub struct CargoPackageIdResolver {
    alias_to_package: BTreeMap<String, Vec<String>>,
    package_to_logical: BTreeMap<String, String>,
}

/// Error when resolving a Cargo package ID through the identity map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageResolutionError {
    AmbiguousAlias {
        alias: String,
        packages: Vec<String>,
    },
    UnknownPackage {
        package: String,
    },
}

impl CargoPackageIdResolver {
    /// Build a resolver from a V2 architecture manifest.
    pub fn from_manifest(manifest: &ArchitectureManifestV2) -> CargoAllowResult<Self> {
        let mut alias_to_package: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut package_to_logical: BTreeMap<String, String> = BTreeMap::new();

        for entry in &manifest.crate_identity {
            if package_to_logical
                .insert(entry.cargo_package_name.clone(), entry.logical_id.clone())
                .is_some()
            {
                return Err(CargoAllowError::new(format!(
                    "duplicate cargo_package_name `{}` in identity map",
                    entry.cargo_package_name
                )));
            }
            for alias in &entry.workspace_dependency_aliases {
                alias_to_package
                    .entry(alias.clone())
                    .or_default()
                    .push(entry.cargo_package_name.clone());
            }
        }

        // Validate alias uniqueness
        for (alias, packages) in &alias_to_package {
            let unique: std::collections::BTreeSet<_> = packages.iter().collect();
            if unique.len() > 1 {
                return Err(CargoAllowError::new(format!(
                    "ambiguous alias `{alias}` resolves to multiple packages: {packages:?}"
                )));
            }
        }

        Ok(Self {
            alias_to_package,
            package_to_logical,
        })
    }

    /// Resolve a Cargo package name (or alias) to a logical ID.
    pub fn resolve(&self, name_or_alias: &str) -> Result<&str, PackageResolutionError> {
        // Try direct package name first
        if let Some(logical) = self.package_to_logical.get(name_or_alias) {
            return Ok(logical);
        }
        // Try alias resolution
        if let Some(packages) = self.alias_to_package.get(name_or_alias) {
            if packages.len() > 1 {
                return Err(PackageResolutionError::AmbiguousAlias {
                    alias: name_or_alias.to_string(),
                    packages: packages.clone(),
                });
            }
            if let Some(package) = packages.first()
                && let Some(logical) = self.package_to_logical.get(package)
            {
                return Ok(logical);
            }
        }
        Err(PackageResolutionError::UnknownPackage {
            package: name_or_alias.to_string(),
        })
    }

    /// Returns true if the package name is known (directly or via alias).
    pub fn knows(&self, name_or_alias: &str) -> bool {
        self.package_to_logical.contains_key(name_or_alias)
            || self.alias_to_package.contains_key(name_or_alias)
    }
}

// ---------------------------------------------------------------------------
// Closure result vocabulary (#2922)
// ---------------------------------------------------------------------------

/// Closed result vocabulary for product closure validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClosureResultKind {
    Complete,
    IdentityConflict,
    UnclassifiedPackage,
    ForbiddenDependency,
    ExpiredTransition,
    UnpublishedDependency,
    FeatureClosureMismatch,
    TargetClosureMismatch,
    MetadataStale,
    LockfileMismatch,
    MalformedInput,
    UnsupportedGeneration,
    UnsupportedMetadataFormat,
    InstrumentFailure,
}

impl ClosureResultKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::IdentityConflict => "identity_conflict",
            Self::UnclassifiedPackage => "unclassified_package",
            Self::ForbiddenDependency => "forbidden_dependency",
            Self::ExpiredTransition => "expired_transition",
            Self::UnpublishedDependency => "unpublished_dependency",
            Self::FeatureClosureMismatch => "feature_closure_mismatch",
            Self::TargetClosureMismatch => "target_closure_mismatch",
            Self::MetadataStale => "metadata_stale",
            Self::LockfileMismatch => "lockfile_mismatch",
            Self::MalformedInput => "malformed_input",
            Self::UnsupportedGeneration => "unsupported_generation",
            Self::UnsupportedMetadataFormat => "unsupported_metadata_format",
            Self::InstrumentFailure => "instrument_failure",
        }
    }
}

/// A product closure diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureDiagnostic {
    pub kind: ClosureResultKind,
    pub message: String,
    pub package_names: Vec<String>,
    pub dependency_path: Vec<String>,
}

// ---------------------------------------------------------------------------
// Enhanced Cargo metadata graph parser (#2922)
// ---------------------------------------------------------------------------

/// Enhanced Cargo metadata graph with full dependency class information.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CargoMetadataGraphV2 {
    pub edges: Vec<CargoDependencyEdge>,
}

/// Parse a `cargo metadata --format-version 1` JSON blob into an enhanced
/// graph with full dependency class, optional, and target information.
///
/// Does not invoke Cargo — parses only the supplied JSON text.
pub fn parse_cargo_metadata_graph_v2(input: &str) -> CargoAllowResult<CargoMetadataGraphV2> {
    let parsed: MetadataJson = serde_json::from_str(input).map_err(|err| {
        CargoAllowError::new(format!("failed to parse cargo metadata JSON: {err}"))
    })?;

    let mut edges = Vec::new();
    for package in &parsed.packages {
        for dependency in &package.dependencies {
            let class = CargoDependencyClass::from_kind_and_flags(
                dependency.kind.as_deref(),
                dependency.optional,
                dependency.target.as_deref(),
            );
            edges.push(CargoDependencyEdge {
                from_package: package.name.clone(),
                to_package: dependency.name.clone(),
                dependency_alias: None, // Cargo metadata resolves aliases to package names
                class,
                activating_feature: dependency
                    .feature
                    .clone()
                    .and_then(|features| features.into_iter().next()),
                target_predicate: dependency.target.clone(),
                resolved: true,
            });
        }
    }

    Ok(CargoMetadataGraphV2 { edges })
}

/// Load the enhanced metadata graph from a workspace by reading member
/// manifests directly (no Cargo invocation).
pub fn load_workspace_metadata_graph_v2(root: &Path) -> CargoAllowResult<CargoMetadataGraphV2> {
    let members = super::workspace::workspace_members_from_manifest(root)?;
    let mut edges = Vec::new();
    for member in members {
        let manifest_path = root.join(&member).join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest_path).map_err(|err| {
            CargoAllowError::new(format!(
                "workspace crate manifest unreadable at {}: {err}",
                manifest_path.display()
            ))
        })?;
        let parsed: toml::Value = toml::from_str(&text).map_err(|err| {
            CargoAllowError::new(format!(
                "workspace crate manifest parse error at {}: {err}",
                manifest_path.display()
            ))
        })?;
        let Some(package_name) = parsed
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
        else {
            continue;
        };
        for section_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
            let Some(section) = parsed.get(section_name) else {
                continue;
            };
            let Some(table) = section.as_table() else {
                continue;
            };
            let class = match section_name {
                "dev-dependencies" => CargoDependencyClass::Dev,
                "build-dependencies" => CargoDependencyClass::Build,
                _ => CargoDependencyClass::Normal,
            };
            for (dep_name, dep_value) in table {
                let (to_package, optional, target) = extract_dep_info(dep_name, dep_value);
                let final_class = if optional {
                    CargoDependencyClass::Optional
                } else if target.is_some() {
                    CargoDependencyClass::TargetSpecific
                } else {
                    class
                };
                edges.push(CargoDependencyEdge {
                    from_package: package_name.to_string(),
                    to_package,
                    dependency_alias: None,
                    class: final_class,
                    activating_feature: None,
                    target_predicate: target,
                    resolved: false,
                });
            }
        }
    }
    Ok(CargoMetadataGraphV2 { edges })
}

fn extract_dep_info(name: &str, value: &toml::Value) -> (String, bool, Option<String>) {
    match value {
        toml::Value::Table(t) => {
            let package = t
                .get("package")
                .and_then(|v| v.as_str())
                .unwrap_or(name)
                .to_string();
            let optional = t.get("optional").and_then(|v| v.as_bool()).unwrap_or(false);
            let target = t
                .get("target")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            (package, optional, target)
        }
        _ => (name.to_string(), false, None),
    }
}

/// Compute the shortest dependency path in the enhanced graph using BFS.
///
/// Returns the ordered list of package names from `from` to `to`, or `None`
/// if no path exists. Deterministic regardless of input traversal order
/// (uses BTreeMap for adjacency and visited sets).
pub fn shortest_closure_path(
    graph: &CargoMetadataGraphV2,
    from: &str,
    to: &str,
) -> Option<Vec<String>> {
    if from == to {
        return Some(vec![from.to_string()]);
    }
    let mut adjacency: BTreeMap<&str, Vec<&CargoDependencyEdge>> = BTreeMap::new();
    for edge in &graph.edges {
        adjacency
            .entry(edge.from_package.as_str())
            .or_default()
            .push(edge);
    }
    let mut queue = std::collections::VecDeque::from([(from, vec![from.to_string()])]);
    let mut visited: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    visited.insert(from);
    while let Some((current, path)) = queue.pop_front() {
        let Some(neighbors) = adjacency.get(current) else {
            continue;
        };
        // Sort neighbors for deterministic output
        let mut sorted: Vec<_> = neighbors.iter().collect();
        sorted.sort_by(|a, b| a.to_package.cmp(&b.to_package));
        for edge in sorted {
            if visited.contains(edge.to_package.as_str()) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push(edge.to_package.clone());
            if edge.to_package == to {
                return Some(next_path);
            }
            visited.insert(edge.to_package.as_str());
            queue.push_back((&edge.to_package, next_path));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Cargo metadata JSON structures (private)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct MetadataJson {
    #[serde(default)]
    packages: Vec<MetadataPackage>,
}

#[derive(Debug, Deserialize)]
struct MetadataPackage {
    name: String,
    #[serde(default)]
    dependencies: Vec<MetadataDependency>,
}

#[derive(Debug, Deserialize)]
struct MetadataDependency {
    name: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    optional: bool,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    feature: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Identity map lookup helper (#2922)
// ---------------------------------------------------------------------------

/// Look up a [`CrateIdentityV2`] by Cargo package name.
pub fn find_identity_by_package<'a>(
    manifest: &'a ArchitectureManifestV2,
    package_name: &str,
) -> Option<&'a CrateIdentityV2> {
    manifest
        .crate_identity
        .iter()
        .find(|e| e.cargo_package_name == package_name)
}

/// Look up a [`CrateIdentityV2`] by Rust library name.
pub fn find_identity_by_library<'a>(
    manifest: &'a ArchitectureManifestV2,
    library_name: &str,
) -> Option<&'a CrateIdentityV2> {
    manifest
        .crate_identity
        .iter()
        .find(|e| e.rust_library_name == library_name)
}
