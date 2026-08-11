use crate::parity::{
    IdentityQueryParityContract, ObligationPlanParityContract, ViewDiffClosureParityContract,
    load_identity_query_parity_contract, load_obligation_plan_parity_contract,
    load_view_diff_closure_parity_contract,
};
use crate::{
    REPOSITORY_SNAPSHOT_SCHEMA_ID, RepositorySnapshotKindV1, RepositorySnapshotV1,
    ResolvedRevisionV1,
};
use std::path::PathBuf;

#[test]
fn parity_contracts_load_from_fixtures() -> Result<(), String> {
    let root = workspace_root();
    for path in crate::parity::identity_query_parity_contract_paths(&root) {
        let contract = load_identity_query_parity_contract(&path)?;
        validate_identity_contract(&contract)?;
    }
    for path in crate::parity::view_diff_closure_parity_contract_paths(&root) {
        let contract = load_view_diff_closure_parity_contract(&path)?;
        validate_view_diff_closure_contract(&contract)?;
    }
    for path in crate::parity::obligation_plan_parity_contract_paths(&root) {
        let contract = load_obligation_plan_parity_contract(&path)?;
        validate_obligation_plan_contract(&contract)?;
    }
    Ok(())
}

#[test]
fn query_envelope_roundtrip_preserves_identity() -> Result<(), String> {
    let snapshot = sample_snapshot();
    let identity = crate::IntentIdentityEnvelopeV1::new(
        snapshot,
        crate::IntentArtifactKindV1::SpecSystemConfig,
        "policy/spec-system.toml",
        "policy/spec-system.toml",
        "sha256:v1:fixture-config",
    );
    let query = crate::IntentQueryEnvelopeV1::new(
        identity,
        crate::IntentQueryKindV1::LoadArtifact,
        "policy/spec-system.toml",
    );
    let json =
        serde_json::to_string(&query).map_err(|err| format!("serialize query envelope: {err}"))?;
    let decoded: crate::IntentQueryEnvelopeV1 =
        serde_json::from_str(&json).map_err(|err| format!("deserialize query envelope: {err}"))?;
    if decoded.artifact_kind() != crate::IntentArtifactKindV1::SpecSystemConfig {
        return Err("artifact kind did not round-trip".to_string());
    }
    if decoded.selector != "policy/spec-system.toml" {
        return Err("selector did not round-trip".to_string());
    }
    Ok(())
}

#[test]
fn obligation_plan_envelope_roundtrip() -> Result<(), String> {
    let snapshot = sample_snapshot();
    let identity = crate::IntentIdentityEnvelopeV1::new(
        snapshot,
        crate::IntentArtifactKindV1::ImplementationSlice,
        "slice/self-hosted-runtime-promotion-v1",
        ".allow/spec-system/slices/self-hosted-runtime-promotion-v1.toml",
        "sha256:v1:fixture-slice",
    );
    let obligation = crate::IntentPhaseObligationV1 {
        obligation_id: "obligation-evidence-closure".to_string(),
        phase: "precommit".to_string(),
        kind: crate::IntentPhaseObligationKindV1::EvidenceReview,
        statement: "Review evidence closure before support promotion.".to_string(),
        posture: crate::IntentObligationPostureV1::Blocking,
        evidence_refs: vec![
            "doc:docs/specs/CARGO-ALLOW-SPEC-0009-design-to-proof-walking-skeleton.md".to_string(),
        ],
    };
    let plan = crate::IntentObligationPlanEnvelopeV1::new(identity, "precommit", vec![obligation]);
    let json =
        serde_json::to_string(&plan).map_err(|err| format!("serialize obligation plan: {err}"))?;
    let decoded: crate::IntentObligationPlanEnvelopeV1 =
        serde_json::from_str(&json).map_err(|err| format!("deserialize obligation plan: {err}"))?;
    if decoded.obligations.len() != 1 {
        return Err("obligation count did not round-trip".to_string());
    }
    Ok(())
}

