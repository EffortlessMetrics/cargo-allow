//! `change status --staged --phase precommit` vertical (#2599-B).

use crate::change::report::{
    CHANGE_STATUS_CLAIM_BOUNDARY, CHANGE_STATUS_SCHEMA_ID, ChangeStatusReportV1, StagedChangeV1,
};
use crate::config::IntentConfigV1;
use crate::exit::{ProcessExitFamilyV1, exit_family_for_result_class};
use crate::render::{OutputFormat, emit_frame};
use effortless_repo_snapshot::{
    StagedPathStatus, StagedRepositorySnapshot, StagedSnapshotCompleteness,
    staged_repository_snapshot,
};
use intent_engine::{
    GraphMovementKindV1, GraphMovementV1, InventoryPostureV1, ObligationPostureV1,
    PRECOMMIT_PHASE_ID, PhaseObligationCompileInputV1, PhaseObligationItemV1,
    PhaseObligationKindV1, PhaseObligationPlanV1, compile_phase_obligation_plan,
};
use intent_protocol::{
    IntentArtifactKindV1, IntentIdentityEnvelopeV1, IntentObligationPlanEnvelopeV1,
    IntentObligationPlanResponseV1, IntentObligationPostureV1, IntentPhaseObligationKindV1,
    IntentPhaseObligationV1, IntentViewEnvelopeV1, IntentViewKindV1, IntentViewResponseV1,
    RepositorySnapshotV1, ResolvedRevisionV1, ResultClassV1,
};
use std::path::Path;

pub fn change_status_staged_precommit(
    root: &Path,
    config: &IntentConfigV1,
    format: OutputFormat,
    analysis_receipt: bool,
) -> Result<ProcessExitFamilyV1, String> {
    let snapshot = match staged_repository_snapshot(root) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            let message = error.to_string();
            let result_class = classify_snapshot_error(&message);
            return emit_error_report(config, format, result_class, message);
        }
    };
    let report = build_report(config, &snapshot)?;
    let family = exit_family_for_result_class(&report.result_class);
    let rendered = if analysis_receipt && matches!(format, OutputFormat::Json) {
        let identity = staged_identity_envelope(&snapshot);
        let envelope = crate::transport::wrap_change_status_report(&report, &identity.snapshot)?;
        serde_json::to_string_pretty(&envelope)
            .map_err(|err| format!("serialize analysis receipt envelope: {err}"))?
    } else {
        emit_frame(&report, format)?
    };
    print!("{rendered}");
    Ok(family)
}

fn build_report(
    config: &IntentConfigV1,
    snapshot: &StagedRepositorySnapshot,
) -> Result<ChangeStatusReportV1, String> {
    let inventory = inventory_posture(snapshot.completeness);
    let movements = movements_for_snapshot(snapshot);
    let plan = compile_phase_obligation_plan(&PhaseObligationCompileInputV1 {
        phase: PRECOMMIT_PHASE_ID.to_string(),
        movements,
        inventory,
        legacy_baseline: false,
    });
    let unmapped_staged_surface = !snapshot.changes.is_empty();
    let result_class = result_class_for(snapshot, &plan, unmapped_staged_surface);
    let identity = staged_identity_envelope(snapshot);
    let obligation_plan = protocol_obligation_response(&plan, identity.clone(), result_class);
    let staged_view = staged_view_response(snapshot, result_class);
    Ok(ChangeStatusReportV1 {
        schema_id: CHANGE_STATUS_SCHEMA_ID.to_string(),
        command: "change status".to_string(),
        phase: PRECOMMIT_PHASE_ID.to_string(),
        profile: config.profile.as_str().to_string(),
        staged_identity: snapshot.identity.semantic_hash.clone(),
        staged_changes: snapshot.changes.iter().map(staged_change).collect(),
        inventory_completeness: completeness_label(snapshot.completeness),
        staged_view,
        obligation_plan,
        unmapped_staged_surface,
        result_class: result_class.as_str().to_string(),
        process_exit_family: exit_family_for_result_class(result_class.as_str())
            .as_str()
            .to_string(),
        claim_boundary: CHANGE_STATUS_CLAIM_BOUNDARY.to_string(),
    })
}

