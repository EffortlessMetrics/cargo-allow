use crate::contracts::WORKLIST_ARTIFACT;
use crate::json::{bool_json, json_string_array, option_json, push_json_fixed_artifact_preamble};
use crate::worklist_summary::{worklist_difficulty_count, worklist_risk_count};
use crate::{InventoryContext, WorklistFilters, WorklistItem};
use allow_core::json_escape;

pub fn render_worklist_json(
    items: &[WorklistItem<'_>],
    filters: WorklistFilters<'_>,
    inventory: InventoryContext<'_>,
) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    push_json_fixed_artifact_preamble(&mut out, WORKLIST_ARTIFACT, inventory);
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
    if let Some(reference) = item.evidence_reference.as_ref() {
        out.push_str("      \"evidence_reference\": ");
        out.push_str(&render_worklist_evidence_reference_json(reference));
        out.push_str(",\n");
    }
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

fn render_worklist_evidence_reference_json(reference: &crate::EvidenceReference<'_>) -> String {
    format!(
        "{{\n        \"raw\": \"{}\",\n        \"prefix\": {},\n        \"target\": {},\n        \"status\": \"{}\",\n        \"category\": \"{}\",\n        \"message\": \"{}\"\n      }}",
        json_escape(reference.raw),
        option_json(reference.prefix),
        option_json(reference.target),
        json_escape(reference.status),
        json_escape(reference.category),
        json_escape(reference.message)
    )
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
