use crate::contracts::LIST_ARTIFACT;
use crate::json::{bool_json, option_json, push_json_fixed_artifact_preamble};
use crate::{CLAIM_BOUNDARY_TEXT, InventoryContext, ListFilters, ListRow};
use allow_core::json_escape;

pub fn render_list_json(
    rows: &[ListRow<'_>],
    filters: ListFilters<'_>,
    inventory: InventoryContext<'_>,
) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    push_json_fixed_artifact_preamble(&mut out, LIST_ARTIFACT, inventory);
    out.push_str("  \"filters\": ");
    out.push_str(&render_list_filters_json(filters, "  "));
    out.push_str(",\n");
    out.push_str(&format!(
        "  \"summary\": {{\n    \"allow_entries\": {}\n  }},\n",
        rows.len()
    ));
    out.push_str("  \"allow_entries\": [\n");
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&render_list_row_json(row));
    }
    out.push_str("\n  ]\n");
    out.push_str("}\n");
    out
}

pub fn render_list_human(rows: &[ListRow<'_>], inventory: InventoryContext<'_>) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "inventory: {}/{} via {}{}\n",
        inventory.scope,
        inventory.scanner,
        inventory.source,
        list_inventory_files_suffix(inventory)
    ));
    if let Some(root) = inventory.root {
        out.push_str(&format!("source_tree_root: {root}\n"));
    }
    out.push_str("id\tstatus\tmatches\tkind\tfamily\towner\tclassification\tscope\tsource_package\tevidence_count\tbroken_evidence_references\tweak_evidence_references\tselector_precision\tbroad_scope\treview_after\texpires\treason\n");
    for row in rows {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            row.id,
            row.status,
            row.matches,
            row.kind,
            row.family.unwrap_or("-"),
            empty_as_dash(row.owner),
            empty_as_dash(row.classification),
            row.scope,
            row.source_package.unwrap_or("-"),
            row.evidence_count,
            row.broken_evidence_references,
            row.weak_evidence_references,
            row.selector_precision,
            row.broad_scope,
            row.review_after.unwrap_or("-"),
            row.expires.unwrap_or("-"),
            row.reason
        ));
    }
    if rows.is_empty() {
        out.push_str("(no allow entries matched filters)\n");
    }
    out.push_str(CLAIM_BOUNDARY_TEXT);
    out.push('\n');
    out
}

fn list_inventory_files_suffix(inventory: InventoryContext<'_>) -> String {
    inventory
        .files_scanned
        .map(|files| format!("; files scanned: {files}"))
        .unwrap_or_default()
}

fn empty_as_dash(value: &str) -> &str {
    if value.trim().is_empty() { "-" } else { value }
}

fn render_list_row_json(row: &ListRow<'_>) -> String {
    let mut out = String::new();
    out.push_str("    {\n");
    out.push_str(&format!("      \"id\": \"{}\",\n", json_escape(row.id)));
    out.push_str(&format!(
        "      \"status\": \"{}\",\n",
        json_escape(row.status)
    ));
    out.push_str(&format!("      \"matches\": {},\n", row.matches));
    out.push_str(&format!("      \"kind\": \"{}\",\n", json_escape(row.kind)));
    out.push_str(&format!("      \"family\": {},\n", option_json(row.family)));
    out.push_str(&format!(
        "      \"owner\": \"{}\",\n",
        json_escape(row.owner)
    ));
    out.push_str(&format!(
        "      \"classification\": \"{}\",\n",
        json_escape(row.classification)
    ));
    out.push_str(&format!(
        "      \"scope\": \"{}\",\n",
        json_escape(row.scope)
    ));
    out.push_str(&format!(
        "      \"source_package\": {},\n",
        option_json(row.source_package)
    ));
    out.push_str(&format!(
        "      \"evidence_count\": {},\n",
        row.evidence_count
    ));
    if row.broken_evidence_references > 0 {
        out.push_str(&format!(
            "      \"broken_evidence_references\": {},\n",
            row.broken_evidence_references
        ));
    }
    if row.weak_evidence_references > 0 {
        out.push_str(&format!(
            "      \"weak_evidence_references\": {},\n",
            row.weak_evidence_references
        ));
    }
    out.push_str(&format!(
        "      \"selector_precision\": {},\n",
        row.selector_precision
    ));
    out.push_str(&format!(
        "      \"broad_scope\": {},\n",
        bool_json(row.broad_scope)
    ));
    out.push_str(&format!(
        "      \"review_after\": {},\n",
        option_json(row.review_after)
    ));
    out.push_str(&format!(
        "      \"expires\": {},\n",
        option_json(row.expires)
    ));
    out.push_str(&format!(
        "      \"reason\": \"{}\"\n",
        json_escape(row.reason)
    ));
    out.push_str("    }");
    out
}

fn render_list_filters_json(filters: ListFilters<'_>, indent: &str) -> String {
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
        "{indent}  \"owner\": {},\n",
        option_json(filters.owner)
    ));
    out.push_str(&format!(
        "{indent}  \"classification\": {},\n",
        option_json(filters.classification)
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
        "{indent}  \"allow_id\": {},\n",
        option_json(filters.allow_id)
    ));
    out.push_str(&format!(
        "{indent}  \"status\": {},\n",
        option_json(filters.status)
    ));
    out.push_str(&format!(
        "{indent}  \"expired\": {},\n",
        bool_json(filters.expired)
    ));
    out.push_str(&format!(
        "{indent}  \"review_due\": {},\n",
        bool_json(filters.review_due)
    ));
    out.push_str(&format!(
        "{indent}  \"stale\": {},\n",
        bool_json(filters.stale)
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
        "{indent}  \"missing_evidence\": {},\n",
        bool_json(filters.missing_evidence)
    ));
    out.push_str(&format!(
        "{indent}  \"broken_evidence\": {},\n",
        bool_json(filters.broken_evidence)
    ));
    out.push_str(&format!(
        "{indent}  \"weak_evidence\": {}\n",
        bool_json(filters.weak_evidence)
    ));
    out.push_str(&format!("{indent}}}"));
    out
}
