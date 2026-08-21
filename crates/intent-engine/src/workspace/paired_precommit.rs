//! Paired precommit evaluation over canonical movement and diagnostic facts
//! (#3523 slice D step i).
//!
//! cargo-allow's workspace orchestrator keeps its local adapter over the
//! legacy allow-policy copies under the promotion switch; this module is
//! the canonical engine-side counterpart. It derives the evaluator input
//! from canonical paired-comparison facts — movements
//! ([`GraphMovementV1`]) and compile diagnostics
//! ([`GraphDiagnosticV1`]) — and runs the canonical precommit evaluator.
//! The cargo-allow mirror stays string-compatible; the dev-scope parity
//! tests bind the two derivations together until the cutover.

use crate::graph_comparison::{GraphMovementKindV1, GraphMovementV1};
use crate::precommit_evaluator::evaluate_precommit_objectives;
use intent_model::{
    CompiledSpecGraph, EvidenceSubjectId, ImplementationSliceV1, PrecommitChangeDeclaration,
    PrecommitEvaluationInput, PrecommitInventoryPosture, PrecommitMovement, PrecommitMovementKind,
    PrecommitObjectiveEvaluation, PrecommitSubjectResolution, PrecommitSubjectResolutionStatus,
};
use serde::{Deserialize, Serialize};

/// A normalized compile diagnostic from a paired graph compilation.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphDiagnosticV1 {
    pub code: String,
    pub subject: String,
    pub message: String,
}

/// Canonical mapping from graph-comparison movement kinds to the precommit
/// movement vocabulary. The three `GeneratedSourceRelation*` precommit
/// kinds have no graph-comparison producer yet and are unreachable here.
pub fn graph_movement_kind_to_precommit(kind: GraphMovementKindV1) -> PrecommitMovementKind {
    match kind {
        GraphMovementKindV1::RequirementAdded => PrecommitMovementKind::RequirementAdded,
        GraphMovementKindV1::RequirementRemoved => PrecommitMovementKind::RequirementRemoved,
        GraphMovementKindV1::RequirementChanged => PrecommitMovementKind::RequirementChanged,
        GraphMovementKindV1::ImplementationSliceAdded => {
            PrecommitMovementKind::ImplementationSliceAdded
        }
        GraphMovementKindV1::ImplementationSliceRemoved => {
            PrecommitMovementKind::ImplementationSliceRemoved
        }
        GraphMovementKindV1::ImplementationSliceChanged => {
            PrecommitMovementKind::ImplementationSliceChanged
        }
        GraphMovementKindV1::SeamMappingAdded => PrecommitMovementKind::SeamMappingAdded,
        GraphMovementKindV1::SeamMappingRemoved => PrecommitMovementKind::SeamMappingRemoved,
        GraphMovementKindV1::SeamMappingChanged => PrecommitMovementKind::SeamMappingChanged,
        GraphMovementKindV1::EvidencePurposeAdded => PrecommitMovementKind::EvidencePurposeAdded,
        GraphMovementKindV1::EvidencePurposeRemoved => {
            PrecommitMovementKind::EvidencePurposeRemoved
        }
        GraphMovementKindV1::EvidencePurposeChanged => {
            PrecommitMovementKind::EvidencePurposeChanged
        }
        GraphMovementKindV1::EvidenceClaimChanged => PrecommitMovementKind::EvidenceClaimChanged,
        GraphMovementKindV1::SubjectSelectorAdded => PrecommitMovementKind::SubjectSelectorAdded,
        GraphMovementKindV1::SubjectSelectorRemoved => {
            PrecommitMovementKind::SubjectSelectorRemoved
        }
        GraphMovementKindV1::SubjectSelectorChanged => {
            PrecommitMovementKind::SubjectSelectorChanged
        }
        GraphMovementKindV1::SubjectBodyIdentityChanged => {
            PrecommitMovementKind::SubjectBodyIdentityChanged
        }
        GraphMovementKindV1::ProfileOrDialectChanged => {
            PrecommitMovementKind::ProfileOrDialectChanged
        }
        GraphMovementKindV1::UnknownOrUncomparable => PrecommitMovementKind::UnknownOrUncomparable,
    }
}

fn movement_touches_subject(kind: GraphMovementKindV1) -> bool {
    matches!(
        kind,
        GraphMovementKindV1::SubjectSelectorAdded
            | GraphMovementKindV1::SubjectSelectorRemoved
            | GraphMovementKindV1::SubjectSelectorChanged
            | GraphMovementKindV1::SubjectBodyIdentityChanged
    )
}

/// Derive a subject resolution from a compile diagnostic code, when the code
/// carries resolution semantics.
pub fn subject_resolution_from_diagnostic(
    diagnostic: &GraphDiagnosticV1,
) -> Option<PrecommitSubjectResolution> {
    let status = match diagnostic.code.as_str() {
        "spec_graph_selector_ambiguous" => PrecommitSubjectResolutionStatus::Ambiguous,
        "spec_graph_selector_not_found" => PrecommitSubjectResolutionStatus::Missing,
        "spec_graph_rust_inventory_partial" => PrecommitSubjectResolutionStatus::Partial,
        "spec_graph_subject_non_executable"
        | "spec_graph_subject_generated_or_parameterized"
        | "spec_graph_selector_malformed"
        | "spec_graph_selector_cfg_or_feature_unknown" => {
            PrecommitSubjectResolutionStatus::Unsupported
        }
        _ => return None,
    };
    Some(PrecommitSubjectResolution {
        id: EvidenceSubjectId(diagnostic.subject.clone()),
        status,
    })
}

