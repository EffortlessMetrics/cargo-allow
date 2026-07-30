use crate::Style;
use crate::allow_entry_json::push_optional_string_field;
use crate::contracts::ADD_ARTIFACT;
use crate::json::{bool_json, option_json, push_json_fixed_artifact_preamble};
use crate::mutation_receipt::render_mutation_receipt_json;
use crate::{
    AddReport, CLAIM_BOUNDARY_TEXT, finding_location_text, render_explain_finding_json,
    render_last_seen_json, render_selector_json,
};
use allow_core::{json_escape, normalize_path};

pub fn render_add_human(report: AddReport<'_>) -> String {
    render_add_human_styled(report, Style::PLAIN)
}

pub fn render_add_human_styled(report: AddReport<'_>, style: Style) -> String {
    let entry = report.entry;
    let selected_finding = report.selected_finding;
    let mut out = String::new();
    out.push_str("cargo-allow add summary\n");
    out.push_str(&format!(
        "inventory: {}/{} via {}{}\n",
        report.inventory.scope,
        report.inventory.scanner,
        report.inventory.source,
        report.inventory.files_scanned_suffix()
    ));
    if let Some(root) = report.inventory.root {
        out.push_str(&format!("source_tree_root: {root}\n"));
    }
    out.push_str(&format!("id: {}\n", entry.id));
    out.push_str(&format!("kind: {}\n", entry.kind));
    if let Some(family) = &entry.family {
        out.push_str(&format!("family: {family}\n"));
    }
    out.push_str(&format!("scope: {}\n", entry.path_or_glob()));
    out.push_str(&format!("owner: {}\n", entry.owner));
    out.push_str(&format!("classification: {}\n", entry.classification));
    out.push_str(&format!(
        "matched finding: {}\n",
        finding_location_text(selected_finding)
    ));
    if let Some(output) = report.policy_output {
        out.push_str(&format!("output: {output}\n"));
    } else {
        out.push_str("output: stdout\n");
    }
    out.push_str("claim boundary: generated policy entry requires human ");
    out.push_str(&style.status("review_due", "review"));
    out.push_str(" before merge.\n");
    out.push_str(CLAIM_BOUNDARY_TEXT);
    out.push('\n');
    out
}

pub fn render_add_json(report: AddReport<'_>) -> String {
    let entry = report.entry;
    let selected_finding = report.selected_finding;
    let path = entry.path.as_ref().map(normalize_path);
    let mut out = String::new();
    out.push_str("{\n");
    push_json_fixed_artifact_preamble(&mut out, ADD_ARTIFACT, report.inventory);
    out.push_str("  \"mutation_receipt\": ");
    out.push_str(&render_mutation_receipt_json(
        &report.mutation_receipt,
        "  ",
    ));
    out.push_str(",\n");
    out.push_str("  \"options\": {\n");
    out.push_str(&format!(
        "    \"policy_output\": {},\n",
        option_json(report.policy_output)
    ));
    out.push_str(&format!("    \"force\": {}\n", bool_json(report.force)));
    out.push_str("  },\n");
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!(
        "    \"entry_id\": \"{}\",\n",
        json_escape(&entry.id)
    ));
    out.push_str(&format!(
        "    \"selected_finding\": \"{}\",\n",
        json_escape(&finding_location_text(selected_finding))
    ));
    out.push_str("    \"human_review_required\": true\n");
    out.push_str("  },\n");
    let mut entry_fields = vec![
        format!("    \"id\": \"{}\"", json_escape(&entry.id)),
        format!("    \"kind\": \"{}\"", entry.kind),
    ];
    push_optional_string_field(&mut entry_fields, "    ", "family", entry.family.as_deref());
    entry_fields.extend([
        format!("    \"path\": {}", option_json(path.as_deref())),
        format!("    \"glob\": {}", option_json(entry.glob.as_deref())),
        format!("    \"owner\": \"{}\"", json_escape(&entry.owner)),
        format!(
            "    \"classification\": \"{}\"",
            json_escape(&entry.classification)
        ),
        format!("    \"reason\": \"{}\"", json_escape(&entry.reason)),
    ]);
    push_optional_string_field(
        &mut entry_fields,
        "    ",
        "review_after",
        entry.lifecycle.review_after.as_deref(),
    );
    push_optional_string_field(
        &mut entry_fields,
        "    ",
        "expires",
        entry.lifecycle.expires.as_deref(),
    );
    entry_fields.extend([
        format!("    \"evidence_count\": {}", entry.evidence.len()),
        format!(
            "    \"selector\": {}",
            render_selector_json(&entry.selector, "    ")
        ),
        format!(
            "    \"last_seen\": {}",
            render_last_seen_json(entry.last_seen.as_ref(), "    ")
        ),
    ]);
    out.push_str(&format!(
        "  \"allow_entry\": {{\n{}\n  }},\n",
        entry_fields.join(",\n")
    ));
    out.push_str("  \"selected_finding\": ");
    out.push_str(&render_explain_finding_json(
        selected_finding,
        "selected",
        "  ",
    ));
    out.push_str("\n}\n");
    out
}
