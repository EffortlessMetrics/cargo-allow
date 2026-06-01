use crate::evidence_reference_human::evidence_reference_human_status;
use crate::worklist_summary::{worklist_difficulty_count, worklist_risk_count};
use crate::{CLAIM_BOUNDARY_TEXT, InventoryContext, WorklistFilters, WorklistItem};

pub fn render_worklist_human(
    items: &[WorklistItem<'_>],
    filters: WorklistFilters<'_>,
    inventory: InventoryContext<'_>,
) -> String {
    let mut out = String::new();
    out.push_str("cargo-allow worklist\n\n");
    out.push_str(&format!(
        "Inventory: source_tree/source_syntax via {}{}\n",
        inventory.source,
        worklist_inventory_files_suffix(inventory)
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
    for item in items.iter().take(80) {
        out.push_str(&format!(
            "\n{} ({}, {}) {}\n",
            item.id, item.risk, item.difficulty, item.kind
        ));
        if let Some(path) = item.path {
            out.push_str(&format!("  path: {path}\n"));
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
            out.push_str(&format!("  reason: {reason}\n"));
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
                "  evidence reference: [{}] {}: {} (status={}, prefix={}, target={})\n",
                status.marker,
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
        for command in item.proof_commands.iter().take(6) {
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

fn worklist_inventory_files_suffix(inventory: InventoryContext<'_>) -> String {
    inventory
        .files_scanned
        .map(|files| format!("; files scanned: {files}"))
        .unwrap_or_default()
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
    if parts.is_empty() {
        "Filters: none\n".to_string()
    } else {
        format!("Filters: {}\n", parts.join(", "))
    }
}
