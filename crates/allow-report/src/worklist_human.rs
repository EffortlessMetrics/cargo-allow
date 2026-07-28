use crate::evidence_reference_human::evidence_reference_human_status;
use crate::worklist_summary::{
    worklist_difficulty_count, worklist_kind_counts, worklist_risk_count,
};
use crate::{CLAIM_BOUNDARY_TEXT, InventoryContext, WorklistFilters, WorklistItem};

pub fn render_worklist_human(
    items: &[WorklistItem<'_>],
    filters: WorklistFilters<'_>,
    inventory: InventoryContext<'_>,
) -> String {
    let mut out = String::new();
    out.push_str("cargo-allow worklist\n\n");
    out.push_str(&format!(
        "Inventory: {}/{} via {}{}\n",
        inventory.scope,
        inventory.scanner,
        inventory.source,
        inventory.files_scanned_suffix()
    ));
    if let Some(root) = inventory.root {
        out.push_str(&format!("Source tree root: {root}\n"));
    }
    out.push_str(&worklist_filters_human(filters));
    out.push_str(&format!("Work items: {}\n", items.len()));
    out.push_str("Risk:\n");
    out.push_str(&format!(
        "  high      {}\n",
        worklist_risk_count(items, "high")
    ));
    out.push_str(&format!(
        "  medium    {}\n",
        worklist_risk_count(items, "medium")
    ));
    out.push_str(&format!(
        "  low       {}\n",
        worklist_risk_count(items, "low")
    ));
    out.push_str("Difficulty:\n");
    out.push_str(&format!(
        "  small     {}\n",
        worklist_difficulty_count(items, "small")
    ));
    out.push_str(&format!(
        "  medium    {}\n",
        worklist_difficulty_count(items, "medium")
    ));
    let kind_counts = worklist_kind_counts(items);
    if !kind_counts.is_empty() {
        out.push_str("Queue kinds:\n");
        for (kind, count) in kind_counts {
            out.push_str(&format!("  {kind:<26} {count}\n"));
        }
    }
    for item in items.iter().take(80) {
        out.push_str(&format!(
            "\n{} ({}, {}) {}\n",
            item.id, item.risk, item.difficulty, item.kind
        ));
        if let Some(path) = item.path {
            if let Some(line) = item.line {
                let col = item.column.map_or(String::new(), |c| format!(":{c}"));
                out.push_str(&format!(
                    "  path: {}:{line}{col}\n",
                    crate::style::sanitize_terminal_text(path)
                ));
            } else {
                out.push_str(&format!("  path: {path}\n"));
            }
        }
        if let Some(package) = item.source_package {
            out.push_str(&format!("  source package: {package}\n"));
        }
        if let Some(allow_id) = item.allow_id {
            out.push_str(&format!("  allow: {allow_id}\n"));
        }
        if let Some(owner) = item.owner {
            out.push_str(&format!("  owner: {owner}\n"));
        }
        if let Some(classification) = item.classification {
            out.push_str(&format!("  classification: {classification}\n"));
        }
        if let Some(reason) = item.reason {
            out.push_str(&format!(
                "  reason: {}\n",
                crate::style::sanitize_terminal_text(reason)
            ));
        }
        if let Some(created) = item.created {
            out.push_str(&format!("  created: {created}\n"));
        }
        if let Some(review_after) = item.review_after {
            out.push_str(&format!("  review_after: {review_after}\n"));
        }
        if let Some(expires) = item.expires {
            out.push_str(&format!("  expires: {expires}\n"));
        }
        if let Some(evidence_count) = item.evidence_count {
            out.push_str(&format!("  evidence: {evidence_count} reference(s)\n"));
        }
        if let Some(selector_precision) = item.selector_precision {
            out.push_str(&format!("  selector_precision: {selector_precision}\n"));
        }
        if let Some(reference) = item.evidence_reference.as_ref() {
            let status = evidence_reference_human_status(reference);
            out.push_str(&format!(
                "  evidence reference: {}: {} (status={}, prefix={}, target={})\n",
                status.label,
                reference.raw,
                reference.status,
                reference.prefix.unwrap_or("-"),
                reference.target.unwrap_or("-")
            ));
            out.push_str(&format!("  evidence message: {}\n", reference.message));
        }
        if let Some(exception_kind) = item.exception_kind {
            out.push_str(&format!("  exception: {exception_kind}"));
            if let Some(family) = item.family {
                out.push_str(&format!(".{family}"));
            }
            out.push('\n');
        }
        out.push_str(&format!("  status: {}\n", item.status));
        out.push_str(&format!("  message: {}\n", item.message));
        for action in item.suggested_actions.iter().take(2) {
            out.push_str(&format!("  action: {action}\n"));
        }
        for command in item.proof_commands.iter().take(8) {
            out.push_str(&format!("  proof: {command}\n"));
        }
    }
    if items.len() > 80 {
        out.push_str(&format!(
            "\n{} additional work items omitted from human output; use `cargo-allow worklist --format json` for the full queue.\n",
            items.len() - 80
        ));
    }
    out.push('\n');
    out.push_str(CLAIM_BOUNDARY_TEXT);
    out.push('\n');
    out
}

fn worklist_filters_human(filters: WorklistFilters<'_>) -> String {
    let mut parts = Vec::new();
    if let Some(kind) = filters.kind {
        parts.push(format!("kind={kind}"));
    }
    if let Some(family) = filters.family {
        parts.push(format!("family={family}"));
    }
    if let Some(item_kind) = filters.item_kind {
        parts.push(format!("item_kind={item_kind}"));
    }
    if let Some(status) = filters.status {
        parts.push(format!("status={status}"));
    }
    if let Some(allow_id) = filters.allow_id {
        parts.push(format!("allow_id={allow_id}"));
    }
    if let Some(path) = filters.path {
        parts.push(format!("path={path}"));
    }
    if let Some(source_package) = filters.source_package {
        parts.push(format!("source_package={source_package}"));
    }
    if let Some(owner) = filters.owner {
        parts.push(format!("owner={owner}"));
    }
    if let Some(classification) = filters.classification {
        parts.push(format!("classification={classification}"));
    }
    if filters.baseline_debt {
        parts.push("baseline_debt=true".to_string());
    }
    if filters.broad_scope {
        parts.push("broad_scope=true".to_string());
    }
    if let Some(risk) = filters.risk {
        parts.push(format!("risk={risk}"));
    }
    if let Some(difficulty) = filters.difficulty {
        parts.push(format!("difficulty={difficulty}"));
    }
    if filters.missing_evidence {
        parts.push("missing_evidence=true".to_string());
    }
    if filters.broken_evidence {
        parts.push("broken_evidence=true".to_string());
    }
    if filters.weak_evidence {
        parts.push("weak_evidence=true".to_string());
    }
    if parts.is_empty() {
        "Filters: none\n".to_string()
    } else {
        format!("Filters: {}\n", parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::worklist_filters_human;
    use crate::WorklistFilters;

    #[test]
    fn worklist_filters_human_records_every_filter_field() {
        let text = worklist_filters_human(WorklistFilters {
            kind: Some("unsafe"),
            family: Some("unsafe_block"),
            item_kind: Some("missing_evidence"),
            status: Some("evidence_missing"),
            allow_id: Some("allow-0001"),
            path: Some("src/lib.rs"),
            source_package: Some("allow-report"),
            owner: Some("runtime"),
            classification: Some("reviewed"),
            baseline_debt: true,
            broad_scope: true,
            risk: Some("high"),
            difficulty: Some("small"),
            missing_evidence: true,
            broken_evidence: true,
            weak_evidence: true,
        });

        assert_eq!(
            text,
            "Filters: kind=unsafe, family=unsafe_block, item_kind=missing_evidence, status=evidence_missing, allow_id=allow-0001, path=src/lib.rs, source_package=allow-report, owner=runtime, classification=reviewed, baseline_debt=true, broad_scope=true, risk=high, difficulty=small, missing_evidence=true, broken_evidence=true, weak_evidence=true\n"
        );
    }

    #[test]
    fn worklist_filters_human_reports_none_when_no_filters_are_set() {
        assert_eq!(
            worklist_filters_human(WorklistFilters::default()),
            "Filters: none\n"
        );
    }
}