#[test]
fn view_and_closure_envelopes_roundtrip() -> Result<(), String> {
    let snapshot = sample_snapshot();
    let view = crate::IntentViewEnvelopeV1::new(
        snapshot.clone(),
        crate::IntentViewKindV1::CommittedTree,
        "HEAD",
    );
    let view_json =
        serde_json::to_string(&view).map_err(|err| format!("serialize view envelope: {err}"))?;
    let _: crate::IntentViewEnvelopeV1 = serde_json::from_str(&view_json)
        .map_err(|err| format!("deserialize view envelope: {err}"))?;

    let closure = crate::IntentSourceClosureEnvelopeV1::new(
        snapshot,
        vec!["policy/spec-system.toml".to_string()],
    );
    let closure_json = serde_json::to_string(&closure)
        .map_err(|err| format!("serialize closure envelope: {err}"))?;
    let decoded: crate::IntentSourceClosureEnvelopeV1 = serde_json::from_str(&closure_json)
        .map_err(|err| format!("deserialize closure envelope: {err}"))?;
    if decoded.selected_paths != ["policy/spec-system.toml"] {
        return Err("selected_paths did not round-trip".to_string());
    }
    Ok(())
}

#[test]
fn repository_protocol_uses_canonical_package_without_private_copy() -> Result<(), String> {
    let root = workspace_root();
    let manifest = std::fs::read_to_string(root.join("crates/intent-protocol/Cargo.toml"))
        .map_err(|err| format!("read intent-protocol manifest: {err}"))?;
    let manifest: toml::Value =
        toml::from_str(&manifest).map_err(|err| format!("parse intent-protocol manifest: {err}"))?;
    let dependencies = manifest
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "intent-protocol dependencies table missing".to_string())?;
    let dependency = dependencies
        .get("effortless-repo-protocol")
        .ok_or_else(|| "intent-protocol does not use effortless-repo-protocol".to_string())?;
    if dependency
        .get("workspace")
        .and_then(toml::Value::as_bool)
        != Some(true)
    {
        return Err(
            "intent-protocol effortless-repo-protocol dependency is not workspace-owned".to_string(),
        );
    }
    let copied_root = root.join("crates/intent-protocol/src/snapshot_package/repo_protocol");
    if copied_root.exists() {
        return Err(format!(
            "private repository protocol copy remains at {}",
            copied_root.display()
        ));
    }
    Ok(())
}

fn validate_identity_contract(contract: &IdentityQueryParityContract) -> Result<(), String> {
    if contract.scenario_id.is_empty() {
        return Err("empty scenario_id".to_string());
    }
    if contract.move_ledger_entry != "move-allow-report-spec-system-schema" {
        return Err(format!(
            "unexpected move ledger entry {}",
            contract.move_ledger_entry
        ));
    }
    if contract.required_identity_fields.len() < 4 {
        return Err("required_identity_fields too small".to_string());
    }
    Ok(())
}

fn validate_view_diff_closure_contract(
    contract: &ViewDiffClosureParityContract,
) -> Result<(), String> {
    if contract.scenario_id.is_empty() {
        return Err("empty scenario_id".to_string());
    }
    if contract.move_ledger_entry != "move-allow-report-spec-system-schema" {
        return Err(format!(
            "unexpected move ledger entry {}",
            contract.move_ledger_entry
        ));
    }
    if contract.required_view_fields.len() < 3 {
        return Err("required_view_fields too small".to_string());
    }
    if contract.required_closure_fields.len() < 2 {
        return Err("required_closure_fields too small".to_string());
    }
    Ok(())
}

fn validate_obligation_plan_contract(
    contract: &ObligationPlanParityContract,
) -> Result<(), String> {
    if contract.scenario_id.is_empty() {
        return Err("empty scenario_id".to_string());
    }
    if contract.move_ledger_entry != "move-allow-report-spec-system-schema" {
        return Err(format!(
            "unexpected move ledger entry {}",
            contract.move_ledger_entry
        ));
    }
    if contract.required_obligation_fields.len() < 4 {
        return Err("required_obligation_fields too small".to_string());
    }
    Ok(())
}

fn sample_snapshot() -> RepositorySnapshotV1 {
    RepositorySnapshotV1 {
        schema_id: REPOSITORY_SNAPSHOT_SCHEMA_ID.to_string(),
        kind: RepositorySnapshotKindV1::CommittedHead,
        root_identity: "sha256:v1:fixture-root".to_string(),
        object_format: "sha1".to_string(),
        head: ResolvedRevisionV1 {
            requested: "HEAD".to_string(),
            commit: "cccccccccccccccccccccccccccccccccccccccc".to_string(),
            tree: "tttttttttttttttttttttttttttttttttttttttt".to_string(),
        },
        base: None,
        merge_base: None,
        dirty_state: "not_probed".to_string(),
        selected_paths: Vec::new(),
        selected_source_closure: "sha256:v1:fixture-closure".to_string(),
        limitations: Vec::new(),
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
