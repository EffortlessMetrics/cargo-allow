//! Pre-commit objective evaluator — moved from allow-policy to intent-engine (#3521 / #2935 slice 5b).
//!
//! This is the canonical precommit evaluator. It evaluates authored spec-graph
//! evidence against staged change movements and produces typed findings.

use intent_model::{
    CompiledSpecGraph, EvidenceDispositionState, EvidenceSubjectRole, ImplementationClaimStatus,
    ImplementationSeamId, ImplementationSliceClass, ImplementationSliceV1, PrecommitChangeClass,
    PrecommitChangeDeclaration, PrecommitEvaluationInput, PrecommitFinding, PrecommitFindingCode,
    PrecommitFindingPosture, PrecommitInventoryPosture, PrecommitMovement, PrecommitMovementKind,
    PrecommitObjectiveEvaluation, PrecommitSubjectResolution, PrecommitSubjectResolutionStatus,
    RequirementStatus, SupportClaimDispositionState,
};

pub fn evaluate_precommit_objectives(
    input: PrecommitEvaluationInput<'_>,
) -> PrecommitObjectiveEvaluation {
    let mut findings = Vec::new();
    let change_class = classify_change(input.movements, input.declaration.class);
    let posture = if input.legacy_baseline {
        PrecommitFindingPosture::Advisory
    } else {
        PrecommitFindingPosture::Blocking
    };

    if matches!(change_class, PrecommitChangeClass::UnknownOrMixed) {
        push_finding(
            &mut findings,
            PrecommitFindingCode::ChangeClassMissingOrConflicting,
            "change-class",
            posture,
            "the staged candidate has no single explicit or high-confidence change class",
            "declare one current change class and split contradictory changes into separate slices",
        );
        push_finding(
            &mut findings,
            PrecommitFindingCode::UnknownStagedSurface,
            "staged-candidate",
            posture,
            "the staged candidate contains a surface the objective policy cannot classify",
            "name the affected surface or provide a reviewed change declaration",
        );
    }

    if input.inventory != PrecommitInventoryPosture::Complete {
        push_finding(
            &mut findings,
            PrecommitFindingCode::InventoryPartialOrUnsupported,
            "staged-inventory",
            posture,
            "the staged source inventory is partial or contains unsupported entries",
            "repair inventory completeness before treating exact subject obligations as current",
        );
    }

    let selected_slices = selected_slices(input.slices, input.declaration);
    if matches!(
        change_class,
        PrecommitChangeClass::BehaviorChange | PrecommitChangeClass::BugFix
    ) && selected_slices.is_empty()
    {
        push_finding(
            &mut findings,
            PrecommitFindingCode::BehaviorSliceMissing,
            "implementation-slice",
            PrecommitFindingPosture::Blocking,
            "behavior-changing staged code has no current implementation slice",
            "add one current implementation slice and bind it to the affected requirement",
        );
    }

    if matches!(
        change_class,
        PrecommitChangeClass::BehaviorChange
            | PrecommitChangeClass::BugFix
            | PrecommitChangeClass::RefactorNoIntendedBehaviorChange
            | PrecommitChangeClass::SpecOrPolicyChange
    ) {
        for slice in &selected_slices {
            validate_slice(
                input.candidate,
                slice,
                change_class,
                input.subject_resolutions,
                &mut findings,
            );
        }
    }

    for movement in input.movements {
        if movement.kind == PrecommitMovementKind::SubjectBodyIdentityChanged {
            push_finding(
                &mut findings,
                PrecommitFindingCode::TestBodyIdentityStale,
                &movement.id,
                PrecommitFindingPosture::Blocking,
                "an exact test body changed and dependent current evidence must be revalidated",
                "refresh the dependent receipt or evidence disposition for the changed test body",
            );
        }
    }

    match change_class {
        PrecommitChangeClass::BugFix if input.declaration.regression_subject_ids.is_empty() => {
            push_finding(
                &mut findings,
                PrecommitFindingCode::BugFixRegressionMissing,
                "bug-fix",
                PrecommitFindingPosture::Blocking,
                "the bug-fix declaration has no discriminating regression subject",
                "map an exact regression selector that fails on the buggy behavior",
            );
        }
        PrecommitChangeClass::RefactorNoIntendedBehaviorChange
            if !input.declaration.no_intended_behavior_change =>
        {
            push_finding(
                &mut findings,
                PrecommitFindingCode::ChangeClassMissingOrConflicting,
                "refactor",
                PrecommitFindingPosture::Blocking,
                "the refactor class is missing its no-intended-behavior-change declaration",
                "declare the compatibility boundary or classify the change honestly",
            );
        }
        PrecommitChangeClass::RefactorNoIntendedBehaviorChange
            if !input.declaration.evidence_closure_reviewed =>
        {
            push_finding(
                &mut findings,
                PrecommitFindingCode::EvidencePurposeMissing,
                "refactor-evidence",
                PrecommitFindingPosture::Blocking,
                "the refactor has no affected-evidence closure disposition",
                "record whether existing evidence remains sufficient or update the mapping",
            );
        }
        PrecommitChangeClass::GeneratedArtifactChange
            if !input.declaration.generated_source_relation_present =>
        {
            push_finding(
                &mut findings,
                PrecommitFindingCode::GeneratedSourceRelationMissing,
                "generated-source",
                PrecommitFindingPosture::Blocking,
                "generated output changed without an owned source relation",
                "map the generated output to its authoritative source or classify the change differently",
            );
        }
        PrecommitChangeClass::TestOnlyChange => {
            validate_test_only_subjects(input, &mut findings);
        }
        _ => {}
    }

    findings.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then_with(|| left.subject.cmp(&right.subject))
            .then_with(|| left.message.cmp(&right.message))
    });
    PrecommitObjectiveEvaluation {
        change_class,
        findings,
    }
}

