//! Spec graph compiler — moved from allow-policy to intent-engine (#3520 / #2935 slice 5a).
//!
//! This is the canonical graph compiler. It consumes authored requirement/slice/seam/evidence
//! inputs and produces a CompiledSpecGraph with diagnostics and a deterministic snapshot ID.

use intent_model::stable_hash_hex;
use intent_model::{
    CompiledSpecGraph, EvidenceClaimId, EvidenceClaimNode, EvidenceClaimRegistration,
    EvidenceDispositionState, EvidenceSubjectId, EvidenceSubjectNode, EvidenceSubjectRegistration,
    EvidenceSubjectRole, GraphCompileInput, GraphDiagnostic, GraphDiagnosticCode, GraphSnapshotId,
    ImplementationClaimStatus, ImplementationSeamId, ImplementationSeamNode,
    ImplementationSeamRegistration, ImplementationSliceClass, ImplementationSliceId,
    ImplementationSliceNode, ImplementationSliceV1, RequirementClaimClass, RequirementGraph,
    RequirementId, RequirementNode, RequirementStatus, SourceLocation,
    SupportClaimDispositionState,
};
use std::collections::BTreeMap;
use std::fmt::Write;

pub fn compile_spec_graph(input: GraphCompileInput) -> CompiledSpecGraph {
    let mut diagnostics = Vec::new();
    let requirements = compile_requirements(input.requirement_graphs, &mut diagnostics);
    let slices = compile_slices(input.implementation_slices, &requirements, &mut diagnostics);
    let seams = compile_seams(input.seams, &mut diagnostics);
    let subjects = compile_subjects(input.subjects, &mut diagnostics);
    let evidence_claims = compile_evidence_claims(
        input.evidence_claims,
        &requirements,
        &slices,
        &seams,
        &subjects,
        &mut diagnostics,
    );
    diagnostics.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then_with(|| left.subject.cmp(&right.subject))
            .then_with(|| left.message.cmp(&right.message))
    });

    let snapshot_id = graph_snapshot_id(
        &requirements,
        &slices,
        &seams,
        &evidence_claims,
        &subjects,
        &diagnostics,
    );
    CompiledSpecGraph {
        snapshot_id,
        requirements,
        slices,
        seams,
        evidence_claims,
        subjects,
        diagnostics,
    }
}

fn compile_requirements(
    graphs: Vec<RequirementGraph>,
    diagnostics: &mut Vec<GraphDiagnostic>,
) -> BTreeMap<RequirementId, RequirementNode> {
    let mut requirements = BTreeMap::new();
    for graph in graphs {
        for requirement in graph.requirements {
            let id = requirement.id.clone();
            let node = RequirementNode {
                id: id.clone(),
                generation: requirement.generation,
                status: requirement.status,
                claim_class: requirement.claim_class,
                source: SourceLocation {
                    path: graph.source.path.clone().unwrap_or_default(),
                    line: Some(graph.source.start_line),
                    symbol: Some(requirement.local_id),
                },
            };
            insert_unique(
                &mut requirements,
                id.clone(),
                node,
                id.as_str(),
                diagnostics,
            );
        }
    }
    requirements
}

fn compile_slices(
    slices_input: Vec<ImplementationSliceV1>,
    requirements: &BTreeMap<RequirementId, RequirementNode>,
    diagnostics: &mut Vec<GraphDiagnostic>,
) -> BTreeMap<ImplementationSliceId, ImplementationSliceNode> {
    let mut slices = BTreeMap::new();
    for slice in slices_input {
        for delta in &slice.requirement_delta {
            match requirements.get(&delta.requirement_id) {
                None => diagnostics.push(GraphDiagnostic::new(
                    GraphDiagnosticCode::UnknownRequirement,
                    delta.requirement_id.as_str(),
                    format!(
                        "slice {} references unknown requirement {}",
                        slice.id.as_str(),
                        delta.requirement_id.as_str()
                    ),
                )),
                Some(requirement) if requirement.generation != delta.requirement_generation => {
                    diagnostics.push(GraphDiagnostic::new(
                        GraphDiagnosticCode::SliceRequirementGenerationMismatch,
                        delta.requirement_id.as_str(),
                        format!(
                            "slice {} uses requirement generation {}, current generation is {}",
                            slice.id.as_str(),
                            delta.requirement_generation,
                            requirement.generation
                        ),
                    ));
                }
                Some(_) => {}
            }
        }
        let id = slice.id.clone();
        let node = ImplementationSliceNode {
            id: id.clone(),
            generation: slice.generation,
            change_class: slice.change_class,
            implementation_claim_status: slice.implementation_claim.status,
            evidence_state: slice.evidence.state,
            support_claim_state: slice.support_claim.state,
            requirement_ids: slice
                .requirement_delta
                .into_iter()
                .map(|delta| delta.requirement_id)
                .collect(),
            owned_seams: slice.owned_seams.into_iter().collect(),
            shared_seams: slice.shared_seams.into_iter().collect(),
            forbidden_seams: slice.forbidden_seams.into_iter().collect(),
        };
        insert_unique(&mut slices, id.clone(), node, id.as_str(), diagnostics);
    }
    slices
}

