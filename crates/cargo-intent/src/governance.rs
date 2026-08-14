//! Governance receipt operation: compile the live authority into a
//! deterministic typed receipt (#2942 step 4 / #3540).
//!
//! This is the repository-CI entry point: cargo-intent reads the live
//! governance authority files and workspace member manifests (bounded file
//! reads at the product layer; cargo-allow source scans never spawn Cargo
//! or gain intent dependencies), feeds them through the intent-engine
//! reconciliation and closure validation operations, and emits a
//! deterministic, versioned receipt with candidate package-row projections.
//!
//! The operation performs no package publish, tag, or provider execution.

use std::collections::BTreeMap;
use std::path::Path;

use intent_engine::{
    ClosureValidationInputV2, ComponentDispositionRecordV2, GovernanceReconciliationInputV2,
    ObservedDependencyClassV2, ObservedDependencyEdgeV2, ObservedMetadataGraphV2,
    WorkspaceMemberFactV2, reconcile_governance_authority, validate_observed_closure,
};
use intent_model::{
    GovernanceCrateIdentityV2, GovernanceForbiddenEdgeV2, GovernancePackagePostureV2,
    GovernanceRequiredEdgeV2, MoveReferenceV2, ParityReferenceV2, ShimReferenceV2,
    TransitionExpiryV2, parse_crate_identities_v1, parse_dependency_law_v1,
    parse_move_references_v1, parse_package_postures_v1, parse_parity_references_v1,
    parse_shim_references_v1, stable_hash_hex,
};
use serde::Serialize;

pub const GOVERNANCE_RECEIPT_SCHEMA_ID: &str = "cargo-intent.governance-receipt.v1";
pub const GOVERNANCE_CLAIM_BOUNDARY: &str = "Governance authority compilation over supplied repository files; no package publish, tag, or provider execution.";

/// One candidate package row projected from package posture + membership.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CandidatePackageRowV1 {
    pub logical_id: String,
    pub cargo_package_name: String,
    pub version_line: String,
    pub package_version: String,
    pub publish: bool,
    pub candidate_inclusion: bool,
    pub release_order: u32,
}

/// Deterministic governance validation receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GovernanceReceiptV1 {
    pub schema_id: String,
    pub result: &'static str,
    pub blocking_finding_count: usize,
    /// Finding kinds with counts, sorted by kind for determinism.
    pub finding_kinds: Vec<(String, usize)>,
    pub deletion_eligible_count: usize,
    pub candidate_package_rows: Vec<CandidatePackageRowV1>,
    pub receipt_digest: String,
    pub claim_boundary: String,
}

/// Parsed live-authority state, ready for the engine operations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GovernanceAuthorityStateV1 {
    pub identities: Vec<GovernanceCrateIdentityV2>,
    pub postures: Vec<GovernancePackagePostureV2>,
    pub moves: Vec<MoveReferenceV2>,
    pub shims: Vec<ShimReferenceV2>,
    pub expiries: Vec<TransitionExpiryV2>,
    pub parity: Vec<ParityReferenceV2>,
    pub forbidden: Vec<GovernanceForbiddenEdgeV2>,
    pub required: Vec<GovernanceRequiredEdgeV2>,
}

