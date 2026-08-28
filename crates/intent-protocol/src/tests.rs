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
        handoff: None,
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
fn repo_protocol_snapshot_reexports_canonical() -> Result<(), String> {
    // #3308: the copied repo-protocol files were deleted in favor of a
    // direct re-export from effortless-repo-protocol. This test verifies
    // the re-export produces the canonical types, not file copies.
    use crate::snapshot_package::repo_protocol::{
        REPOSITORY_SNAPSHOT_SCHEMA_ID, RepositorySnapshotV1, ResultClassV1,
    };
    // The re-exported schema id must match the canonical crate's constant.
    assert_eq!(
        REPOSITORY_SNAPSHOT_SCHEMA_ID,
        effortless_repo_protocol::REPOSITORY_SNAPSHOT_SCHEMA_ID,
        "re-exported schema id must match effortless-repo-protocol"
    );
    // Type identity: the re-exported types ARE the canonical types.
    // A RepositorySnapshotV1 constructed from the re-export must be usable
    // where the canonical type is expected without any conversion.
    let snapshot = RepositorySnapshotV1::new_committed_head(
        "identity",
        "sha1",
        effortless_repo_protocol::ResolvedRevisionV1 {
            requested: "HEAD".to_string(),
            commit: "abc".to_string(),
            tree: String::new(),
        },
    );
    let _: &effortless_repo_protocol::RepositorySnapshotV1 = &snapshot;
    let class = ResultClassV1::Completed;
    let _: effortless_repo_protocol::ResultClassV1 = class;
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

// --- #3964 INTENT-PROOF-HANDOFF-1 protocol fixtures (PR A) ---

use crate::obligation::{
    IntentEvidenceIndependenceV1, IntentObligationHandoffV1, IntentPlanEnrichmentV1,
    IntentProofHandoffDispositionV1, IntentSubjectInventoryCompletenessV1, IntentSubjectPostureV1,
};

fn handoff_row(
    obligation_id: &str,
    handoff: IntentObligationHandoffV1,
) -> crate::IntentPhaseObligationV1 {
    crate::IntentPhaseObligationV1 {
        handoff: Some(handoff),
        obligation_id: obligation_id.to_string(),
        phase: "pretest".to_string(),
        kind: crate::IntentPhaseObligationKindV1::EvidenceReview,
        statement: "Establish the PreTest evidence need.".to_string(),
        posture: crate::IntentObligationPostureV1::Blocking,
        evidence_refs: Vec::new(),
    }
}

fn ready_handoff() -> IntentObligationHandoffV1 {
    IntentObligationHandoffV1 {
        disposition: Some(IntentProofHandoffDispositionV1::ReadyForProofPlanning),
        evidence_purpose_refs: vec!["evidence-intent:positive-admission".to_string()],
        requested_evidence_class: Some("semantic_observation".to_string()),
        subject_selector_ref: Some("rust-item:cargo_intention::admit".to_string()),
        subject_posture: Some(IntentSubjectPostureV1::Exact),
        ..Default::default()
    }
}

fn sample_plan_with_rows(
    rows: Vec<crate::IntentPhaseObligationV1>,
) -> crate::IntentObligationPlanEnvelopeV1 {
    let snapshot = sample_snapshot();
    let identity = crate::IntentIdentityEnvelopeV1::new(
        snapshot,
        crate::IntentArtifactKindV1::ImplementationSlice,
        "slice/handoff-fixture",
        ".allow/spec-system/slices/handoff-fixture.toml",
        "sha256:v1:handoff-fixture",
    );
    crate::IntentObligationPlanEnvelopeV1::new(identity, "pretest", rows)
}

#[test]
fn ready_handoff_requires_exact_subject_purpose_class_and_selector() -> Result<(), String> {
    ready_handoff().validate()?;
    for mutate in [
        |h: &mut IntentObligationHandoffV1| h.subject_posture = Some(IntentSubjectPostureV1::Weak),
        |h: &mut IntentObligationHandoffV1| {
            h.subject_posture = Some(IntentSubjectPostureV1::Ambiguous);
        },
        |h: &mut IntentObligationHandoffV1| {
            h.subject_posture = Some(IntentSubjectPostureV1::Missing)
        },
        |h: &mut IntentObligationHandoffV1| {
            h.subject_posture = Some(IntentSubjectPostureV1::ZeroSubject);
        },
        |h: &mut IntentObligationHandoffV1| h.subject_posture = None,
        |h: &mut IntentObligationHandoffV1| h.evidence_purpose_refs = Vec::new(),
        |h: &mut IntentObligationHandoffV1| h.requested_evidence_class = None,
        |h: &mut IntentObligationHandoffV1| h.subject_selector_ref = None,
        |h: &mut IntentObligationHandoffV1| {
            h.unproven = vec!["compiler runtime behavior remains unobserved".to_string()];
        },
    ] {
        let mut broken = ready_handoff();
        mutate(&mut broken);
        if broken.validate().is_ok() {
            return Err("ready handoff accepted an incomplete basis".to_string());
        }
    }
    Ok(())
}

#[test]
fn zero_subject_and_weak_postures_stay_visibly_not_ready() -> Result<(), String> {
    for posture in [
        IntentSubjectPostureV1::ZeroSubject,
        IntentSubjectPostureV1::Weak,
        IntentSubjectPostureV1::Ambiguous,
        IntentSubjectPostureV1::Missing,
    ] {
        let handoff = IntentObligationHandoffV1 {
            disposition: Some(IntentProofHandoffDispositionV1::EvidenceDesignIncomplete),
            disposition_reason: Some("subject posture is not exact".to_string()),
            subject_posture: Some(posture),
            ..Default::default()
        };
        handoff.validate()?;
        let row = handoff_row("obl-posture", handoff);
        let json =
            serde_json::to_string(&row).map_err(|err| format!("serialize posture row: {err}"))?;
        if !json.contains(posture.as_str()) {
            return Err(format!(
                "posture {:?} did not survive serialization",
                posture
            ));
        }
    }
    Ok(())
}

#[test]
fn manual_and_native_outstanding_survive_without_provider_assumption() -> Result<(), String> {
    for independence in [
        IntentEvidenceIndependenceV1::ManualOutstanding,
        IntentEvidenceIndependenceV1::NativeOutstanding,
    ] {
        let handoff = IntentObligationHandoffV1 {
            disposition: Some(IntentProofHandoffDispositionV1::ManualOrNativeOutstanding),
            evidence_purpose_refs: vec!["evidence-intent:manual-review".to_string()],
            requested_evidence_class: Some("review_observation".to_string()),
            independence: Some(independence),
            subject_posture: Some(IntentSubjectPostureV1::Exact),
            subject_selector_ref: Some("rust-item:cargo_intention::admit".to_string()),
            ..Default::default()
        };
        handoff.validate()?;
        let json = serde_json::to_string(&handoff)
            .map_err(|err| format!("serialize independence handoff: {err}"))?;
        if !json.contains(independence.as_str()) {
            return Err("independence posture did not survive serialization".to_string());
        }
        let mut flipped = handoff.clone();
        flipped.disposition = Some(IntentProofHandoffDispositionV1::ReadyForProofPlanning);
        if flipped.validate().is_ok() {
            return Err("manual/native outstanding cannot be ready for proof planning".to_string());
        }
    }
    Ok(())
}

#[test]
fn distinct_evidence_purposes_and_requirements_produce_distinct_rows() -> Result<(), String> {
    let positive = handoff_row(
        "obl-positive",
        IntentObligationHandoffV1 {
            semantic_digest: Some("sha256:v1:row-positive".to_string()),
            ..ready_handoff()
        },
    );
    let negative = handoff_row(
        "obl-negative",
        IntentObligationHandoffV1 {
            evidence_purpose_refs: vec!["evidence-intent:negative-admission".to_string()],
            semantic_digest: Some("sha256:v1:row-negative".to_string()),
            ..ready_handoff()
        },
    );
    let other_requirement = handoff_row(
        "obl-second-requirement",
        IntentObligationHandoffV1 {
            requirement_ref: Some("requirement:admission-hysteresis".to_string()),
            semantic_digest: Some("sha256:v1:row-second".to_string()),
            ..ready_handoff()
        },
    );
    let plan = sample_plan_with_rows(vec![positive, negative, other_requirement]);
    let json = serde_json::to_string(&plan).map_err(|err| format!("serialize plan: {err}"))?;
    for marker in [
        "row-positive",
        "row-negative",
        "row-second",
        "evidence-intent:negative-admission",
        "requirement:admission-hysteresis",
    ] {
        if !json.contains(marker) {
            return Err(format!("distinct row marker {marker:?} missing"));
        }
    }
    Ok(())
}

#[test]
fn exact_basis_produces_deterministic_serialization() -> Result<(), String> {
    let build = || -> Result<String, String> {
        let enrichment = IntentPlanEnrichmentV1 {
            protocol_generation: Some(1),
            guidance_result_identity: Some("guidance-result:pretest-001".to_string()),
            resolved_config_identity: Some("intent-config:resolved-001".to_string()),
            repository_subject_identity: Some("repo-snapshot:v2:tree-abc".to_string()),
            requested_semantic_boundary: Some("pretest".to_string()),
            ..Default::default()
        };
        let mut plan = sample_plan_with_rows(vec![handoff_row("obl-1", ready_handoff())]);
        plan.enrichment = Some(enrichment);
        serde_json::to_string(&plan).map_err(|err| err.to_string())
    };
    let first = build()?;
    let second = build()?;
    if first != second {
        return Err("identical basis produced different serializations".to_string());
    }
    Ok(())
}

#[test]
fn staged_worktree_and_committed_subjects_stay_distinct() -> Result<(), String> {
    let mut digests = Vec::new();
    for basis in [
        "source:staged:abc",
        "source:worktree:def",
        "source:committed:123",
    ] {
        let handoff = IntentObligationHandoffV1 {
            source_identity: Some(basis.to_string()),
            ..ready_handoff()
        };
        digests.push(serde_json::to_string(&handoff).map_err(|err| err.to_string())?);
    }
    let unique: std::collections::BTreeSet<_> = digests.iter().collect();
    if unique.len() != 3 {
        return Err("staged/worktree/committed subjects collapsed".to_string());
    }
    Ok(())
}

#[test]
fn basis_movement_moves_only_the_affected_row() -> Result<(), String> {
    let unaffected = IntentObligationHandoffV1 {
        semantic_digest: Some("sha256:v1:row-unaffected".to_string()),
        ..ready_handoff()
    };
    let affected_before = IntentObligationHandoffV1 {
        source_identity: Some("source:committed:aaa".to_string()),
        semantic_digest: Some("sha256:v1:row-affected-before".to_string()),
        ..ready_handoff()
    };
    let affected_after = IntentObligationHandoffV1 {
        source_identity: Some("source:committed:bbb".to_string()),
        semantic_digest: Some("sha256:v1:row-affected-after".to_string()),
        ..ready_handoff()
    };
    let before = sample_plan_with_rows(vec![
        handoff_row("obl-a", affected_before),
        handoff_row("obl-b", unaffected.clone()),
    ]);
    let after = sample_plan_with_rows(vec![
        handoff_row("obl-a", affected_after),
        handoff_row("obl-b", unaffected.clone()),
    ]);
    let before_json = serde_json::to_string(&before).map_err(|err| err.to_string())?;
    let after_json = serde_json::to_string(&after).map_err(|err| err.to_string())?;
    if before_json == after_json {
        return Err("affected row basis movement did not change the plan".to_string());
    }
    let decoded_after: crate::IntentObligationPlanEnvelopeV1 =
        serde_json::from_str(&after_json).map_err(|err| err.to_string())?;
    let preserved = decoded_after
        .obligations
        .iter()
        .find(|row| row.obligation_id == "obl-b")
        .ok_or_else(|| "unaffected row missing after movement".to_string())?;
    if preserved.handoff != Some(unaffected) {
        return Err("unaffected row was not preserved exactly".to_string());
    }
    Ok(())
}

#[test]
fn historical_row_reads_without_fabricated_enrichment() -> Result<(), String> {
    let legacy_row = crate::IntentPhaseObligationV1 {
        handoff: None,
        obligation_id: "obl-legacy".to_string(),
        phase: "pretest".to_string(),
        kind: crate::IntentPhaseObligationKindV1::EvidenceReview,
        statement: "Legacy row.".to_string(),
        posture: crate::IntentObligationPostureV1::Blocking,
        evidence_refs: Vec::new(),
    };
    let legacy_plan = sample_plan_with_rows(vec![legacy_row]);
    let legacy_json =
        serde_json::to_string(&legacy_plan).map_err(|err| format!("serialize legacy: {err}"))?;
    if legacy_json.contains("\"handoff\"") || legacy_json.contains("\"enrichment\"") {
        return Err("legacy serialization must not emit enrichment keys".to_string());
    }
    let decoded: crate::IntentObligationPlanEnvelopeV1 = serde_json::from_str(&legacy_json)
        .map_err(|err| format!("historical envelope must stay readable: {err}"))?;
    let row = decoded
        .obligations
        .first()
        .ok_or_else(|| "legacy row missing".to_string())?;
    if row.handoff.is_some() {
        return Err("historical row must not fabricate handoff semantics".to_string());
    }
    if decoded.enrichment.is_some() {
        return Err("historical envelope must not fabricate enrichment".to_string());
    }
    Ok(())
}

#[test]
fn proof_and_provider_fields_are_rejected_from_the_envelope() -> Result<(), String> {
    for injected in [
        r#""selected_provider": "cargo-allow""#,
        r#""argv": ["cargo", "allow"]"#,
        r#""receipt": {"result": "pass"}"#,
        r#""gate_satisfied": true"#,
    ] {
        let legacy = format!(
            r#"{{
                "obligation_id": "obl-x",
                "phase": "pretest",
                "kind": "evidence_review",
                "statement": "s",
                "posture": "blocking",
                {injected}
            }}"#
        );
        let outcome: Result<crate::IntentPhaseObligationV1, _> = serde_json::from_str(&legacy);
        if outcome.is_ok() {
            return Err(format!("proof-owned field was accepted: {injected}"));
        }
    }
    Ok(())
}