fn compile_seams(
    registrations: Vec<ImplementationSeamRegistration>,
    diagnostics: &mut Vec<GraphDiagnostic>,
) -> BTreeMap<ImplementationSeamId, ImplementationSeamNode> {
    let mut seams = BTreeMap::new();
    for seam in registrations {
        let id = seam.id.clone();
        let node = ImplementationSeamNode {
            id: id.clone(),
            owner: seam.owner,
            operation: seam.operation,
            source: seam.source,
        };
        insert_unique(&mut seams, id.clone(), node, id.as_str(), diagnostics);
    }
    seams
}

fn compile_subjects(
    registrations: Vec<EvidenceSubjectRegistration>,
    diagnostics: &mut Vec<GraphDiagnostic>,
) -> BTreeMap<EvidenceSubjectId, EvidenceSubjectNode> {
    let mut subjects = BTreeMap::new();
    for subject in registrations {
        let id = subject.id.clone();
        let node = EvidenceSubjectNode {
            id: id.clone(),
            role: subject.role,
            package: subject.package,
            target: subject.target,
            module_path: subject.module_path,
            test_name: subject.test_name,
            source: subject.source,
            source_identity: subject.source_identity,
        };
        insert_unique(&mut subjects, id.clone(), node, id.as_str(), diagnostics);
    }
    subjects
}

fn compile_evidence_claims(
    registrations: Vec<EvidenceClaimRegistration>,
    requirements: &BTreeMap<RequirementId, RequirementNode>,
    slices: &BTreeMap<ImplementationSliceId, ImplementationSliceNode>,
    seams: &BTreeMap<ImplementationSeamId, ImplementationSeamNode>,
    subjects: &BTreeMap<EvidenceSubjectId, EvidenceSubjectNode>,
    diagnostics: &mut Vec<GraphDiagnostic>,
) -> BTreeMap<EvidenceClaimId, EvidenceClaimNode> {
    let mut claims = BTreeMap::new();
    for claim in registrations {
        validate_claim(&claim, requirements, slices, seams, subjects, diagnostics);
        let id = claim.id.clone();
        let node = EvidenceClaimNode {
            id: id.clone(),
            requirement_id: claim.requirement_id,
            slice_id: claim.slice_id,
            seam_id: claim.seam_id,
            purpose: claim.purpose,
            precondition: claim.precondition,
            operation: claim.operation,
            expected_observable: claim.expected_observable,
            discriminator: claim.discriminator,
            claim_boundary: claim.claim_boundary,
            source: claim.source,
            subject_ids: claim.subject_ids,
            related_subject_ids: claim.related_subject_ids,
        };
        insert_unique(&mut claims, id.clone(), node, id.as_str(), diagnostics);
    }
    claims
}