/// Pure receipt compilation: deterministic for identical input, distinct
/// for changed input. Dispositions enter when an authored disposition
/// record exists; today's authority carries none, so the disposition seam
/// is exercised through engine fixtures.
pub fn compile_governance_receipt(
    state: &GovernanceAuthorityStateV1,
    members: &[WorkspaceMemberFactV2],
    observed: &ObservedMetadataGraphV2,
) -> GovernanceReceiptV1 {
    let empty_dispositions: [ComponentDispositionRecordV2; 0] = [];
    let reconciliation_input = GovernanceReconciliationInputV2 {
        crate_identities: &state.identities,
        package_postures: &state.postures,
        moves: &state.moves,
        shims: &state.shims,
        expiries: &state.expiries,
        parity_cases: &state.parity,
        cutover_receipts: &[],
        workspace_members: members,
        dispositions: &empty_dispositions,
    };
    let reconciliation = reconcile_governance_authority(&reconciliation_input);

    let closure_input = ClosureValidationInputV2 {
        observed,
        identities: &state.identities,
        forbidden_edges: &state.forbidden,
        required_edges: &state.required,
    };
    let closure = validate_observed_closure(&closure_input);

    let mut kind_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut blocking = 0usize;
    for finding in &reconciliation.findings {
        if finding.severity == intent_engine::GovernanceFindingSeverityV2::Blocking {
            blocking += 1;
        }
        *kind_counts
            .entry(format!("governance:{}", finding.kind.as_str()))
            .or_default() += 1;
    }
    for finding in &closure.findings {
        blocking += 1;
        *kind_counts
            .entry(format!("closure:{}", finding.kind.as_str()))
            .or_default() += 1;
    }
    let deletion_eligible_count = reconciliation
        .deletion_eligibility
        .iter()
        .filter(|decision| decision.eligibility == intent_engine::DeletionEligibilityV2::Eligible)
        .count();

    let mut candidate_package_rows: Vec<CandidatePackageRowV1> = state
        .postures
        .iter()
        .map(|posture| CandidatePackageRowV1 {
            logical_id: posture.logical_id.clone(),
            cargo_package_name: posture.cargo_package_name.clone(),
            version_line: posture.version_line.clone(),
            package_version: posture.package_version.clone(),
            publish: posture.membership.publish,
            candidate_inclusion: posture.membership.candidate_inclusion,
            release_order: posture.release_order,
        })
        .collect();
    candidate_package_rows.sort_by(|a, b| {
        a.release_order
            .cmp(&b.release_order)
            .then_with(|| a.logical_id.cmp(&b.logical_id))
    });

    let result: &'static str = if blocking == 0 { "passed" } else { "failed" };

    let mut digest_source = String::new();
    digest_source.push_str(GOVERNANCE_RECEIPT_SCHEMA_ID);
    digest_source.push('|');
    digest_source.push_str(result);
    digest_source.push('|');
    digest_source.push_str(&blocking.to_string());
    digest_source.push('|');
    for (kind, count) in &kind_counts {
        digest_source.push_str(kind);
        digest_source.push('=');
        digest_source.push_str(&count.to_string());
        digest_source.push(';');
    }
    digest_source.push_str(&deletion_eligible_count.to_string());
    digest_source.push('|');
    for row in &candidate_package_rows {
        digest_source.push_str(&row.logical_id);
        digest_source.push('@');
        digest_source.push_str(&row.package_version);
        digest_source.push(';');
    }

    GovernanceReceiptV1 {
        schema_id: GOVERNANCE_RECEIPT_SCHEMA_ID.to_string(),
        result,
        blocking_finding_count: blocking,
        finding_kinds: kind_counts.into_iter().collect(),
        deletion_eligible_count,
        candidate_package_rows,
        receipt_digest: stable_hash_hex(&digest_source),
        claim_boundary: GOVERNANCE_CLAIM_BOUNDARY.to_string(),
    }
}

/// Read the live governance authority files into the parsed state.
pub fn read_governance_authority_at(root: &Path) -> Result<GovernanceAuthorityStateV1, String> {
    let read = |rel: &str| -> Result<String, String> {
        std::fs::read_to_string(root.join(rel)).map_err(|err| format!("read {rel}: {err}"))
    };

    let identities = parse_crate_identities_v1(&read("policy/product-crates-v2.toml")?)?;
    let postures = parse_package_postures_v1(&read("policy/product-package-topology-v2.toml")?)?;
    let moves = parse_move_references_v1(&read("policy/product-move-ledger.toml")?)?;
    let (shims, expiries) = parse_shim_references_v1(&read("policy/extraction-shims.toml")?)?;
    let parity = parse_parity_references_v1(&read("policy/extraction-parity.toml")?)?;
    let (forbidden, required) = parse_dependency_law_v1(&read("policy/product-crates.toml")?)?;
    Ok(GovernanceAuthorityStateV1 {
        identities,
        postures,
        moves,
        shims,
        expiries,
        parity,
        forbidden,
        required,
    })
}

/// Read the live authority and workspace facts, then compile the receipt.
///
/// This is the only layer that touches the filesystem: the live authority
/// files, the workspace member list, and member manifests. No Cargo
/// invocation — manifests are parsed as text.
pub fn compile_governance_receipt_at(root: &Path) -> Result<GovernanceReceiptV1, String> {
    let state = read_governance_authority_at(root)?;
    let (members, observed) = read_workspace_facts(root)?;
    Ok(compile_governance_receipt(&state, &members, &observed))
}