fn emit_error_report(
    config: &IntentConfigV1,
    format: OutputFormat,
    result_class: ResultClassV1,
    message: String,
) -> Result<ProcessExitFamilyV1, String> {
    let identity = placeholder_identity();
    let empty_plan = PhaseObligationPlanV1::new(PRECOMMIT_PHASE_ID, Vec::new());
    let obligation_plan = protocol_obligation_response(&empty_plan, identity.clone(), result_class);
    let report = ChangeStatusReportV1 {
        schema_id: CHANGE_STATUS_SCHEMA_ID.to_string(),
        command: "change status".to_string(),
        phase: PRECOMMIT_PHASE_ID.to_string(),
        profile: config.profile.as_str().to_string(),
        staged_identity: String::new(),
        staged_changes: Vec::new(),
        inventory_completeness: "unknown".to_string(),
        staged_view: IntentViewResponseV1::new(
            IntentViewEnvelopeV1::new(
                placeholder_identity().snapshot,
                IntentViewKindV1::StagedIndex,
                "unavailable",
            ),
            result_class,
            0,
        ),
        obligation_plan,
        unmapped_staged_surface: false,
        result_class: result_class.as_str().to_string(),
        process_exit_family: exit_family_for_result_class(result_class.as_str())
            .as_str()
            .to_string(),
        claim_boundary: format!("{CHANGE_STATUS_CLAIM_BOUNDARY} error: {message}"),
    };
    let family = exit_family_for_result_class(result_class.as_str());
    let rendered = emit_frame(&report, format)?;
    print!("{rendered}");
    Ok(family)
}

fn classify_snapshot_error(message: &str) -> ResultClassV1 {
    if message.contains("staged_index_changed") {
        ResultClassV1::StaleInput
    } else if message.contains("not a git repository") || message.contains("Usage") {
        ResultClassV1::MalformedInput
    } else {
        ResultClassV1::InstrumentFailure
    }
}

fn result_class_for(
    snapshot: &StagedRepositorySnapshot,
    plan: &PhaseObligationPlanV1,
    unmapped_staged_surface: bool,
) -> ResultClassV1 {
    if unmapped_staged_surface {
        return ResultClassV1::Findings;
    }
    if snapshot.completeness != StagedSnapshotCompleteness::Complete {
        return ResultClassV1::Findings;
    }
    if plan
        .obligations
        .iter()
        .any(|item| item.posture == ObligationPostureV1::Blocking)
    {
        return ResultClassV1::Findings;
    }
    ResultClassV1::Completed
}

fn movements_for_snapshot(snapshot: &StagedRepositorySnapshot) -> Vec<GraphMovementV1> {
    if snapshot.changes.is_empty() {
        return Vec::new();
    }
    vec![GraphMovementV1 {
        kind: GraphMovementKindV1::UnknownOrUncomparable,
        id: "staged-candidate".to_string(),
    }]
}

fn inventory_posture(completeness: StagedSnapshotCompleteness) -> InventoryPostureV1 {
    match completeness {
        StagedSnapshotCompleteness::Complete => InventoryPostureV1::Complete,
        StagedSnapshotCompleteness::Partial => InventoryPostureV1::Partial,
    }
}

fn completeness_label(completeness: StagedSnapshotCompleteness) -> String {
    match completeness {
        StagedSnapshotCompleteness::Complete => "complete".to_string(),
        StagedSnapshotCompleteness::Partial => "partial".to_string(),
    }
}

fn staged_change(change: &effortless_repo_snapshot::StagedPathChange) -> StagedChangeV1 {
    StagedChangeV1 {
        status: staged_status_label(change.status),
        path: change
            .path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        previous_path: change
            .previous_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
    }
}

fn staged_status_label(status: StagedPathStatus) -> String {
    match status {
        StagedPathStatus::Added => "Added",
        StagedPathStatus::Modified => "Modified",
        StagedPathStatus::Deleted => "Deleted",
        StagedPathStatus::Renamed => "Renamed",
        StagedPathStatus::Copied => "Copied",
        StagedPathStatus::TypeChanged => "TypeChanged",
        StagedPathStatus::Unmerged => "Unmerged",
        StagedPathStatus::Unknown => "Unknown",
    }
    .to_string()
}

fn staged_view_response(
    snapshot: &StagedRepositorySnapshot,
    result_class: ResultClassV1,
) -> IntentViewResponseV1 {
    let identity = staged_identity_envelope(snapshot);
    let view = IntentViewEnvelopeV1::new(
        identity.snapshot.clone(),
        IntentViewKindV1::StagedIndex,
        snapshot.identity.semantic_hash.clone(),
    );
    IntentViewResponseV1::new(view, result_class, snapshot.entries.len() as u32)
}

fn staged_identity_envelope(snapshot: &StagedRepositorySnapshot) -> IntentIdentityEnvelopeV1 {
    let commit = snapshot
        .parent_commit
        .clone()
        .unwrap_or_else(|| "0000000000000000000000000000000000000000".to_string());
    let snapshot_transport = RepositorySnapshotV1::new_committed_head(
        snapshot.identity.semantic_hash.clone(),
        "sha1",
        ResolvedRevisionV1 {
            requested: "HEAD".to_string(),
            commit,
            tree: String::new(),
        },
    );
    IntentIdentityEnvelopeV1::new(
        snapshot_transport,
        IntentArtifactKindV1::SpecSystemConfig,
        "staged-candidate",
        "staged-index",
        snapshot.identity.semantic_hash.clone(),
    )
}