fn validate_claim(
    claim: &EvidenceClaimRegistration,
    requirements: &BTreeMap<RequirementId, RequirementNode>,
    slices: &BTreeMap<ImplementationSliceId, ImplementationSliceNode>,
    seams: &BTreeMap<ImplementationSeamId, ImplementationSeamNode>,
    subjects: &BTreeMap<EvidenceSubjectId, EvidenceSubjectNode>,
    diagnostics: &mut Vec<GraphDiagnostic>,
) {
    if !requirements.contains_key(&claim.requirement_id) {
        diagnostics.push(GraphDiagnostic::new(
            GraphDiagnosticCode::UnknownRequirement,
            claim.requirement_id.as_str(),
            format!(
                "evidence {} references an unknown requirement",
                claim.id.as_str()
            ),
        ));
    }
    let slice = slices.get(&claim.slice_id);
    if slice.is_none() {
        diagnostics.push(GraphDiagnostic::new(
            GraphDiagnosticCode::UnknownSlice,
            claim.slice_id.as_str(),
            format!("evidence {} references an unknown slice", claim.id.as_str()),
        ));
    }
    if !seams.contains_key(&claim.seam_id) {
        diagnostics.push(GraphDiagnostic::new(
            GraphDiagnosticCode::UnknownSeam,
            claim.seam_id.as_str(),
            format!("evidence {} references an unknown seam", claim.id.as_str()),
        ));
    }
    if claim.subject_ids.is_empty() {
        diagnostics.push(GraphDiagnostic::new(
            GraphDiagnosticCode::EmptyEvidenceSubjects,
            claim.id.as_str(),
            "an evidence claim must name at least one exact subject",
        ));
    }
    if let Some(slice) = slice {
        let seam = claim.seam_id.as_str();
        if slice.forbidden_seams.contains(seam) {
            diagnostics.push(GraphDiagnostic::new(
                GraphDiagnosticCode::ForbiddenSeam,
                claim.slice_id.as_str(),
                format!("evidence {} uses forbidden seam {seam}", claim.id.as_str()),
            ));
        } else if !slice.owned_seams.contains(seam) && !slice.shared_seams.contains(seam) {
            diagnostics.push(GraphDiagnostic::new(
                GraphDiagnosticCode::SeamNotDeclaredBySlice,
                claim.slice_id.as_str(),
                format!("evidence {} uses undeclared seam {seam}", claim.id.as_str()),
            ));
        }
    }
    for subject_id in &claim.subject_ids {
        match subjects.get(subject_id) {
            None => diagnostics.push(GraphDiagnostic::new(
                GraphDiagnosticCode::UnknownSubject,
                subject_id.as_str(),
                format!(
                    "evidence {} references an unknown exact subject",
                    claim.id.as_str()
                ),
            )),
            Some(subject) if subject.role != EvidenceSubjectRole::ExactEvidence => {
                diagnostics.push(GraphDiagnostic::new(
                    GraphDiagnosticCode::ExactSubjectMarkedWeak,
                    subject_id.as_str(),
                    format!(
                        "evidence {} maps a weak subject as exact evidence",
                        claim.id.as_str()
                    ),
                ));
            }
            Some(_) => {}
        }
    }
    for subject_id in &claim.related_subject_ids {
        match subjects.get(subject_id) {
            None => diagnostics.push(GraphDiagnostic::new(
                GraphDiagnosticCode::UnknownSubject,
                subject_id.as_str(),
                format!(
                    "evidence {} references an unknown related subject",
                    claim.id.as_str()
                ),
            )),
            Some(subject) if subject.role != EvidenceSubjectRole::RelatedWeak => {
                diagnostics.push(GraphDiagnostic::new(
                    GraphDiagnosticCode::RelatedSubjectMarkedExact,
                    subject_id.as_str(),
                    format!(
                        "evidence {} maps an exact subject as a weak neighbor",
                        claim.id.as_str()
                    ),
                ));
            }
            Some(_) => {}
        }
    }
}

fn insert_unique<K: Ord, V>(
    map: &mut BTreeMap<K, V>,
    key: K,
    value: V,
    subject: &str,
    diagnostics: &mut Vec<GraphDiagnostic>,
) {
    if map.insert(key, value).is_some() {
        diagnostics.push(GraphDiagnostic::new(
            GraphDiagnosticCode::DuplicateId,
            subject,
            format!("duplicate graph id {subject}"),
        ));
    }
}