/// Read workspace member facts and the observed dependency graph from
/// member manifests (text parsing only; no Cargo invocation).
fn read_workspace_facts(
    root: &Path,
) -> Result<(Vec<WorkspaceMemberFactV2>, ObservedMetadataGraphV2), String> {
    let workspace_text = std::fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|err| format!("read workspace manifest: {err}"))?;
    let workspace: toml::Table = toml::from_str(&workspace_text)
        .map_err(|err| format!("parse workspace manifest: {err}"))?;
    let members = workspace
        .get("workspace")
        .and_then(|section| section.get("members"))
        .and_then(|value| value.as_array())
        .ok_or_else(|| "workspace manifest missing members".to_string())?;

    let mut facts = Vec::with_capacity(members.len());
    let mut packages = Vec::with_capacity(members.len());
    let mut edges = Vec::new();
    for member in members {
        let Some(member_path) = member.as_str() else {
            continue;
        };
        let manifest_path = root.join(member_path).join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest_path)
            .map_err(|err| format!("read member manifest {}: {err}", manifest_path.display()))?;
        let manifest: toml::Table = toml::from_str(&text)
            .map_err(|err| format!("parse member manifest {}: {err}", manifest_path.display()))?;
        let Some(package_name) = manifest
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(|name| name.as_str())
        else {
            continue;
        };
        facts.push(WorkspaceMemberFactV2 {
            workspace_path: member_path.to_string(),
            cargo_package_name: package_name.to_string(),
        });
        packages.push(package_name.to_string());
        for (section_name, section_class) in [
            ("dependencies", ObservedDependencyClassV2::Normal),
            ("dev-dependencies", ObservedDependencyClassV2::Dev),
            ("build-dependencies", ObservedDependencyClassV2::Build),
        ] {
            let Some(section) = manifest.get(section_name).and_then(|v| v.as_table()) else {
                continue;
            };
            for (dep_name, dep_value) in section {
                let (to_package, optional, target) = extract_dep_info(dep_name, dep_value);
                let class = if optional {
                    ObservedDependencyClassV2::Optional
                } else if target.is_some() {
                    ObservedDependencyClassV2::TargetSpecific
                } else {
                    section_class
                };
                edges.push(ObservedDependencyEdgeV2 {
                    from_package: package_name.to_string(),
                    to_package,
                    class,
                });
            }
        }
    }

    // The governance denominator is the workspace closure: scope the
    // observed graph to member packages so external registry dependencies
    // (serde, toml, ...) are not unclassified governance findings.
    let member_packages: std::collections::BTreeSet<&str> = facts
        .iter()
        .map(|fact| fact.cargo_package_name.as_str())
        .collect();
    edges.retain(|edge| member_packages.contains(edge.to_package.as_str()));

    Ok((facts, ObservedMetadataGraphV2 { packages, edges }))
}

