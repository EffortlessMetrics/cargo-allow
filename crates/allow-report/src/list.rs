use crate::contracts::LIST_ARTIFACT;
use crate::json::{bool_json, option_json, push_json_fixed_artifact_preamble};
use crate::{CLAIM_BOUNDARY_TEXT, InventoryContext, ListColumn, ListFilters, ListRow};
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
    render_list_human_columns(rows, inventory, ListColumn::ALL)
}

/// Render the concise CLI human view with bounded repository-controlled cells
/// and a status/evidence summary. The complete human projection remains
/// available through [`render_list_human_columns`] and `--wide`.
pub fn render_list_human_concise(
    rows: &[ListRow<'_>],
    inventory: InventoryContext<'_>,
    filters: ListFilters<'_>,
    columns: &[ListColumn],
) -> String {
    render_list_human_columns_internal(rows, inventory, columns, true, filters)
}

/// Render the list human-format TSV with a column subset (#2595).
///
/// `columns` controls both the header row and the per-row cell projection.
/// All other behavior (inventory prefix, empty-rows notice, next-steps,
/// claim boundary) is identical to the default `render_list_human`.
pub fn render_list_human_columns(
    rows: &[ListRow<'_>],
    inventory: InventoryContext<'_>,
    columns: &[ListColumn],
) -> String {
    render_list_human_columns_internal(rows, inventory, columns, false, ListFilters::default())
}

fn render_list_human_columns_internal(
    rows: &[ListRow<'_>],
    inventory: InventoryContext<'_>,
    columns: &[ListColumn],
    concise: bool,
    filters: ListFilters<'_>,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "inventory: {}/{} via {}{}\n",
        inventory.scope,
        inventory.scanner,
        inventory.source,
        inventory.files_scanned_suffix()
    ));
    if let Some(root) = inventory.root {
        out.push_str(&format!("source_tree_root: {root}\n"));
    }
    if concise {
        push_concise_summary(&mut out, rows);
    }
    push_header(&mut out, columns);
    for row in rows {
        push_row(&mut out, row, columns, concise);
    }
    if rows.is_empty() {
        if !concise {
            out.push_str("(no allow entries matched filters)\n");
        } else if inventory.empty_git_tracked {
            out.push_str("(no tracked source files were found; inventory is empty)\n");
        } else if filters.has_active_filter() {
            out.push_str("(no allow entries matched filters)\n");
        } else {
            out.push_str("(no allow entries are configured)\n");
        }
    }
    append_list_next_steps(&mut out, rows);
    if !crate::contracts::is_quiet() {
        out.push_str(CLAIM_BOUNDARY_TEXT);
        out.push('\n');
    }
    out
}

fn push_header(out: &mut String, columns: &[ListColumn]) {
    for (index, column) in columns.iter().enumerate() {
        if index > 0 {
            out.push('\t');
        }
        out.push_str(column.header());
    }
    out.push('\n');
}

fn push_row(out: &mut String, row: &ListRow<'_>, columns: &[ListColumn], concise: bool) {
    for (index, column) in columns.iter().enumerate() {
        if index > 0 {
            out.push('\t');
        }
        if concise {
            out.push_str(&column.concise_value(row));
        } else {
            out.push_str(&column.value(row));
        }
    }
    out.push('\n');
}

fn push_concise_summary(out: &mut String, rows: &[ListRow<'_>]) {
    const STATUS_ORDER: &[&str] = &[
        "matched",
        "new",
        "stale",
        "expired",
        "review_due",
        "location_drift",
        "ambiguous",
        "invalid_selector",
        "missing_required_field",
        "evidence_missing",
        "baseline_debt",
    ];

    let mut details = Vec::new();
    for status in STATUS_ORDER {
        let count = rows.iter().filter(|row| row.status == *status).count();
        if count > 0 {
            details.push(format!("{status}: {count}"));
        }
    }
    let broken = rows
        .iter()
        .map(|row| row.broken_evidence_references)
        .sum::<usize>();
    if broken > 0 {
        details.push(format!("broken evidence: {broken}"));
    }
    let weak = rows
        .iter()
        .map(|row| row.weak_evidence_references)
        .sum::<usize>();
    if weak > 0 {
        details.push(format!("weak evidence: {weak}"));
    }

    out.push_str(&format!("summary: {} allow entries shown", rows.len()));
    if !details.is_empty() {
        out.push_str(" (");
        out.push_str(&details.join("; "));
        out.push(')');
    }
    out.push('\n');
}

/// Suggests per-entry commands for rows with actionable statuses or broken
/// evidence so the operator can go from "this entry is stale" to the fix
/// without looking up the command syntax.
fn append_list_next_steps(out: &mut String, rows: &[ListRow<'_>]) {
    let actionable: Vec<&ListRow<'_>> = rows
        .iter()
        .filter(|row| {
            row.status != "matched" && row.status != "healthy" && row.status != "baseline_debt"
                || row.broken_evidence_references > 0
        })
        .take(40)
        .collect();
    if actionable.is_empty() {
        return;
    }
    out.push_str("\nNext steps:\n");
    for row in actionable {
        let cmd = list_row_command(row);
        out.push_str(&format!("  {} ({}): {}\n", row.id, row.status, cmd));
    }
}

fn list_row_command(row: &ListRow<'_>) -> &'static str {
    if row.broken_evidence_references > 0 {
        return "cargo-allow worklist --broken-evidence";
    }
    match row.status {
        "stale" | "expired" => "cargo-allow prune --dry-run",
        "location_drift" => "cargo-allow refresh --dry-run",
        "review_due" => "cargo-allow explain <id>",
        _ => "cargo-allow explain <id>",
    }
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
        "{indent}  \"location_drift\": {},\n",
        bool_json(filters.location_drift)
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
