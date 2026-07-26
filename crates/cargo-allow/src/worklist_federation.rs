use super::{WorkItem, worklist_types::WorkItemLedger};
use allow_policy::federation::FederationDivergenceRecord;

pub(super) fn work_items_from_federation_divergences(
    divergences: &[FederationDivergenceRecord],
    start_index: usize,
) -> Vec<WorkItem> {
    divergences
        .iter()
        .enumerate()
        .map(|(offset, record)| {
            let item_index = start_index + offset;
            WorkItem {
                id: format!("work-mirror-divergence-{item_index:04}"),
                kind: super::worklist_item_kind::MIRROR_DIVERGENCE.to_string(),
                exception_kind: None,
                family: None,
                owner: None,
                classification: None,
                reason: None,
                created: None,
                review_after: None,
                expires: None,
                evidence_count: None,
                selector_precision: None,
                risk: "medium",
                difficulty: super::worklist_priority::DIFFICULTY_SMALL,
                status: allow_core::MatchStatus::Stale,
                allow_id: record.sample_entry_ids.first().cloned(),
                candidate_ids: Vec::new(),
                finding_index: None,
                path: Some(record.mirror_path.clone()),
                line: None,
                column: None,
                evidence_reference: None,
                source_package: None,
                message: record.message.clone(),
                suggested_actions: vec![record.recommended_action.to_string()],
                proof_commands: vec![
                    "cargo-allow doctor".to_string(),
                    "cargo-allow check --mode no-new".to_string(),
                ],
                ledger: WorkItemLedger {
                    ledger_id: Some(record.mirror_ledger_id.clone()),
                    ledger_path: Some(record.mirror_path.clone()),
                    lane: Some("source-exception".to_string()),
                    mode: Some("advisory".to_string()),
                    role: Some("mirror".to_string()),
                },
            }
        })
        .collect()
}