fn graph_snapshot_id(
    requirements: &BTreeMap<RequirementId, RequirementNode>,
    slices: &BTreeMap<ImplementationSliceId, ImplementationSliceNode>,
    seams: &BTreeMap<ImplementationSeamId, ImplementationSeamNode>,
    claims: &BTreeMap<EvidenceClaimId, EvidenceClaimNode>,
    subjects: &BTreeMap<EvidenceSubjectId, EvidenceSubjectNode>,
    diagnostics: &[GraphDiagnostic],
) -> GraphSnapshotId {
    let mut canonical = String::new();
    for node in requirements.values() {
        let _ = writeln!(
            canonical,
            "requirement|{}|{}|{}|{}|{}",
            node.id.as_str(),
            node.generation,
            requirement_status_name(node.status),
            requirement_claim_class_name(node.claim_class),
            node.source.path
        );
    }
    for node in slices.values() {
        let _ = writeln!(
            canonical,
            "slice|{}|{}|{}|{}|{}|{}",
            node.id.as_str(),
            node.generation,
            slice_class_name(node.change_class),
            implementation_status_name(node.implementation_claim_status),
            evidence_state_name(node.evidence_state),
            support_state_name(node.support_claim_state)
        );
        for requirement_id in &node.requirement_ids {
            let _ = writeln!(
                canonical,
                "slice_requirement|{}|{}",
                node.id.as_str(),
                requirement_id.as_str()
            );
        }
        for seam in &node.owned_seams {
            let _ = writeln!(canonical, "slice_owned_seam|{}|{}", node.id.as_str(), seam);
        }
        for seam in &node.shared_seams {
            let _ = writeln!(canonical, "slice_shared_seam|{}|{}", node.id.as_str(), seam);
        }
        for seam in &node.forbidden_seams {
            let _ = writeln!(
                canonical,
                "slice_forbidden_seam|{}|{}",
                node.id.as_str(),
                seam
            );
        }
    }
    for node in seams.values() {
        let _ = writeln!(
            canonical,
            "seam|{}|{}|{}|{}",
            node.id.as_str(),
            node.owner,
            node.operation,
            node.source.path
        );
    }
    for node in subjects.values() {
        let _ = writeln!(
            canonical,
            "subject|{}|{}|{}|{}|{}|{}|{}",
            node.id.as_str(),
            node.role.as_str(),
            node.package,
            node.target,
            node.module_path,
            node.test_name,
            node.source_identity
        );
    }
    for node in claims.values() {
        let _ = writeln!(
            canonical,
            "evidence|{}|{}|{}|{}|{}",
            node.id.as_str(),
            node.requirement_id.as_str(),
            node.slice_id.as_str(),
            node.seam_id.as_str(),
            node.purpose.as_str()
        );
        for subject_id in &node.subject_ids {
            let _ = writeln!(
                canonical,
                "evidence_subject|{}|{}",
                node.id.as_str(),
                subject_id.as_str()
            );
        }
        for subject_id in &node.related_subject_ids {
            let _ = writeln!(
                canonical,
                "related_subject|{}|{}",
                node.id.as_str(),
                subject_id.as_str()
            );
        }
    }
    for diagnostic in diagnostics {
        let _ = writeln!(
            canonical,
            "diagnostic|{}|{}|{}",
            diagnostic.code.as_str(),
            diagnostic.subject,
            diagnostic.message
        );
    }
    GraphSnapshotId(stable_hash_hex(&canonical))
}

fn requirement_status_name(status: RequirementStatus) -> &'static str {
    match status {
        RequirementStatus::Draft => "draft",
        RequirementStatus::Accepted => "accepted",
        RequirementStatus::Deferred => "deferred",
        RequirementStatus::Superseded => "superseded",
        RequirementStatus::Rejected => "rejected",
        RequirementStatus::RemovedWithReplacement => "removed_with_replacement",
    }
}

fn requirement_claim_class_name(class: RequirementClaimClass) -> &'static str {
    match class {
        RequirementClaimClass::RuntimeBehavior => "runtime_behavior",
    }
}

fn slice_class_name(class: ImplementationSliceClass) -> &'static str {
    match class {
        ImplementationSliceClass::SpecOrPolicyChange => "spec_or_policy_change",
        ImplementationSliceClass::BehaviorChange => "behavior_change",
    }
}

fn implementation_status_name(status: ImplementationClaimStatus) -> &'static str {
    match status {
        ImplementationClaimStatus::Outstanding => "outstanding",
        ImplementationClaimStatus::Partial => "partial",
        ImplementationClaimStatus::Implemented => "implemented",
        ImplementationClaimStatus::Unsupported => "unsupported",
        ImplementationClaimStatus::NotApplicable => "not_applicable",
        ImplementationClaimStatus::Removed => "removed",
    }
}

