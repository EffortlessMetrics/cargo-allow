use crate::contracts::LIST_ARTIFACT;
use crate::json::{bool_json, option_json, push_json_fixed_artifact_preamble};
use crate::{CLAIM_BOUNDARY_TEXT, InventoryContext, ListColumn, ListFilters, ListRow, Style};
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
    render_list_human_columns_styled(rows, inventory, ListColumn::ALL, Style::PLAIN)
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
    render_list_human_concise_styled(rows, inventory, filters, columns, Style::PLAIN)
}

pub fn render_list_human_concise_styled(
    rows: &[ListRow<'_>],
    inventory: InventoryContext<'_>,
    filters: ListFilters<'_>,
    columns: &[ListColumn],
    style: Style,
) -> String {
    render_list_human_columns_internal(rows, inventory, columns, true, filters, style)
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
    render_list_human_columns_styled(rows, inventory, columns, Style::PLAIN)
}

pub fn render_list_human_columns_styled(
    rows: &[ListRow<'_>],
    inventory: InventoryContext<'_>,
    columns: &[ListColumn],
    style: Style,
) -> String {
    render_list_human_columns_internal(
        rows,
        inventory,
        columns,
        false,
        ListFilters::default(),
        style,
    )
}

fn render_list_human_columns_internal(
    rows: &[ListRow<'_>],
    inventory: InventoryContext<'_>,
    columns: &[ListColumn],
    concise: bool,
    filters: ListFilters<'_>,
    style: Style,
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
        push_concise_summary(&mut out, rows, style);
        push_concise_cards(&mut out, rows, style);
    } else {
        push_header(&mut out, columns);
        for row in rows {
            push_row(&mut out, row, columns, style);
        }
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

fn push_row(out: &mut String, row: &ListRow<'_>, columns: &[ListColumn], style: Style) {
    for (index, column) in columns.iter().enumerate() {
        if index > 0 {
            out.push('\t');
        }
        if *column == ListColumn::Status {
            out.push_str(&style_status(style, row.status));
        } else {
            out.push_str(&column.value(row));
        }
    }
    out.push('\n');
}

fn push_concise_cards(out: &mut String, rows: &[ListRow<'_>], style: Style) {
    if rows.is_empty() {
        return;
    }
    out.push_str("entries:\n");
    for row in rows {
        out.push_str("- [");
        out.push_str(&style_status(style, row.status));
        out.push_str("] ");
        out.push_str(&ListColumn::Id.concise_value(row));
        out.push('\n');
        out.push_str("  kind: ");
        out.push_str(&ListColumn::Kind.concise_value(row));
        if row.family.is_some() {
            out.push('.');
            out.push_str(&ListColumn::Family.concise_value(row));
        }
        out.push('\n');
        out.push_str("  scope: ");
        out.push_str(&ListColumn::Scope.concise_value(row));
        out.push('\n');
        out.push_str("  owner: ");
        out.push_str(&ListColumn::Owner.concise_value(row));
        out.push('\n');
        out.push_str(&format!(
            "  matches: {}; evidence: {}",
            row.matches, row.evidence_count
        ));
        if row.broken_evidence_references > 0 || row.weak_evidence_references > 0 {
            out.push_str(" (");
            let mut evidence_details = Vec::new();
            if row.broken_evidence_references > 0 {
                evidence_details.push(format!("broken: {}", row.broken_evidence_references));
            }
            if row.weak_evidence_references > 0 {
                evidence_details.push(format!("weak: {}", row.weak_evidence_references));
            }
            out.push_str(&evidence_details.join("; "));
            out.push(')');
        }
        out.push('\n');
        out.push_str("  reason: ");
        out.push_str(&ListColumn::Reason.concise_value(row));
        out.push('\n');
    }
}

fn style_status(style: Style, status: &str) -> String {
    style.status(status, status)
}

fn push_concise_summary(out: &mut String, rows: &[ListRow<'_>], style: Style) {
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
            details.push(format!("{}: {count}", style_status(style, status)));
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
    let mut fields = vec![
        format!("      \"id\": \"{}\"", json_escape(row.id)),
        format!("      \"status\": \"{}\"", json_escape(row.status)),
        format!("      \"matches\": {}", row.matches),
        format!("      \"kind\": \"{}\"", json_escape(row.kind)),
    ];
    if let Some(family) = row.family {
        fields.push(format!("      \"family\": \"{}\"", json_escape(family)));
    }
    fields.extend([
        format!("      \"owner\": \"{}\"", json_escape(row.owner)),
        format!(
            "      \"classification\": \"{}\"",
            json_escape(row.classification)
        ),
        format!("      \"scope\": \"{}\"", json_escape(row.scope)),
    ]);
    if let Some(source_package) = row.source_package {
        fields.push(format!(
            "      \"source_package\": \"{}\"",
            json_escape(source_package)
        ));
    }
    fields.push(format!("      \"evidence_count\": {}", row.evidence_count));
    if row.broken_evidence_references > 0 {
        fields.push(format!(
            "      \"broken_evidence_references\": {}",
            row.broken_evidence_references
        ));
    }
    if row.weak_evidence_references > 0 {
        fields.push(format!(
            "      \"weak_evidence_references\": {}",
            row.weak_evidence_references
        ));
    }
    fields.extend([
        format!("      \"selector_precision\": {}", row.selector_precision),
        format!("      \"broad_scope\": {}", bool_json(row.broad_scope)),
    ]);
    if let Some(review_after) = row.review_after {
        fields.push(format!(
            "      \"review_after\": \"{}\"",
            json_escape(review_after)
        ));
    }
    if let Some(expires) = row.expires {
        fields.push(format!("      \"expires\": \"{}\"", json_escape(expires)));
    }
    fields.push(format!("      \"reason\": \"{}\"", json_escape(row.reason)));
    format!("    {{\n{}\n    }}", fields.join(",\n"))
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