#[test]
fn not_applicable_disposition_requires_a_reason() -> Result<(), String> {
    let without_reason = IntentObligationHandoffV1 {
        disposition: Some(IntentProofHandoffDispositionV1::NotApplicableWithReason),
        ..Default::default()
    };
    if without_reason.validate().is_ok() {
        return Err("not_applicable_with_reason accepted an empty reason".to_string());
    }
    let with_reason = IntentObligationHandoffV1 {
        disposition: Some(IntentProofHandoffDispositionV1::NotApplicableWithReason),
        disposition_reason: Some("obligation retired with its requirement".to_string()),
        ..Default::default()
    };
    with_reason.validate()?;
    Ok(())
}

#[test]
fn present_handoff_requires_an_explicit_disposition() -> Result<(), String> {
    let empty = IntentObligationHandoffV1::default();
    if empty.validate().is_ok() {
        return Err("an empty handoff block must not validate".to_string());
    }
    Ok(())
}

#[test]
fn blank_references_are_rejected() -> Result<(), String> {
    let mut blank_purpose = ready_handoff();
    blank_purpose.evidence_purpose_refs = vec!["   ".to_string()];
    if blank_purpose.validate().is_ok() {
        return Err("a blank evidence purpose reference was accepted".to_string());
    }
    let mut blank_discriminator = ready_handoff();
    blank_discriminator.discriminator_refs = vec![" ".to_string()];
    if blank_discriminator.validate().is_ok() {
        return Err("a blank discriminator reference was accepted".to_string());
    }
    Ok(())
}