/// Evaluate a paired parent/candidate compilation through the canonical
/// precommit evaluator.
///
/// Pure over the given facts: the caller (cargo-intent, or cargo-allow's
/// mirror adapter during the parity window) has already compiled both
/// graphs, derived the file/test inventory posture, and resolved revision
/// identities. When the declaration leaves `changed_subject_ids` empty,
/// subject-selector and subject-body movements infer them, mirroring the
/// legacy adapter.
pub fn evaluate_paired_precommit_objectives_v1(
    candidate_graph: &CompiledSpecGraph,
    candidate_slice: &ImplementationSliceV1,
    movements: &[GraphMovementV1],
    declaration: &PrecommitChangeDeclaration,
    diagnostics: &[GraphDiagnosticV1],
    inventory: PrecommitInventoryPosture,
    legacy_baseline: bool,
) -> PrecommitObjectiveEvaluation {
    let precommit_movements = movements
        .iter()
        .map(|movement| PrecommitMovement {
            kind: graph_movement_kind_to_precommit(movement.kind),
            id: movement.id.clone(),
        })
        .collect::<Vec<_>>();
    let mut declaration = declaration.clone();
    if declaration.changed_subject_ids.is_empty() {
        declaration.changed_subject_ids = movements
            .iter()
            .filter(|movement| movement_touches_subject(movement.kind))
            .map(|movement| EvidenceSubjectId(movement.id.clone()))
            .collect();
    }
    let subject_resolutions = diagnostics
        .iter()
        .filter_map(subject_resolution_from_diagnostic)
        .collect::<Vec<_>>();
    evaluate_precommit_objectives(PrecommitEvaluationInput {
        candidate: candidate_graph,
        slices: std::slice::from_ref(candidate_slice),
        movements: &precommit_movements,
        declaration: &declaration,
        subject_resolutions: &subject_resolutions,
        inventory,
        legacy_baseline,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_comparison::GraphMovementV1;
    use intent_model::{EvidenceSubjectId, PrecommitChangeClass, PrecommitChangeDeclaration};

    fn subject_movements() -> [GraphMovementV1; 2] {
        [
            GraphMovementV1 {
                kind: GraphMovementKindV1::SubjectSelectorAdded,
                id: "tests::suite_a".to_string(),
            },
            GraphMovementV1 {
                kind: GraphMovementKindV1::SubjectBodyIdentityChanged,
                id: "tests::suite_b".to_string(),
            },
        ]
    }

    fn declaration() -> PrecommitChangeDeclaration {
        PrecommitChangeDeclaration {
            class: Some(PrecommitChangeClass::TestOnlyChange),
            ..Default::default()
        }
    }

    #[test]
    fn inferred_changed_subjects_match_explicit_subjects() -> Result<(), String> {
        let graph = crate::graph_compiler::compile_spec_graph(Default::default());
        let slice = fixture_slice()?;
        let movements = subject_movements();
        let inferred = evaluate_paired_precommit_objectives_v1(
            &graph,
            &slice,
            &movements,
            &declaration(),
            &[],
            PrecommitInventoryPosture::Complete,
            true,
        );
        let mut explicit_declaration = declaration();
        explicit_declaration.changed_subject_ids = movements
            .iter()
            .map(|movement| EvidenceSubjectId(movement.id.clone()))
            .collect();
        let explicit = evaluate_paired_precommit_objectives_v1(
            &graph,
            &slice,
            &movements,
            &explicit_declaration,
            &[],
            PrecommitInventoryPosture::Complete,
            true,
        );
        assert_eq!(inferred, explicit);
        Ok(())
    }

    #[test]
    fn explicit_changed_subjects_are_not_overridden_by_inference() -> Result<(), String> {
        let graph = crate::graph_compiler::compile_spec_graph(Default::default());
        let slice = fixture_slice()?;
        let movements = subject_movements();
        let mut declaration = declaration();
        declaration.changed_subject_ids = vec![EvidenceSubjectId("tests::other".to_string())];
        let evaluated = evaluate_paired_precommit_objectives_v1(
            &graph,
            &slice,
            &movements,
            &declaration,
            &[],
            PrecommitInventoryPosture::Complete,
            true,
        );
        // The explicit non-movement subject is preserved: it is flagged as
        // unowned, and the movement subjects are not substituted into the
        // declared subject set (they surface only through their own
        // movement findings).
        let unowned_subjects: Vec<&str> = evaluated
            .findings
            .iter()
            .filter(|finding| {
                finding.code == intent_model::PrecommitFindingCode::TestOnlySubjectUnowned
            })
            .map(|finding| finding.subject.as_str())
            .collect();
        assert_eq!(unowned_subjects, vec!["tests::other"]);
        Ok(())
    }

    fn fixture_slice() -> Result<intent_model::ImplementationSliceV1, String> {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.allow/spec-system/slices/self-hosted-runtime-promotion-v1.toml");
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("read slice fixture: {error}"))?;
        intent_model::parse_implementation_slice(&text)
            .map_err(|error| format!("parse slice fixture: {error}"))
    }
}
