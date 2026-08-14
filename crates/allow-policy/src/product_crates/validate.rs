use super::config::{ArchitectureManifest, parse_architecture_manifest_at};
use super::dependency_graph::{CargoMetadataGraph, DependencyClass};
use allow_core::CargoAllowResult;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArchitectureDiagnosticKind {
    DuplicateCrateOwner,
    UnownedWorkspaceCrate,
    UnknownOwnedCrate,
    EmptyManifest,
    ForbiddenProductDependency,
    ForbiddenCrateDependency,
    SharedProtocolDomainLeak,
    ManifestTopologyLinkMismatch,
    ManifestMoveLedgerLinkMismatch,
    PackageTopologyFamilyMismatch,
    ArchitectureCrateMissingFromTopology,
    PackageTopologyCrateMissingFromArchitecture,
    PlannedCrateNowPresent,
    MoveLedgerUnknownTargetCrate,
    MissingRequiredCrateDependency,
}

impl ArchitectureDiagnosticKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateCrateOwner => "duplicate_crate_owner",
            Self::UnownedWorkspaceCrate => "unowned_workspace_crate",
            Self::UnknownOwnedCrate => "unknown_owned_crate",
            Self::EmptyManifest => "empty_manifest",
            Self::ForbiddenProductDependency => "forbidden_product_dependency",
            Self::ForbiddenCrateDependency => "forbidden_crate_dependency",
            Self::SharedProtocolDomainLeak => "shared_protocol_domain_leak",
            Self::ManifestTopologyLinkMismatch => "manifest_topology_link_mismatch",
            Self::ManifestMoveLedgerLinkMismatch => "manifest_move_ledger_link_mismatch",
            Self::PackageTopologyFamilyMismatch => "package_topology_family_mismatch",
            Self::ArchitectureCrateMissingFromTopology => {
                "architecture_crate_missing_from_topology"
            }
            Self::PackageTopologyCrateMissingFromArchitecture => {
                "package_topology_crate_missing_from_architecture"
            }
            Self::PlannedCrateNowPresent => "planned_crate_now_present",
            Self::MoveLedgerUnknownTargetCrate => "move_ledger_unknown_target_crate",
            Self::MissingRequiredCrateDependency => "missing_required_crate_dependency",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchitectureDiagnostic {
    pub kind: ArchitectureDiagnosticKind,
    pub message: String,
    pub crate_names: Vec<String>,
    pub dependency_class: Option<DependencyClass>,
    pub dependency_path: Vec<String>,
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
            dependency_class: None,
            dependency_path: Vec::new(),
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
                    dependency_class: None,
                    dependency_path: Vec::new(),
                });
            }
        }
    }

    for shared in &manifest.shared_crate {
        if let Some(existing) = owners.insert(shared.name.clone(), "shared".to_string()) {
            diagnostics.push(ArchitectureDiagnostic {
                kind: ArchitectureDiagnosticKind::DuplicateCrateOwner,
                message: format!(
                    "crate `{}` owned by both `{existing}` and shared",
                    shared.name
                ),
                crate_names: vec![shared.name.clone()],
                dependency_class: None,
                dependency_path: Vec::new(),
            });
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
                dependency_class: None,
                dependency_path: Vec::new(),
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum CrateOwner {
    Product(String),
    Shared(String),
}

pub fn validate_dependency_law(
    manifest: &ArchitectureManifest,
    graph: &CargoMetadataGraph,
) -> Vec<ArchitectureDiagnostic> {
    let owners = crate_owner_map(manifest);
    let product_forbid = product_forbid_map(manifest);
    let shared_allowed = shared_allowed_map(manifest);
    let mut diagnostics = Vec::new();
    let mut seen = BTreeSet::new();

    for edge in &graph.edges {
        let Some(from_owner) = owners.get(&edge.from) else {
            continue;
        };
        let Some(to_owner) = owners.get(&edge.to) else {
            continue;
        };

        if let Some(diag) =
            check_forbidden_crate_dependency(manifest, &edge.from, &edge.to, edge.class, &owners)
        {
            let key = (
                diag.kind,
                diag.crate_names.join("->"),
                diag.dependency_class,
            );
            if seen.insert(key) {
                diagnostics.push(diag);
            }
            continue;
        }

        match (from_owner, to_owner) {
            (CrateOwner::Product(from_product), CrateOwner::Product(to_product)) => {
                if from_product == to_product {
                    continue;
                }
                if is_product_forbidden(from_product, to_product, &product_forbid) {
                    let path = vec![edge.from.clone(), edge.to.clone()];
                    let key = (
                        ArchitectureDiagnosticKind::ForbiddenProductDependency,
                        path.join("->"),
                        Some(edge.class),
                    );
                    if seen.insert(key) {
                        diagnostics.push(ArchitectureDiagnostic {
                            kind: ArchitectureDiagnosticKind::ForbiddenProductDependency,
                            message: format!(
                                "product `{from_product}` crate `{}` must not depend on product `{to_product}` crate `{}` via {} dependency",
                                edge.from,
                                edge.to,
                                edge.class.as_str()
                            ),
                            crate_names: path.clone(),
                            dependency_class: Some(edge.class),
                            dependency_path: path,
                        });
                    }
                }
            }
            (CrateOwner::Shared(shared_name), CrateOwner::Product(to_product)) => {
                let path = vec![edge.from.clone(), edge.to.clone()];
                let key = (
                    ArchitectureDiagnosticKind::SharedProtocolDomainLeak,
                    path.join("->"),
                    Some(edge.class),
                );
                if seen.insert(key) {
                    diagnostics.push(ArchitectureDiagnostic {
                        kind: ArchitectureDiagnosticKind::SharedProtocolDomainLeak,
                        message: format!(
                            "shared crate `{shared_name}` must not depend on product `{to_product}` crate `{}` via {} dependency",
                            edge.to,
                            edge.class.as_str()
                        ),
                        crate_names: path.clone(),
                        dependency_class: Some(edge.class),
                        dependency_path: path,
                    });
                }
            }
            (CrateOwner::Shared(shared_name), CrateOwner::Shared(to_shared)) => {
                if shared_name == to_shared {
                    continue;
                }
                let allowed = shared_allowed
                    .get(shared_name.as_str())
                    .map(|deps| deps.contains(to_shared.as_str()))
                    .unwrap_or(false);
                if !allowed {
                    let path = vec![edge.from.clone(), edge.to.clone()];
                    let key = (
                        ArchitectureDiagnosticKind::SharedProtocolDomainLeak,
                        path.join("->"),
                        Some(edge.class),
                    );
                    if seen.insert(key) {
                        diagnostics.push(ArchitectureDiagnostic {
                            kind: ArchitectureDiagnosticKind::SharedProtocolDomainLeak,
                            message: format!(
                                "shared crate `{shared_name}` may not depend on shared crate `{to_shared}` via {} dependency",
                                edge.class.as_str()
                            ),
                            crate_names: path.clone(),
                            dependency_class: Some(edge.class),
                            dependency_path: path,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    check_required_crate_dependencies(manifest, graph, &mut diagnostics);

    diagnostics
}

/// Required dependency paths must stay declared in the workspace graph
/// (#2936 / #3317). A missing required edge means the converged obligation
/// input authority was severed.
fn check_required_crate_dependencies(
    manifest: &ArchitectureManifest,
    graph: &CargoMetadataGraph,
    diagnostics: &mut Vec<ArchitectureDiagnostic>,
) {
    for rule in &manifest.required_crate_dependency {
        let from_name = rule.from_package.as_deref().unwrap_or(&rule.from);
        let present = graph
            .edges
            .iter()
            .any(|edge| edge.from == from_name && edge.to == rule.to);
        if !present {
            let rationale = rule
                .rationale_issue
                .map(|issue| format!(" (rationale: #{issue})"))
                .unwrap_or_default();
            diagnostics.push(ArchitectureDiagnostic {
                kind: ArchitectureDiagnosticKind::MissingRequiredCrateDependency,
                message: format!(
                    "crate `{}` must depend on crate `{}`{rationale}; the converged dependency path is not declared",
                    rule.from, rule.to
                ),
                crate_names: vec![rule.from.clone(), rule.to.clone()],
                dependency_class: None,
                dependency_path: vec![rule.from.clone(), rule.to.clone()],
            });
        }
    }
}

pub fn validate_architecture_with_dependency_graph(
    manifest: ArchitectureManifest,
    workspace_members: &[String],
    graph: &CargoMetadataGraph,
) -> (
    ArchitectureManifest,
    Vec<ArchitectureDiagnostic>,
    ArchitectureReport,
) {
    let (manifest, mut diagnostics, report) =
        validate_architecture_manifest(manifest, workspace_members);
    diagnostics.extend(validate_dependency_law(&manifest, graph));
    (manifest, diagnostics, report)
}

pub fn validate_architecture_with_dependency_graph_at(
    _root: &Path,
    manifest_path: &Path,
    workspace_members: &[String],
    graph: &CargoMetadataGraph,
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
    Ok(validate_architecture_with_dependency_graph(
        manifest,
        workspace_members,
        graph,
    ))
}

fn crate_owner_map(manifest: &ArchitectureManifest) -> BTreeMap<String, CrateOwner> {
    let mut owners = BTreeMap::new();
    for product in &manifest.product {
        for crate_name in &product.owned_crates {
            owners.insert(crate_name.clone(), CrateOwner::Product(product.id.clone()));
        }
    }
    for shared in &manifest.shared_crate {
        owners.insert(shared.name.clone(), CrateOwner::Shared(shared.name.clone()));
    }
    owners
}

fn product_forbid_map(manifest: &ArchitectureManifest) -> BTreeMap<String, BTreeSet<String>> {
    let mut map = BTreeMap::new();
    for product in &manifest.product {
        map.insert(
            product.id.clone(),
            product
                .forbid_product_dependencies
                .iter()
                .cloned()
                .collect(),
        );
    }
    map
}

fn shared_allowed_map(manifest: &ArchitectureManifest) -> BTreeMap<String, BTreeSet<String>> {
    let mut map = BTreeMap::new();
    for shared in &manifest.shared_crate {
        map.insert(
            shared.name.clone(),
            shared.allowed_domain_dependencies.iter().cloned().collect(),
        );
    }
    map
}

fn is_product_forbidden(
    from_product: &str,
    to_product: &str,
    product_forbid: &BTreeMap<String, BTreeSet<String>>,
) -> bool {
    product_forbid
        .get(from_product)
        .is_some_and(|forbidden| forbidden.contains(to_product))
}

fn check_forbidden_crate_dependency(
    manifest: &ArchitectureManifest,
    from: &str,
    to: &str,
    class: DependencyClass,
    owners: &BTreeMap<String, CrateOwner>,
) -> Option<ArchitectureDiagnostic> {
    for rule in &manifest.forbidden_crate_dependency {
        if rule.from == from && rule.to == to {
            let repair = rule
                .repair_hint
                .as_deref()
                .map(|hint| format!("; use `{hint}` instead"))
                .unwrap_or_default();
            let from_product = owners.get(from).and_then(crate_owner_product_id);
            let to_product = owners.get(to).and_then(crate_owner_product_id);
            let product_context = match (from_product, to_product) {
                (Some(from), Some(to)) => format!(" ({from} -> {to})"),
                _ => String::new(),
            };
            return Some(ArchitectureDiagnostic {
                kind: ArchitectureDiagnosticKind::ForbiddenCrateDependency,
                message: format!(
                    "crate `{from}` must not depend on crate `{to}` via {} dependency{product_context}{repair}",
                    class.as_str()
                ),
                crate_names: vec![from.to_string(), to.to_string()],
                dependency_class: Some(class),
                dependency_path: vec![from.to_string(), to.to_string()],
            });
        }
    }
    None
}

fn crate_owner_product_id(owner: &CrateOwner) -> Option<&str> {
    match owner {
        CrateOwner::Product(id) => Some(id.as_str()),
        CrateOwner::Shared(_) => None,
    }
}