#[test]
fn enrichment_validates_generation_and_reference_hygiene() -> Result<(), String> {
    let mut enrichment = IntentPlanEnrichmentV1 {
        protocol_generation: Some(0),
        ..Default::default()
    };
    if enrichment.validate().is_ok() {
        return Err("protocol generation zero was accepted".to_string());
    }
    enrichment.protocol_generation = Some(1);
    enrichment.requested_semantic_boundary = Some("   ".to_string());
    if enrichment.validate().is_ok() {
        return Err("blank semantic boundary was accepted".to_string());
    }
    enrichment.requested_semantic_boundary = Some("pretest".to_string());
    enrichment.validate()?;
    Ok(())
}

#[test]
fn decision_posture_and_inventory_completeness_roundtrip() -> Result<(), String> {
    let handoff = IntentObligationHandoffV1 {
        disposition: Some(IntentProofHandoffDispositionV1::RepositoryDecisionRequired),
        disposition_reason: Some("cutover stage needs a repository decision".to_string()),
        subject_inventory_completeness: Some(IntentSubjectInventoryCompletenessV1::Partial),
        subject_inventory_limitations: vec!["generated wrappers excluded".to_string()],
        ..Default::default()
    };
    let json = serde_json::to_string(&handoff).map_err(|err| err.to_string())?;
    let decoded: IntentObligationHandoffV1 =
        serde_json::from_str(&json).map_err(|err| err.to_string())?;
    if decoded.disposition != Some(IntentProofHandoffDispositionV1::RepositoryDecisionRequired)
        || decoded.subject_inventory_completeness
            != Some(IntentSubjectInventoryCompletenessV1::Partial)
    {
        return Err("decision posture or inventory completeness did not round-trip".to_string());
    }
    Ok(())
}