fn classify_change(
    movements: &[PrecommitMovement],
    declared: Option<PrecommitChangeClass>,
) -> PrecommitChangeClass {
    if let Some(declared) = declared {
        return declared;
    }
    let mut classes = Vec::new();
    for movement in movements {
        let class = match movement.kind {
            PrecommitMovementKind::RequirementAdded
            | PrecommitMovementKind::RequirementRemoved
            | PrecommitMovementKind::RequirementChanged
            | PrecommitMovementKind::ImplementationSliceAdded
            | PrecommitMovementKind::ImplementationSliceRemoved
            | PrecommitMovementKind::ImplementationSliceChanged
            | PrecommitMovementKind::SeamMappingAdded
            | PrecommitMovementKind::SeamMappingRemoved
            | PrecommitMovementKind::SeamMappingChanged
            | PrecommitMovementKind::EvidencePurposeAdded
            | PrecommitMovementKind::EvidencePurposeRemoved
            | PrecommitMovementKind::EvidencePurposeChanged
            | PrecommitMovementKind::EvidenceClaimChanged => {
                PrecommitChangeClass::SpecOrPolicyChange
            }
            PrecommitMovementKind::SubjectSelectorAdded
            | PrecommitMovementKind::SubjectSelectorRemoved
            | PrecommitMovementKind::SubjectSelectorChanged
            | PrecommitMovementKind::SubjectBodyIdentityChanged => {
                PrecommitChangeClass::TestOnlyChange
            }
            PrecommitMovementKind::GeneratedSourceRelationAdded
            | PrecommitMovementKind::GeneratedSourceRelationRemoved
            | PrecommitMovementKind::GeneratedSourceRelationChanged => {
                PrecommitChangeClass::GeneratedArtifactChange
            }
            PrecommitMovementKind::ProfileOrDialectChanged
            | PrecommitMovementKind::UnknownOrUncomparable => PrecommitChangeClass::UnknownOrMixed,
        };
        if !classes.contains(&class) {
            classes.push(class);
        }
    }
    match classes.as_slice() {
        [class] => *class,
        _ => PrecommitChangeClass::UnknownOrMixed,
    }
}

fn selected_slices<'a>(
    slices: &'a [ImplementationSliceV1],
    declaration: &PrecommitChangeDeclaration,
) -> Vec<&'a ImplementationSliceV1> {
    if declaration.implementation_slice_ids.is_empty() {
        return slices.iter().collect();
    }
    declaration
        .implementation_slice_ids
        .iter()
        .filter_map(|id| slices.iter().find(|slice| slice.id == *id))
        .collect()
}