fn evidence_state_name(state: EvidenceDispositionState) -> &'static str {
    match state {
        EvidenceDispositionState::Outstanding => "outstanding",
        EvidenceDispositionState::Current => "current",
    }
}

fn support_state_name(state: SupportClaimDispositionState) -> &'static str {
    match state {
        SupportClaimDispositionState::Unchanged => "unchanged",
        SupportClaimDispositionState::Promoted => "promoted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use intent_model::EvidencePurpose;
    use intent_model::{parse_implementation_slice, parse_requirement_blocks};

    const SPEC: &str = r#"---
id: CARGO-ALLOW-SPEC-0009
---

```toml cargo-allow-requirements
schema_version = "1.0"

[[requirement]]
id = "spec-only-runtime-promotion"
generation = 1
status = "accepted"
statement = "A spec-only change cannot publish runtime completion."
claim_class = "runtime_behavior"
```
"#;

    const SLICE: &str = r#"
schema_version = "2.0"
id = "cargo-allow.slice.self-hosted-runtime-promotion.v1"
generation = 1
source_issue = "issue:2206"
design_reference = "CARGO-ALLOW-SPEC-0009#spec-only-runtime-promotion"
change_class = "spec_or_policy_change"
claim_boundary = "No runtime completion claim."
owned_seams = ["seam:runtime-promotion"]

[[requirement_delta]]
requirement_id = "CARGO-ALLOW-SPEC-0009#spec-only-runtime-promotion"
requirement_generation = 1

[implementation_claim]
status = "outstanding"

[evidence]
state = "outstanding"

[support_claim]
state = "unchanged"
"#;

    fn input() -> Result<GraphCompileInput, String> {
        let requirement_graph =
            parse_requirement_blocks(SPEC).map_err(|error| error.to_string())?;
        let slice = parse_implementation_slice(SLICE).map_err(|error| error.to_string())?;
        let exact = EvidenceSubjectId("subject:exact-negative".into());
        let weak = EvidenceSubjectId("subject:weak-neighbor".into());
        Ok(GraphCompileInput {
            requirement_graphs: vec![requirement_graph],
            implementation_slices: vec![slice],
            seams: vec![ImplementationSeamRegistration {
                id: ImplementationSeamId("seam:runtime-promotion".into()),
                owner: "allow-policy".into(),
                operation: "validate runtime promotion".into(),
                source: SourceLocation::new(
                    "crates/allow-policy/src/spec_system/runtime_promotion.rs",
                )
                .with_symbol("validate_runtime_promotion"),
            }],
            subjects: vec![
                EvidenceSubjectRegistration {
                    id: exact.clone(),
                    role: EvidenceSubjectRole::ExactEvidence,
                    package: "allow-policy".into(),
                    target: "lib".into(),
                    module_path: "spec_system::runtime_promotion::tests".into(),
                    test_name: "spec_or_policy_slice_rejects_unproved_runtime_promotion".into(),
                    source: SourceLocation::new(
                        "crates/allow-policy/src/spec_system/runtime_promotion.rs",
                    ),
                    source_identity: "fnv1a64:exact".into(),
                },
                EvidenceSubjectRegistration {
                    id: weak.clone(),
                    role: EvidenceSubjectRole::RelatedWeak,
                    package: "allow-policy".into(),
                    target: "lib".into(),
                    module_path: "spec_system::runtime_promotion::tests".into(),
                    test_name: "spec_or_policy_slice_rejects_invalid_transition_broadly".into(),
                    source: SourceLocation::new(
                        "crates/allow-policy/src/spec_system/runtime_promotion.rs",
                    ),
                    source_identity: "fnv1a64:weak".into(),
                },
            ],
            evidence_claims: vec![EvidenceClaimRegistration {
                id: EvidenceClaimId("evidence:forbidden-runtime-promotion".into()),
                requirement_id: RequirementId(
                    "CARGO-ALLOW-SPEC-0009#spec-only-runtime-promotion".into(),
                ),
                slice_id: ImplementationSliceId(
                    "cargo-allow.slice.self-hosted-runtime-promotion.v1".into(),
                ),
                seam_id: ImplementationSeamId("seam:runtime-promotion".into()),
                purpose: EvidencePurpose::ForbiddenRuntimePromotion,
                precondition: "spec-only runtime requirement".into(),
                operation: "claim implemented".into(),
                expected_observable: "typed rejection".into(),
                discriminator: "SpecOnlyRuntimeImplementationClaim".into(),
                claim_boundary: "Does not prove execution.".into(),
                source: SourceLocation::new(
                    "docs/specs/CARGO-ALLOW-SPEC-0009-design-to-proof-walking-skeleton.md",
                ),
                subject_ids: vec![exact],
                related_subject_ids: vec![weak],
            }],
        })
    }

    #[test]
    fn compiles_minimal_claim_graph_deterministically() -> Result<(), String> {
        let first = compile_spec_graph(input()?);
        let second = compile_spec_graph(input()?);
        assert!(first.diagnostics.is_empty());
        assert_eq!(first.snapshot_id, second.snapshot_id);
        assert_eq!(first.requirements.len(), 1);
        assert_eq!(first.slices.len(), 1);
        assert_eq!(first.seams.len(), 1);
        assert_eq!(first.evidence_claims.len(), 1);
        assert_eq!(first.subjects.len(), 2);
        let evidence_id = EvidenceClaimId("evidence:forbidden-runtime-promotion".into());
        assert_eq!(first.subjects_for_evidence(&evidence_id).len(), 1);
        // related_subjects_for_evidence is not available in the restored DTO;
        // verify related subjects exist via direct field access instead.
        let claim = first.evidence_claims.get(&evidence_id);
        assert!(claim.is_some());
        assert_eq!(claim.unwrap().related_subject_ids.len(), 1);
        Ok(())
    }

    #[test]
    fn snapshot_id_changes_when_slice_seam_sets_change() -> Result<(), String> {
        let baseline = compile_spec_graph(input()?);

        let mut owned = input()?;
        owned
            .implementation_slices
            .first_mut()
            .ok_or_else(|| "expected implementation slice".to_string())?
            .owned_seams
            .push("seam:additional-owned".into());
        assert_ne!(baseline.snapshot_id, compile_spec_graph(owned).snapshot_id);

        let mut shared = input()?;
        shared
            .implementation_slices
            .first_mut()
            .ok_or_else(|| "expected implementation slice".to_string())?
            .shared_seams
            .push("seam:additional-shared".into());
        assert_ne!(baseline.snapshot_id, compile_spec_graph(shared).snapshot_id);

        let mut forbidden = input()?;
        forbidden
            .implementation_slices
            .first_mut()
            .ok_or_else(|| "expected implementation slice".to_string())?
            .forbidden_seams
            .push("seam:additional-forbidden".into());
        assert_ne!(
            baseline.snapshot_id,
            compile_spec_graph(forbidden).snapshot_id
        );

        Ok(())
    }

    #[test]
    fn reports_weak_subject_mapped_as_exact() -> Result<(), String> {
        let mut input = input()?;
        let claim = input
            .evidence_claims
            .first_mut()
            .ok_or_else(|| "expected evidence claim".to_string())?;
        claim.subject_ids = claim.related_subject_ids.clone();
        let graph = compile_spec_graph(input);
        assert!(
            graph.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == GraphDiagnosticCode::ExactSubjectMarkedWeak
            })
        );
        Ok(())
    }

    #[test]
    fn reports_undeclared_and_forbidden_seams() -> Result<(), String> {
        let mut undeclared = input()?;
        let slice = undeclared
            .implementation_slices
            .first_mut()
            .ok_or_else(|| "expected implementation slice".to_string())?;
        slice.owned_seams.clear();
        let graph = compile_spec_graph(undeclared);
        assert!(
            graph.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == GraphDiagnosticCode::SeamNotDeclaredBySlice
            })
        );

        let mut forbidden = input()?;
        let slice = forbidden
            .implementation_slices
            .first_mut()
            .ok_or_else(|| "expected implementation slice".to_string())?;
        slice.forbidden_seams.push("seam:runtime-promotion".into());
        let graph = compile_spec_graph(forbidden);
        assert!(
            graph
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == GraphDiagnosticCode::ForbiddenSeam })
        );
        Ok(())
    }
}
