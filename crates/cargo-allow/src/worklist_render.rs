use super::{WorkItem, WorklistContext, WorklistFilters};

pub(super) fn render_worklist_json_with_context(
    items: &[WorkItem],
    context: WorklistContext<'_>,
) -> String {
    let report_items = report_worklist_items(items);
    allow_report::render_worklist_json(
        &report_items,
        report_worklist_filters(context.filters),
        context.inventory,
    )
}

pub(super) fn render_worklist_human_with_context(
    items: &[WorkItem],
    context: WorklistContext<'_>,
) -> String {
    let report_items = report_worklist_items(items);
    allow_report::render_worklist_human(
        &report_items,
        report_worklist_filters(context.filters),
        context.inventory,
    )
}

fn report_worklist_items(items: &[WorkItem]) -> Vec<allow_report::WorklistItem<'_>> {
    items
        .iter()
        .map(|item| allow_report::WorklistItem {
            id: &item.id,
            kind: &item.kind,
            exception_kind: item.exception_kind.as_deref(),
            family: item.family.as_deref(),
            owner: item.owner.as_deref(),
            classification: item.classification.as_deref(),
            reason: item.reason.as_deref(),
            created: item.created.as_deref(),
            review_after: item.review_after.as_deref(),
            expires: item.expires.as_deref(),
            evidence_count: item.evidence_count,
            risk: item.risk,
            difficulty: item.difficulty,
            status: item.status.as_str(),
            allow_id: item.allow_id.as_deref(),
            finding_index: item.finding_index,
            path: item.path.as_deref(),
            evidence_reference: item.evidence_reference.as_ref().map(|reference| {
                allow_report::EvidenceReference {
                    raw: &reference.raw,
                    prefix: reference.prefix.as_deref(),
                    target: reference.target.as_deref(),
                    status: &reference.status,
                    message: &reference.message,
                }
            }),
            source_package: item.source_package.as_deref(),
            message: &item.message,
            suggested_actions: &item.suggested_actions,
            proof_commands: &item.proof_commands,
        })
        .collect()
}

fn report_worklist_filters(filters: WorklistFilters<'_>) -> allow_report::WorklistFilters<'_> {
    allow_report::WorklistFilters {
        kind: filters.kind,
        family: filters.family,
        item_kind: filters.item_kind,
        status: filters.status,
        allow_id: filters.allow_id,
        path: filters.path,
        source_package: filters.source_package,
        owner: filters.owner,
        classification: filters.classification,
        baseline_debt: filters.baseline_debt,
        broad_scope: filters.broad_scope,
        risk: filters.risk,
        difficulty: filters.difficulty,
        missing_evidence: filters.missing_evidence,
    }
}
