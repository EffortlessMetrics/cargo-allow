use crate::json::{
    bool_json, json_string_array, option_json, push_json_artifact_header,
    push_json_artifact_source_context,
};
use crate::{
    CLAIM_BOUNDARY_TEXT, InventoryContext, WORKLIST_SCHEMA_ID, WORKLIST_SCHEMA_VERSION,
    WorklistFilters, WorklistItem,
};
use allow_core::json_escape;

pub fn render_worklist_json(
    items: &[WorklistItem<'_>],
    filters: WorklistFilters<'_>,
    inventory: InventoryContext<'_>,
) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    push_json_artifact_header(
        &mut out,
        WORKLIST_SCHEMA_VERSION,
        WORKLIST_SCHEMA_ID,
        "worklist",
    );
    push_json_artifact_source_context(&mut out, inventory);
    out.push_str("  \"filters\": ");
    out.push_str(&render_worklist_filters_json(filters, "  "));
    out.push_str(",\n");
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!("    \"work_items\": {},\n", items.len()));
    out.push_str(&format!(
        "    \"high\": {},\n",
        worklist_risk_count(items, "high")
    ));
    out.push_str(&format!(
        "    \"medium\": {},\n",
        worklist_risk_count(items, "medium")
    ));
    out.push_str(&format!(
        "    \"low\": {},\n",
        worklist_risk_count(items, "low")
    ));
    out.push_str(&format!(
        "    \"small_difficulty\": {},\n",
        worklist_difficulty_count(items, "small")
    ));
    out.push_str(&format!(
        "    \"medium_difficulty\": {}\n",
        worklist_difficulty_count(items, "medium")
    ));
    out.push_str("  },\n");
    out.push_str("  \"work_items\": [\n");
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&render_work_item_json(item));
    }
    out.push_str("\n  ]\n");
    out.push_str("}\n");
    out
}

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
        for command in item.proof_commands.iter().take(3) {
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

fn render_work_item_json(item: &WorklistItem<'_>) -> String {
    let mut out = String::new();
    out.push_str("    {\n");
    out.push_str(&format!("      \"id\": \"{}\",\n", json_escape(item.id)));
    out.push_str(&format!(
        "      \"kind\": \"{}\",\n",
        json_escape(item.kind)
    ));
    out.push_str(&format!(
        "      \"exception_kind\": {},\n",
        option_json(item.exception_kind)
    ));
    out.push_str(&format!(
        "      \"family\": {},\n",
        option_json(item.family)
    ));
    out.push_str(&format!("      \"owner\": {},\n", option_json(item.owner)));
    out.push_str(&format!(
        "      \"classification\": {},\n",
        option_json(item.classification)
    ));
    out.push_str(&format!(
        "      \"reason\": {},\n",
        option_json(item.reason)
    ));
    out.push_str(&format!(
        "      \"created\": {},\n",
        option_json(item.created)
    ));
    out.push_str(&format!(
        "      \"review_after\": {},\n",
        option_json(item.review_after)
    ));
    out.push_str(&format!(
        "      \"expires\": {},\n",
        option_json(item.expires)
    ));
    out.push_str(&format!(
        "      \"evidence_count\": {},\n",
        item.evidence_count
            .map(|count| count.to_string())
            .unwrap_or_else(|| "null".to_string())
    ));
    out.push_str(&format!(
        "      \"risk\": \"{}\",\n",
        json_escape(item.risk)
    ));
    out.push_str(&format!(
        "      \"difficulty\": \"{}\",\n",
        json_escape(item.difficulty)
    ));
    out.push_str(&format!(
        "      \"status\": \"{}\",\n",
        json_escape(item.status)
    ));
    out.push_str(&format!(
        "      \"allow_id\": {},\n",
        option_json(item.allow_id)
    ));
    out.push_str(&format!(
        "      \"finding_index\": {},\n",
        item.finding_index
            .map(|index| index.to_string())
            .unwrap_or_else(|| "null".to_string())
    ));
    out.push_str(&format!("      \"path\": {},\n", option_json(item.path)));
    out.push_str(&format!(
        "      \"source_package\": {},\n",
        option_json(item.source_package)
    ));
    out.push_str(&format!(
        "      \"message\": \"{}\",\n",
        json_escape(item.message)
    ));
    out.push_str(&format!(
        "      \"suggested_actions\": {},\n",
        json_string_array(item.suggested_actions)
    ));
    out.push_str(&format!(
        "      \"proof_commands\": {}\n",
        json_string_array(item.proof_commands)
    ));
    out.push_str("    }");
    out
}

fn render_worklist_filters_json(filters: WorklistFilters<'_>, indent: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "{indent}  \"kind\": {},\n",
        option_json(filters.kind)
    ));
    out.push_str(&format!(
        "{indent}  \"family\": {},\n",
        option_json(filters.family)
    ));
    out.push_str(&format!(
        "{indent}  \"item_kind\": {},\n",
        option_json(filters.item_kind)
    ));
    out.push_str(&format!(
        "{indent}  \"status\": {},\n",
        option_json(filters.status)
    ));
    out.push_str(&format!(
        "{indent}  \"allow_id\": {},\n",
        option_json(filters.allow_id)
    ));
    out.push_str(&format!(
        "{indent}  \"path\": {},\n",
        option_json(filters.path)
    ));
    out.push_str(&format!(
        "{indent}  \"source_package\": {},\n",
        option_json(filters.source_package)
    ));
    out.push_str(&format!(
        "{indent}  \"owner\": {},\n",
        option_json(filters.owner)
    ));
    out.push_str(&format!(
        "{indent}  \"classification\": {},\n",
        option_json(filters.classification)
    ));
    out.push_str(&format!(
        "{indent}  \"baseline_debt\": {},\n",
        bool_json(filters.baseline_debt)
    ));
    out.push_str(&format!(
        "{indent}  \"broad_scope\": {},\n",
        bool_json(filters.broad_scope)
    ));
    out.push_str(&format!(
        "{indent}  \"risk\": {},\n",
        option_json(filters.risk)
    ));
    out.push_str(&format!(
        "{indent}  \"difficulty\": {},\n",
        option_json(filters.difficulty)
    ));
    out.push_str(&format!(
        "{indent}  \"missing_evidence\": {}\n",
        bool_json(filters.missing_evidence)
    ));
    out.push_str(&format!("{indent}}}"));
    out
}

fn worklist_risk_count(items: &[WorklistItem<'_>], risk: &str) -> usize {
    items.iter().filter(|item| item.risk == risk).count()
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

fn worklist_difficulty_count(items: &[WorklistItem<'_>], difficulty: &str) -> usize {
    items
        .iter()
        .filter(|item| item.difficulty == difficulty)
        .count()
}