fn placeholder_identity() -> IntentIdentityEnvelopeV1 {
    IntentIdentityEnvelopeV1::new(
        RepositorySnapshotV1::new_committed_head(
            "unavailable",
            "sha1",
            ResolvedRevisionV1 {
                requested: "HEAD".to_string(),
                commit: "0000000000000000000000000000000000000000".to_string(),
                tree: String::new(),
            },
        ),
        IntentArtifactKindV1::SpecSystemConfig,
        "staged-candidate",
        "staged-index",
        "unavailable",
    )
}

fn protocol_obligation_response(
    plan: &PhaseObligationPlanV1,
    identity: IntentIdentityEnvelopeV1,
    result_class: ResultClassV1,
) -> IntentObligationPlanResponseV1 {
    let obligations = plan
        .obligations
        .iter()
        .map(engine_obligation_to_protocol)
        .collect::<Vec<_>>();
    let open_obligation_count = obligations
        .iter()
        .filter(|item| item.posture == IntentObligationPostureV1::Blocking)
        .count() as u32;
    let envelope = IntentObligationPlanEnvelopeV1::new(identity, plan.phase.clone(), obligations);
    IntentObligationPlanResponseV1::new(envelope, result_class, open_obligation_count)
}

fn engine_obligation_to_protocol(item: &PhaseObligationItemV1) -> IntentPhaseObligationV1 {
    IntentPhaseObligationV1 {
        obligation_id: item.obligation_id.clone(),
        phase: item.phase.clone(),
        kind: match item.kind {
            PhaseObligationKindV1::EvidenceReview => IntentPhaseObligationKindV1::EvidenceReview,
            PhaseObligationKindV1::ImplementationClosure => {
                IntentPhaseObligationKindV1::ImplementationClosure
            }
            PhaseObligationKindV1::SupportClaimReview => {
                IntentPhaseObligationKindV1::SupportClaimReview
            }
            PhaseObligationKindV1::InventoryCompleteness => {
                IntentPhaseObligationKindV1::InventoryCompleteness
            }
            PhaseObligationKindV1::SubjectResolution => {
                IntentPhaseObligationKindV1::SubjectResolution
            }
            PhaseObligationKindV1::PolicyAlignment => IntentPhaseObligationKindV1::PolicyAlignment,
        },
        statement: item.statement.clone(),
        posture: match item.posture {
            ObligationPostureV1::Blocking => IntentObligationPostureV1::Blocking,
            ObligationPostureV1::Advisory => IntentObligationPostureV1::Advisory,
        },
        evidence_refs: Vec::new(),
        // PR B (#3964) wires the #3819 evaluator rows into this field; the
        // current producer predates the enrichment and honestly emits none.
        handoff: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use intent_engine::load_precommit_obligation_plan_fixture;
    use std::path::PathBuf;

    #[test]
    fn unmapped_staged_surface_compiles_policy_alignment_obligation() -> Result<(), String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let fixture = load_precommit_obligation_plan_fixture(&root)?;
        let movements = vec![GraphMovementV1 {
            kind: GraphMovementKindV1::UnknownOrUncomparable,
            id: "staged-candidate".to_string(),
        }];
        let plan = compile_phase_obligation_plan(&PhaseObligationCompileInputV1 {
            phase: PRECOMMIT_PHASE_ID.to_string(),
            movements,
            inventory: InventoryPostureV1::Complete,
            legacy_baseline: false,
        });
        if plan.phase != fixture.phase {
            return Err("phase mismatch".to_string());
        }
        if !plan.obligations.iter().any(|item| {
            item.kind == PhaseObligationKindV1::PolicyAlignment
                && item.posture == ObligationPostureV1::Blocking
        }) {
            return Err("missing policy alignment obligation".to_string());
        }
        Ok(())
    }

    #[test]
    fn unmapped_staged_surface_yields_findings() -> Result<(), String> {
        let snapshot = staged_repository_snapshot(Path::new(env!("CARGO_MANIFEST_DIR")))
            .map_err(|error| format!("manifest dir should be a git worktree: {error}"))?;
        let result = result_class_for(
            &snapshot,
            &PhaseObligationPlanV1::new(PRECOMMIT_PHASE_ID, Vec::new()),
            true,
        );
        if result != ResultClassV1::Findings {
            return Err("expected findings for unmapped staged surface".to_string());
        }
        Ok(())
    }
}