fn validate_slice(
    candidate: &CompiledSpecGraph,
    slice: &ImplementationSliceV1,
    change_class: PrecommitChangeClass,
    subject_resolutions: &[PrecommitSubjectResolution],
    findings: &mut Vec<PrecommitFinding>,
) {
    for delta in &slice.requirement_delta {
        let Some(requirement) = candidate.requirements.get(&delta.requirement_id) else {
            push_finding(
                findings,
                PrecommitFindingCode::RequirementUnknownOrStale,
                delta.requirement_id.as_str(),
                PrecommitFindingPosture::Blocking,
                "implementation slice references a requirement absent from the staged graph",
                "reference an accepted requirement at its current generation",
            );
            continue;
        };
        if requirement.generation != delta.requirement_generation
            || requirement.status != RequirementStatus::Accepted
        {
            push_finding(
                findings,
                PrecommitFindingCode::RequirementUnknownOrStale,
                delta.requirement_id.as_str(),
                PrecommitFindingPosture::Blocking,
                "implementation slice references a stale or non-accepted requirement generation",
                "refresh the requirement delta or record an explicit normative disposition",
            );
        }
    }

    let slice_node = candidate.slices.get(&slice.id);
    if slice_node.is_none() {
        push_finding(
            findings,
            PrecommitFindingCode::BehaviorSliceMissing,
            slice.id.as_str(),
            PrecommitFindingPosture::Blocking,
            "declared implementation slice is absent from the staged graph",
            "stage the slice source and recompile the candidate graph",
        );
    }

    for seam in slice.owned_seams.iter().chain(slice.shared_seams.iter()) {
        if !candidate
            .seams
            .contains_key(&ImplementationSeamId(seam.clone()))
        {
            push_finding(
                findings,
                PrecommitFindingCode::SeamMissingOrForbidden,
                seam,
                PrecommitFindingPosture::Blocking,
                "implementation slice names a seam that is absent from the staged graph",
                "declare the seam or remove it from the slice mapping",
            );
        }
    }
    for seam in &slice.forbidden_seams {
        if slice.owned_seams.contains(seam) || slice.shared_seams.contains(seam) {
            push_finding(
                findings,
                PrecommitFindingCode::SeamMissingOrForbidden,
                seam,
                PrecommitFindingPosture::Blocking,
                "implementation slice maps the same seam as both allowed and forbidden",
                "resolve the seam ownership conflict before committing",
            );
        }
    }

    let claims = candidate
        .evidence_claims
        .values()
        .filter(|claim| claim.slice_id == slice.id)
        .collect::<Vec<_>>();
    if matches!(
        change_class,
        PrecommitChangeClass::BehaviorChange
            | PrecommitChangeClass::BugFix
            | PrecommitChangeClass::RefactorNoIntendedBehaviorChange
    ) && claims.is_empty()
    {
        push_finding(
            findings,
            PrecommitFindingCode::EvidencePurposeMissing,
            slice.id.as_str(),
            PrecommitFindingPosture::Blocking,
            "implementation slice has no purpose-bearing evidence claim",
            "add positive or forbidden-purpose evidence and its exact subject mapping",
        );
    }
    for claim in claims {
        if claim.subject_ids.is_empty() {
            push_finding(
                findings,
                PrecommitFindingCode::ExactSelectorMissing,
                claim.id.as_str(),
                PrecommitFindingPosture::Blocking,
                "evidence claim has no exact selector",
                "map the claim to an exact staged subject selector",
            );
        }
        for subject_id in &claim.subject_ids {
            if let Some(resolution) = subject_resolutions
                .iter()
                .find(|resolution| resolution.id == *subject_id)
            {
                match resolution.status {
                    PrecommitSubjectResolutionStatus::Ambiguous => push_finding(
                        findings,
                        PrecommitFindingCode::ExactSelectorAmbiguous,
                        subject_id.as_str(),
                        PrecommitFindingPosture::Blocking,
                        "exact evidence selector resolves to multiple staged subjects",
                        "narrow the selector until exactly one staged subject resolves",
                    ),
                    PrecommitSubjectResolutionStatus::Missing => push_finding(
                        findings,
                        PrecommitFindingCode::ExactSelectorMissing,
                        subject_id.as_str(),
                        PrecommitFindingPosture::Blocking,
                        "exact evidence selector does not resolve in the staged candidate",
                        "stage or correct the exact subject mapping",
                    ),
                    PrecommitSubjectResolutionStatus::Partial
                    | PrecommitSubjectResolutionStatus::Unsupported => push_finding(
                        findings,
                        PrecommitFindingCode::InventoryPartialOrUnsupported,
                        subject_id.as_str(),
                        PrecommitFindingPosture::Blocking,
                        "exact evidence selector cannot be resolved from a complete supported inventory",
                        "repair the staged subject inventory before claiming exact evidence",
                    ),
                    PrecommitSubjectResolutionStatus::Exact => {}
                }
            }
            match candidate.subjects.get(subject_id) {
                None => push_finding(
                    findings,
                    PrecommitFindingCode::ExactSelectorMissing,
                    subject_id.as_str(),
                    PrecommitFindingPosture::Blocking,
                    "evidence claim references an exact selector absent from the staged graph",
                    "stage or correct the exact subject mapping",
                ),
                Some(subject) if subject.role != EvidenceSubjectRole::ExactEvidence => {
                    push_finding(
                        findings,
                        PrecommitFindingCode::ExactSelectorMissing,
                        subject_id.as_str(),
                        PrecommitFindingPosture::Blocking,
                        "evidence claim references a subject that is not marked exact",
                        "mark the subject exact or move it to related weak evidence",
                    )
                }
                Some(_) => {}
            }
        }
    }

    let implementation_closed =
        slice.implementation_claim.status == ImplementationClaimStatus::Implemented;
    let evidence_closed = slice.evidence.state == EvidenceDispositionState::Current
        && slice
            .evidence
            .receipt
            .as_deref()
            .is_some_and(|receipt| !receipt.trim().is_empty());
    if slice.change_class == ImplementationSliceClass::SpecOrPolicyChange && implementation_closed {
        push_finding(
            findings,
            PrecommitFindingCode::SpecOnlyRuntimePromotion,
            slice.id.as_str(),
            PrecommitFindingPosture::Blocking,
            "spec or policy slice publishes an implemented runtime claim",
            "leave implementation outstanding or move runtime completion to a compatible behavior slice",
        );
    }
    if implementation_closed && !evidence_closed {
        push_finding(
            findings,
            PrecommitFindingCode::ImplementationClaimInvalid,
            slice.id.as_str(),
            PrecommitFindingPosture::Blocking,
            "implemented claim has no current receipt-backed evidence closure",
            "add current receipt-backed evidence or lower the implementation claim",
        );
    }
    if slice.support_claim.state == SupportClaimDispositionState::Promoted
        && (!implementation_closed || !evidence_closed || !named_support_claim(slice))
    {
        push_finding(
            findings,
            PrecommitFindingCode::SupportClaimAheadOfClosure,
            slice.id.as_str(),
            PrecommitFindingPosture::Blocking,
            "support promotion is ahead of implementation and evidence closure",
            "complete implementation, receipt-backed evidence, and the named support claim first",
        );
    }
}

