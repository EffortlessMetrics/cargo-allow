use super::worklist_item_kind::{
    BASELINE_DEBT, BROAD_SCOPE, BROKEN_EVIDENCE_LINK, WEAK_EVIDENCE_REFERENCE,
};
use super::worklist_priority::{
    DIFFICULTY_MEDIUM, DIFFICULTY_SMALL, RISK_HIGH, RISK_LOW, RISK_MEDIUM,
};
use super::{WorkItem, WorklistFilters};
use crate::parse_kind_filter;
use allow_core::{FindingKind, MatchStatus, source_tree_path_matches_filter};

pub(super) fn filter_work_items(
    items: Vec<WorkItem>,
    filters: WorklistFilters<'_>,
) -> Vec<WorkItem> {
    items
        .into_iter()
        .filter(|item| {
            kind_matches(item, filters.kind)
                && filters
                    .family
                    .map(|family| item.family.as_deref() == Some(family))
                    .unwrap_or(true)
                && filters
                    .item_kind
                    .map(|item_kind| item_kind_matches(item.kind.as_str(), item_kind))
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
                    .map(|path| work_item_path_matches_filter(item, path))
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
                && (!filters.broken_evidence || item.kind == BROKEN_EVIDENCE_LINK)
                && (!filters.weak_evidence || item.kind == WEAK_EVIDENCE_REFERENCE)
        })
        .collect()
}

fn work_item_path_matches_filter(item: &WorkItem, filter: &str) -> bool {
    let Some(item_path) = item.path.as_deref() else {
        return false;
    };
    if item
        .evidence_reference
        .as_ref()
        .is_some_and(|reference| reference.status == "invalid_local_path")
    {
        return item_path.replace('\\', "/") == filter.replace('\\', "/");
    }
    source_tree_path_matches_filter(item_path, filter)
}

fn item_kind_matches(item_kind: &str, filter: &str) -> bool {
    item_kind == filter || item_kind == filter.replace('-', "_")
}

fn kind_matches(item: &WorkItem, kind: Option<&str>) -> bool {
    let Some(kind) = kind else {
        return true;
    };
    let Ok(parsed) = parse_kind_filter(kind) else {
        return false;
    };
    item.exception_kind
        .as_deref()
        .and_then(|kind| kind.parse::<FindingKind>().ok())
        .is_some_and(|item_kind| item_kind == parsed.kind)
        && parsed.family.matches(item.family.as_deref())
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
            .then_with(|| selector_precision_rank(left).cmp(&selector_precision_rank(right)))
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

fn selector_precision_rank(item: &WorkItem) -> u32 {
    item.selector_precision.unwrap_or(u32::MAX)
}

pub(super) fn renumber_work_items(items: &mut [WorkItem]) {
    for (index, item) in items.iter_mut().enumerate() {
        item.id = format!("work-{}-{:04}", item.kind.replace('_', "-"), index + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        item_kind_matches, kind_matches, selector_precision_rank, work_item_path_matches_filter,
        work_item_risk_rank,
    };
    use crate::worklist::{
        WorkItem, WorkItemEvidenceReference,
        worklist_item_kind::BROKEN_EVIDENCE_LINK,
        worklist_priority::{RISK_HIGH, RISK_LOW, RISK_MEDIUM},
        worklist_types::WorkItemLedger,
    };
    use allow_core::MatchStatus;

    fn queue_item(path: Option<&str>) -> WorkItem {
        WorkItem {
            id: "work-fixture".to_string(),
            kind: "new_unreceipted_finding".to_string(),
            exception_kind: Some("panic".to_string()),
            family: Some("unwrap".to_string()),
            owner: Some("owner".to_string()),
            classification: Some("classification".to_string()),
            reason: Some("reason".to_string()),
            created: None,
            review_after: None,
            expires: None,
            evidence_count: Some(1),
            selector_precision: Some(10),
            risk: RISK_LOW,
            difficulty: "small",
            status: MatchStatus::New,
            allow_id: Some("allow-1".to_string()),
            finding_index: Some(0),
            path: path.map(str::to_string),
            evidence_reference: None,
            source_package: None,
            message: "message".to_string(),
            suggested_actions: Vec::new(),
            proof_commands: Vec::new(),
            ledger: WorkItemLedger::default(),
        }
    }

    #[test]
    fn work_item_path_matches_invalid_evidence_exactly_and_source_scope_filters() {
        let missing_path = queue_item(None);
        assert!(!work_item_path_matches_filter(
            &missing_path,
            "docs/README.md"
        ));

        let mut invalid_evidence = queue_item(Some(r"evidence\missing.md"));
        invalid_evidence.evidence_reference = Some(WorkItemEvidenceReference {
            raw: "file:evidence\\missing.md".to_string(),
            prefix: Some("file".to_string()),
            target: Some(r"evidence\missing.md".to_string()),
            status: "invalid_local_path".to_string(),
            category: "missing".to_string(),
            message: "missing file".to_string(),
        });
        assert!(work_item_path_matches_filter(
            &invalid_evidence,
            "evidence/missing.md"
        ));
        assert!(!work_item_path_matches_filter(
            &invalid_evidence,
            "evidence/other.md"
        ));

        let source_scope = queue_item(Some("docs/**/*.md"));
        assert!(work_item_path_matches_filter(
            &source_scope,
            "docs/guide.md"
        ));
        assert!(!work_item_path_matches_filter(
            &source_scope,
            "src/guide.md"
        ));
    }

    #[test]
    fn item_kind_and_exception_kind_filters_handle_aliases_and_invalid_input() {
        let accepted = true;
        let rejected = false;

        assert_eq!(
            item_kind_matches("broken_evidence_link", "broken_evidence_link"),
            accepted
        );
        let hyphen_filter = "broken-evidence-link";
        let normalized_item_kind = hyphen_filter.replace('-', "_");
        assert_eq!(normalized_item_kind, "broken_evidence_link");
        assert_eq!(
            item_kind_matches(&normalized_item_kind, hyphen_filter),
            accepted
        );
        assert_eq!(
            item_kind_matches(
                "broken-evidence-link".replace('-', "_").as_str(),
                "broken-evidence-link"
            ),
            accepted
        );
        assert_eq!(
            item_kind_matches(BROKEN_EVIDENCE_LINK, "weak-evidence"),
            rejected
        );

        let panic_item = queue_item(Some("src/lib.rs"));
        assert!(kind_matches(&panic_item, None));
        assert!(kind_matches(&panic_item, Some("panic")));
        assert!(!kind_matches(&panic_item, Some("unsafe")));
        assert!(!kind_matches(&panic_item, Some("not-a-kind")));
    }

    #[test]
    fn risk_and_selector_precision_rankers_keep_sort_contract() {
        assert_eq!(work_item_risk_rank(RISK_HIGH), 0);
        assert_eq!(work_item_risk_rank(RISK_MEDIUM), 1);
        assert_eq!(work_item_risk_rank(RISK_LOW), 2);
        assert_eq!(work_item_risk_rank("unknown"), 3);

        let precise = queue_item(Some("src/lib.rs"));
        assert_eq!(selector_precision_rank(&precise), 10);

        let mut missing_precision = queue_item(Some("src/lib.rs"));
        missing_precision.selector_precision = None;
        assert_eq!(selector_precision_rank(&missing_precision), u32::MAX);
    }
}
