use super::config::ArchitectureManifest;
use super::validate::{ArchitectureDiagnostic, ArchitectureDiagnosticKind};
use crate::product_move::{ProductMoveLedger, parse_product_move_ledger_at};
use crate::product_packages::{ProductPackageTopology, parse_product_package_topology_at};
use allow_core::CargoAllowResult;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DenominatorReport {
    pub architecture_crate_count: usize,
    pub topology_package_count: usize,
    pub move_ledger_target_crate_count: usize,
    pub workspace_member_count: usize,
    pub planned_crate_count: usize,
}

pub fn validate_architecture_denominators(
    manifest: &ArchitectureManifest,
    topology: &ProductPackageTopology,
    ledger: &ProductMoveLedger,
    workspace_members: &[String],
) -> (Vec<ArchitectureDiagnostic>, DenominatorReport) {
    let mut diagnostics = Vec::new();
    let owners = architecture_crate_owners(manifest);
    let workspace_crates = workspace_crate_names(workspace_members);
    let topology_families = topology_family_map(topology);
    let ledger_targets = move_ledger_target_crates(ledger);

    if topology.linked_architecture_manifest != manifest.manifest_id {
        diagnostics.push(diagnostic(
            ArchitectureDiagnosticKind::ManifestTopologyLinkMismatch,
            format!(
                "package topology links `{}` but architecture manifest id is `{}`",
                topology.linked_architecture_manifest, manifest.manifest_id
            ),
            vec![
                topology.linked_architecture_manifest.clone(),
                manifest.manifest_id.clone(),
            ],
        ));
    }

    if manifest.linked_move_ledger != ledger.ledger_id {
        diagnostics.push(diagnostic(
            ArchitectureDiagnosticKind::ManifestMoveLedgerLinkMismatch,
            format!(
                "architecture manifest links `{}` but move ledger id is `{}`",
                manifest.linked_move_ledger, ledger.ledger_id
            ),
            vec![
                manifest.linked_move_ledger.clone(),
                ledger.ledger_id.clone(),
            ],
        ));
    }

    for planned in &manifest.planned_crate {
        if workspace_crates.contains(&planned.name) {
            diagnostics.push(diagnostic(
                ArchitectureDiagnosticKind::PlannedCrateNowPresent,
                format!(
                    "planned crate `{}` is present in workspace; remove planned entry or move to owned inventory",
                    planned.name
                ),
                vec![planned.name.clone()],
            ));
        }
    }

    for (crate_name, owner) in &owners {
        let Some(family) = topology_families.get(crate_name) else {
            diagnostics.push(diagnostic(
                ArchitectureDiagnosticKind::ArchitectureCrateMissingFromTopology,
                format!("architecture-owned crate `{crate_name}` is missing from package topology"),
                vec![crate_name.clone()],
            ));
            continue;
        };
        let expected_family = expected_topology_family(owner);
        if family != expected_family {
            diagnostics.push(diagnostic(
                ArchitectureDiagnosticKind::PackageTopologyFamilyMismatch,
                format!(
                    "crate `{crate_name}` owned by `{owner}` but package topology classifies it under `{family}`"
                ),
                vec![crate_name.clone(), owner.clone(), family.clone()],
            ));
        }
    }

    for package in &topology.package {
        if !owners.contains_key(&package.package) {
            diagnostics.push(diagnostic(
                ArchitectureDiagnosticKind::PackageTopologyCrateMissingFromArchitecture,
                format!(
                    "package topology entry `{}` is not owned by architecture manifest",
                    package.package
                ),
                vec![package.package.clone()],
            ));
        }
    }

    for target_crate in &ledger_targets {
        if !owners.contains_key(target_crate) {
            diagnostics.push(diagnostic(
                ArchitectureDiagnosticKind::MoveLedgerUnknownTargetCrate,
                format!(
                    "move ledger references target crate `{target_crate}` unknown to architecture manifest"
                ),
                vec![target_crate.clone()],
            ));
        }
    }

    let report = DenominatorReport {
        architecture_crate_count: owners.len(),
        topology_package_count: topology.package.len(),
        move_ledger_target_crate_count: ledger_targets.len(),
        workspace_member_count: workspace_crates.len(),
        planned_crate_count: manifest.planned_crate.len(),
    };

    (diagnostics, report)
}

pub fn validate_architecture_denominators_at(
    root: &Path,
    manifest: &ArchitectureManifest,
    workspace_members: &[String],
) -> CargoAllowResult<(Vec<ArchitectureDiagnostic>, DenominatorReport)> {
    let topology_path = root.join("policy/product-package-topology.toml");
    let ledger_path = root.join("policy/product-move-ledger.toml");
    let topology_text = std::fs::read_to_string(&topology_path).map_err(|err| {
        allow_core::CargoAllowError::new(format!(
            "package topology unreadable at {}: {err}",
            topology_path.display()
        ))
    })?;
    let ledger_text = std::fs::read_to_string(&ledger_path).map_err(|err| {
        allow_core::CargoAllowError::new(format!(
            "product move ledger unreadable at {}: {err}",
            ledger_path.display()
        ))
    })?;
    let topology = parse_product_package_topology_at(Some(&topology_path), &topology_text)?;
    let ledger = parse_product_move_ledger_at(Some(&ledger_path), &ledger_text)?;
    Ok(validate_architecture_denominators(
        manifest,
        &topology,
        &ledger,
        workspace_members,
    ))
}

fn architecture_crate_owners(manifest: &ArchitectureManifest) -> BTreeMap<String, String> {
    let mut owners = BTreeMap::new();
    for product in &manifest.product {
        for crate_name in &product.owned_crates {
            owners.insert(crate_name.clone(), product.id.clone());
        }
    }
    for shared in &manifest.shared_crate {
        owners.insert(shared.name.clone(), "shared".to_string());
    }
    owners
}

fn topology_family_map(topology: &ProductPackageTopology) -> BTreeMap<String, String> {
    topology
        .package
        .iter()
        .map(|entry| (entry.package.clone(), entry.product_family.clone()))
        .collect()
}

fn move_ledger_target_crates(ledger: &ProductMoveLedger) -> BTreeSet<String> {
    ledger
        .entry
        .iter()
        .map(|entry| entry.target_crate.clone())
        .filter(|crate_name| !crate_name.is_empty())
        .collect()
}

fn workspace_crate_names(workspace_members: &[String]) -> BTreeSet<String> {
    workspace_members
        .iter()
        .filter_map(|member| member.rsplit('/').next().map(|name| name.to_string()))
        .collect()
}

fn expected_topology_family(owner: &str) -> &str {
    owner
}

fn diagnostic(
    kind: ArchitectureDiagnosticKind,
    message: String,
    crate_names: Vec<String>,
) -> ArchitectureDiagnostic {
    ArchitectureDiagnostic {
        kind,
        message,
        crate_names,
        dependency_class: None,
        dependency_path: Vec::new(),
    }
}