fn named_support_claim(slice: &ImplementationSliceV1) -> bool {
    slice
        .support_claim
        .claim
        .as_deref()
        .is_some_and(|claim| !claim.trim().is_empty())
}

fn validate_test_only_subjects(
    input: PrecommitEvaluationInput<'_>,
    findings: &mut Vec<PrecommitFinding>,
) {
    let changed = input
        .declaration
        .changed_subject_ids
        .iter()
        .map(|id| id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for id in changed {
        let owned = input.candidate.evidence_claims.values().any(|claim| {
            claim
                .subject_ids
                .iter()
                .any(|subject_id| subject_id.as_str() == id)
                || claim
                    .related_subject_ids
                    .iter()
                    .any(|subject_id| subject_id.as_str() == id)
        });
        if !owned {
            push_finding(
                findings,
                PrecommitFindingCode::TestOnlySubjectUnowned,
                id,
                PrecommitFindingPosture::Blocking,
                "test-only subject is not owned by an evidence claim",
                "attach the subject to an existing claim or provide an explicit gap-closing purpose",
            );
        }
    }
}

fn push_finding(
    findings: &mut Vec<PrecommitFinding>,
    code: PrecommitFindingCode,
    subject: impl Into<String>,
    posture: PrecommitFindingPosture,
    message: impl Into<String>,
    action: impl Into<String>,
) {
    findings.push(PrecommitFinding {
        code,
        subject: subject.into(),
        posture,
        message: message.into(),
        action: action.into(),
    });
}

#[cfg(test)]
mod tests {
    use super::{
        PrecommitChangeClass, PrecommitChangeDeclaration, PrecommitEvaluationInput,
        PrecommitFindingCode, PrecommitInventoryPosture, PrecommitMovement, PrecommitMovementKind,
        evaluate_precommit_objectives,
    };
    use intent_model::{
        CompiledSpecGraph, GraphCompileInput, ImplementationSeamId, ImplementationSeamRegistration,
        ImplementationSliceClass, ImplementationSliceV1, SourceLocation,
        parse_implementation_slice, parse_requirement_blocks,
    };

    const SPEC: &str = r#"---
id: CARGO-ALLOW-SPEC-2361
kind: spec
---

```toml cargo-allow-requirements
schema_version = "1.0"
[[requirement]]
id = "behavior"
generation = 1
status = "accepted"
statement = "The behavior is explicit."
claim_class = "runtime_behavior"
```
"#;

    const SLICE: &str = r#"
schema_version = "2.0"
id = "slice.behavior"
generation = 1
source_issue = "issue:2361"
design_reference = "design:2361"
change_class = "behavior_change"
claim_boundary = "behavior"

[[requirement_delta]]
requirement_id = "CARGO-ALLOW-SPEC-2361#behavior"
requirement_generation = 1

[implementation_claim]
status = "outstanding"

[evidence]
state = "outstanding"

[support_claim]
state = "unchanged"
"#;

    fn graph() -> Result<(CompiledSpecGraph, ImplementationSliceV1), String> {
        let requirements = parse_requirement_blocks(SPEC).map_err(|e| e.to_string())?;
        let slice = parse_implementation_slice(SLICE).map_err(|e| e.to_string())?;
        let graph = crate::compile_spec_graph(GraphCompileInput {
            requirement_graphs: vec![requirements],
            implementation_slices: vec![slice.clone()],
            seams: vec![ImplementationSeamRegistration {
                id: ImplementationSeamId("seam:behavior".into()),
                owner: "owner".into(),
                operation: "change".into(),
                source: SourceLocation::new("src/lib.rs"),
            }],
            evidence_claims: Vec::new(),
            subjects: Vec::new(),
        });
        Ok((graph, slice))
    }

    fn input<'a>(
        graph: &'a CompiledSpecGraph,
        slice: &'a ImplementationSliceV1,
        declaration: &'a PrecommitChangeDeclaration,
        movements: &'a [PrecommitMovement],
    ) -> PrecommitEvaluationInput<'a> {
        PrecommitEvaluationInput {
            candidate: graph,
            slices: std::slice::from_ref(slice),
            movements,
            declaration,
            subject_resolutions: &[],
            inventory: PrecommitInventoryPosture::Complete,
            legacy_baseline: false,
        }
    }

    #[test]
    fn classifies_explicit_behavior_without_inventing_file_semantics() -> Result<(), String> {
        let (graph, slice) = graph()?;
        let declaration = PrecommitChangeDeclaration {
            class: Some(PrecommitChangeClass::BehaviorChange),
            ..Default::default()
        };
        let result = evaluate_precommit_objectives(input(&graph, &slice, &declaration, &[]));
        assert_eq!(result.change_class, PrecommitChangeClass::BehaviorChange);
        assert!(
            result
                .findings
                .iter()
                .any(|finding| { finding.code == PrecommitFindingCode::EvidencePurposeMissing })
        );
        Ok(())
    }

    #[test]
    fn spec_only_cannot_publish_implemented_runtime_claim() -> Result<(), String> {
        let (graph, mut slice) = graph()?;
        slice.change_class = ImplementationSliceClass::SpecOrPolicyChange;
        slice.implementation_claim.status = super::ImplementationClaimStatus::Implemented;
        let declaration = PrecommitChangeDeclaration {
            class: Some(PrecommitChangeClass::SpecOrPolicyChange),
            ..Default::default()
        };
        let result = evaluate_precommit_objectives(input(&graph, &slice, &declaration, &[]));
        assert!(
            result
                .findings
                .iter()
                .any(|finding| finding.code == PrecommitFindingCode::SpecOnlyRuntimePromotion)
        );
        Ok(())
    }

    #[test]
    fn docs_and_mechanical_changes_remain_proportionate() -> Result<(), String> {
        let (graph, slice) = graph()?;
        for class in [
            PrecommitChangeClass::DocsOnly,
            PrecommitChangeClass::Mechanical,
        ] {
            let declaration = PrecommitChangeDeclaration {
                class: Some(class),
                ..Default::default()
            };
            let result = evaluate_precommit_objectives(input(&graph, &slice, &declaration, &[]));
            assert!(
                result.findings.is_empty(),
                "unexpected findings for {class:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn unclassified_movement_is_conservative_and_deterministic() -> Result<(), String> {
        let (graph, slice) = graph()?;
        let movements = [PrecommitMovement {
            kind: PrecommitMovementKind::UnknownOrUncomparable,
            id: "surface".into(),
        }];
        let declaration = PrecommitChangeDeclaration::default();
        let result = evaluate_precommit_objectives(input(&graph, &slice, &declaration, &movements));
        assert_eq!(result.change_class, PrecommitChangeClass::UnknownOrMixed);
        assert_eq!(
            result.findings.first().map(|finding| finding.code),
            Some(PrecommitFindingCode::ChangeClassMissingOrConflicting)
        );
        Ok(())
    }

    #[test]
    fn precommit_partial_stage_corpus() -> Result<(), String> {
        let (graph, slice) = graph()?;
        let behavior = PrecommitChangeDeclaration {
            class: Some(PrecommitChangeClass::BehaviorChange),
            ..Default::default()
        };
        let missing_slice = evaluate_precommit_objectives(PrecommitEvaluationInput {
            candidate: &graph,
            slices: &[],
            movements: &[],
            declaration: &behavior,
            subject_resolutions: &[],
            inventory: PrecommitInventoryPosture::Complete,
            legacy_baseline: false,
        });
        if !missing_slice
            .findings
            .iter()
            .any(|finding| finding.code == PrecommitFindingCode::BehaviorSliceMissing)
        {
            return Err("partial candidate without its implementation slice passed".to_string());
        }

        let bug_fix = PrecommitChangeDeclaration {
            class: Some(PrecommitChangeClass::BugFix),
            ..Default::default()
        };
        let missing_regression =
            evaluate_precommit_objectives(input(&graph, &slice, &bug_fix, &[]));
        if !missing_regression
            .findings
            .iter()
            .any(|finding| finding.code == PrecommitFindingCode::BugFixRegressionMissing)
        {
            return Err("bug-fix candidate without an exact regression subject passed".to_string());
        }

        let stale_subject = [PrecommitMovement {
            kind: PrecommitMovementKind::SubjectBodyIdentityChanged,
            id: "subject:negative".to_string(),
        }];
        let stale_evidence =
            evaluate_precommit_objectives(input(&graph, &slice, &behavior, &stale_subject));
        if !stale_evidence
            .findings
            .iter()
            .any(|finding| finding.code == PrecommitFindingCode::TestBodyIdentityStale)
        {
            return Err("changed test body did not invalidate current evidence".to_string());
        }

        let partial_inventory = evaluate_precommit_objectives(PrecommitEvaluationInput {
            candidate: &graph,
            slices: std::slice::from_ref(&slice),
            movements: &[],
            declaration: &PrecommitChangeDeclaration {
                class: Some(PrecommitChangeClass::DocsOnly),
                ..Default::default()
            },
            subject_resolutions: &[],
            inventory: PrecommitInventoryPosture::Partial,
            legacy_baseline: false,
        });
        if !partial_inventory
            .findings
            .iter()
            .any(|finding| finding.code == PrecommitFindingCode::InventoryPartialOrUnsupported)
        {
            return Err("partial inventory was treated as a clean candidate".to_string());
        }
        Ok(())
    }
}
