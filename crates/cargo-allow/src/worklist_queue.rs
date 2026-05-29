use super::worklist_item_kind::{BASELINE_DEBT, BROAD_SCOPE};
use super::worklist_priority::{
    DIFFICULTY_MEDIUM, DIFFICULTY_SMALL, RISK_HIGH, RISK_LOW, RISK_MEDIUM,
};
use super::{WorkItem, WorklistFilters};
use allow_core::{MatchStatus, source_tree_path_matches_filter};

pub(super) fn filter_work_items(
    items: Vec<WorkItem>,
    filters: WorklistFilters<'_>,
) -> Vec<WorkItem> {
    items
        .into_iter()
        .filter(|item| {
            filters
                .family
                .map(|family| item.family.as_deref() == Some(family))
                .unwrap_or(true)
                && filters
                    .item_kind
                    .map(|item_kind| item.kind == item_kind)
                    .unwrap_or(true)
                && filters
                    .status
                    .map(|status| item.status.as_str() == status)
                    .unwrap_or(true)
                && filters
                    .allow_id
                    .map(|allow_id| item.allow_id.as_deref() == Some(allow_id))
                    .unwrap_or(true)
                && filters
                    .path
                    .map(|path| {
                        item.path
                            .as_deref()
                            .map(|item_path| source_tree_path_matches_filter(item_path, path))
                            .unwrap_or(false)
                    })
                    .unwrap_or(true)
                && filters
                    .source_package
                    .map(|source_package| item.source_package.as_deref() == Some(source_package))
                    .unwrap_or(true)
                && filters
                    .owner
                    .map(|owner| item.owner.as_deref() == Some(owner))
                    .unwrap_or(true)
                && filters
                    .classification
                    .map(|classification| item.classification.as_deref() == Some(classification))
                    .unwrap_or(true)
                && (!filters.baseline_debt
                    || item.kind == BASELINE_DEBT
                    || item.classification.as_deref() == Some("baseline_debt")
                    || item.status == MatchStatus::BaselineDebt)
                && (!filters.broad_scope || item.kind == BROAD_SCOPE)
                && filters.risk.map(|risk| item.risk == risk).unwrap_or(true)
                && filters
                    .difficulty
                    .map(|difficulty| item.difficulty == difficulty)
                    .unwrap_or(true)
                && (!filters.missing_evidence || item.evidence_count == Some(0))
        })
        .collect()
}

pub(super) fn sort_work_items(items: &mut [WorkItem]) {
    items.sort_by(|left, right| {
        work_item_risk_rank(left.risk)
            .cmp(&work_item_risk_rank(right.risk))
            .then_with(|| {
                work_item_difficulty_rank(left.difficulty)
                    .cmp(&work_item_difficulty_rank(right.difficulty))
            })
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.allow_id.cmp(&right.allow_id))
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn work_item_risk_rank(risk: &str) -> u8 {
    match risk {
        RISK_HIGH => 0,
        RISK_MEDIUM => 1,
        RISK_LOW => 2,
        _ => 3,
    }
}

fn work_item_difficulty_rank(difficulty: &str) -> u8 {
    match difficulty {
        DIFFICULTY_SMALL => 0,
        DIFFICULTY_MEDIUM => 1,
        _ => 2,
    }
}

pub(super) fn renumber_work_items(items: &mut [WorkItem]) {
    for (index, item) in items.iter_mut().enumerate() {
        item.id = format!("work-{}-{:04}", item.kind.replace('_', "-"), index + 1);
    }
}