fn extract_dep_info(name: &str, value: &toml::Value) -> (String, bool, Option<String>) {
    match value {
        toml::Value::Table(table) => {
            let package = table
                .get("package")
                .and_then(|value| value.as_str())
                .unwrap_or(name)
                .to_string();
            let optional = table
                .get("optional")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let target = table
                .get("target")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            (package, optional, target)
        }
        _ => (name.to_string(), false, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn live_workspace_compiles_a_passing_receipt() -> Result<(), String> {
        let receipt = compile_governance_receipt_at(&workspace_root())?;
        if receipt.schema_id != GOVERNANCE_RECEIPT_SCHEMA_ID {
            return Err("receipt schema drift".into());
        }
        if receipt.result != "passed" {
            return Err(format!(
                "live authority must reconcile clean: {:?}",
                receipt.finding_kinds
            ));
        }
        if receipt.candidate_package_rows.is_empty() {
            return Err("candidate package rows must project from postures".into());
        }
        if !receipt.receipt_digest.starts_with("fnv1a64:") {
            return Err(format!("digest drift: {}", receipt.receipt_digest));
        }
        Ok(())
    }

    #[test]
    fn receipt_is_deterministic() -> Result<(), String> {
        let first = compile_governance_receipt_at(&workspace_root())?;
        let second = compile_governance_receipt_at(&workspace_root())?;
        if first != second {
            return Err("identical inputs must produce identical receipts".into());
        }
        let first_json = serde_json::to_string(&first).map_err(|err| err.to_string())?;
        let second_json = serde_json::to_string(&second).map_err(|err| err.to_string())?;
        if first_json != second_json {
            return Err("receipt JSON must be byte-identical across runs".into());
        }
        Ok(())
    }

    #[test]
    fn seeded_forbidden_edge_fails_the_receipt_and_changes_the_digest() -> Result<(), String> {
        let root = workspace_root();
        let (members, observed) = read_workspace_facts(&root)?;
        let state_source = read_governance_authority_at(&root)?;

        let clean = compile_governance_receipt(&state_source, &members, &observed);
        if clean.result != "passed" {
            return Err("clean baseline expected".into());
        }

        // Seed a forbidden edge the observed workspace violates
        // (cargo-allow -> allow-core is observed everywhere).
        let mut violated = state_source.clone();
        violated.forbidden.push(GovernanceForbiddenEdgeV2 {
            from_logical_id: "cargo-allow".to_string(),
            to_logical_id: "allow-core".to_string(),
            repair_hint: None,
        });
        let failed = compile_governance_receipt(&violated, &members, &observed);
        if failed.result != "failed" || failed.blocking_finding_count == 0 {
            return Err(format!(
                "seeded forbidden edge must fail the receipt: {:?}",
                failed.finding_kinds
            ));
        }
        if !failed
            .finding_kinds
            .iter()
            .any(|(kind, _)| kind == "closure:forbidden_dependency")
        {
            return Err(format!(
                "forbidden_dependency kind expected: {:?}",
                failed.finding_kinds
            ));
        }
        if failed.receipt_digest == clean.receipt_digest {
            return Err("changed input must change the receipt digest".into());
        }
        Ok(())
    }

    #[test]
    fn required_edge_satisfied_passes_and_absent_edge_fails() -> Result<(), String> {
        let root = workspace_root();
        let (members, observed) = read_workspace_facts(&root)?;
        let full = read_governance_authority_at(&root)?;
        let base = GovernanceAuthorityStateV1 {
            identities: full.identities,
            postures: full.postures,
            moves: Vec::new(),
            shims: Vec::new(),
            expiries: Vec::new(),
            parity: Vec::new(),
            forbidden: Vec::new(),
            required: Vec::new(),
        };

        // The live workspace declares proof-engine -> intent-protocol, so a
        // required edge over it is satisfied and must not appear.
        let mut satisfied = base.clone();
        satisfied.required.push(GovernanceRequiredEdgeV2 {
            from_logical_id: "proof-engine".to_string(),
            to_logical_id: "intent-protocol".to_string(),
            rationale_issue: Some(2936),
        });
        let receipt = compile_governance_receipt(&satisfied, &members, &observed);
        if receipt
            .finding_kinds
            .iter()
            .any(|(kind, _)| kind == "closure:missing_required_dependency")
        {
            return Err(format!(
                "satisfied required edge must not flag: {:?}",
                receipt.finding_kinds
            ));
        }

        // allow-core -> proof-protocol is never observed; requiring it fails.
        let mut absent = base;
        absent.required.push(GovernanceRequiredEdgeV2 {
            from_logical_id: "allow-core".to_string(),
            to_logical_id: "proof-protocol".to_string(),
            rationale_issue: None,
        });
        let receipt = compile_governance_receipt(&absent, &members, &observed);
        if receipt.result != "failed"
            || !receipt
                .finding_kinds
                .iter()
                .any(|(kind, _)| kind == "closure:missing_required_dependency")
        {
            return Err(format!(
                "absent required edge must fail: {:?}",
                receipt.finding_kinds
            ));
        }
        Ok(())
    }

    #[test]
    fn candidate_rows_are_sorted_by_release_order() -> Result<(), String> {
        let receipt = compile_governance_receipt_at(&workspace_root())?;
        let orders: Vec<u32> = receipt
            .candidate_package_rows
            .iter()
            .map(|row| row.release_order)
            .collect();
        let mut sorted = orders.clone();
        sorted.sort_unstable();
        if orders != sorted {
            return Err("candidate rows must sort by release_order".into());
        }
        Ok(())
    }
}
