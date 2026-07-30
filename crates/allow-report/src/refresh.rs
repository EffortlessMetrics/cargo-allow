use crate::Style;
use crate::contracts::REFRESH_ARTIFACT;
use crate::json::{bool_json, option_json, push_json_fixed_artifact_preamble};
use crate::{
    CLAIM_BOUNDARY_TEXT, RefreshReport, finding_location_text, render_last_seen_json,
    render_mutation_receipt_json,
};
use allow_core::json_escape;

pub fn render_refresh_human(report: RefreshReport<'_>) -> String {
    render_refresh_human_styled(report, Style::PLAIN)
}

pub fn render_refresh_human_styled(report: RefreshReport<'_>, style: Style) -> String {
    let mut out = String::new();
    out.push_str("cargo-allow refresh\n\n");
    out.push_str(&format!(
        "Inventory: {}/{} via {}{}\n",
        report.inventory.scope,
        report.inventory.scanner,
        report.inventory.source,
        report.inventory.files_scanned_suffix()
    ));
    if let Some(root) = report.inventory.root {
        out.push_str(&format!("Source tree root: {root}\n"));
    }
    if report.mode.write_requested {
        out.push_str("mode: write\n");
    } else {
        out.push_str("mode: dry-run\n");
    }
    if report.mode.explicit_dry_run {
        out.push_str("requested: --dry-run\n");
    }
    out.push_str(&format!("allow id: {}\n", report.entry.id));
    out.push_str(&format!("drift: {}\n", report.drift_message));
    out.push_str(&format!(
        "matched finding: {}\n",
        finding_location_text(report.finding)
    ));
    if let Some(previous) = &report.previous_last_seen {
        out.push_str(&format!(
            "previous last_seen: {}:{}\n",
            previous.line, previous.column
        ));
    }
    if let Some(current) = &report.entry.last_seen {
        out.push_str(&format!(
            "refreshed last_seen: {}:{}\n",
            current.line, current.column
        ));
    }
    out.push_str("lifecycle: ");
    out.push_str(&style.status("preserved", "preserved"));
    out.push_str(" (expires and review_after unchanged)\n");
    if let Some(path) = report.mode.written_path {
        out.push_str(&format!("\nUpdated policy at `{path}`.\n"));
    } else {
        out.push_str(
            "\nNo files were changed. Pass --write to record the operator-approved refresh.\n",
        );
    }
    out.push('\n');
    out.push_str(CLAIM_BOUNDARY_TEXT);
    out.push('\n');
    out
}

pub fn render_refresh_json(report: RefreshReport<'_>) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    push_json_fixed_artifact_preamble(&mut out, REFRESH_ARTIFACT, report.inventory);
    out.push_str("  \"mode\": {\n");
    out.push_str(&format!(
        "    \"dry_run\": {},\n",
        bool_json(!report.mode.write_requested)
    ));
    out.push_str(&format!(
        "    \"write_requested\": {},\n",
        bool_json(report.mode.write_requested)
    ));
    out.push_str(&format!(
        "    \"explicit_dry_run\": {},\n",
        bool_json(report.mode.explicit_dry_run)
    ));
    out.push_str(&format!(
        "    \"written_path\": {}\n",
        option_json(report.mode.written_path)
    ));
    out.push_str("  },\n");
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!(
        "    \"entry_id\": \"{}\",\n",
        json_escape(&report.entry.id)
    ));
    out.push_str(&format!(
        "    \"drift_message\": \"{}\",\n",
        json_escape(report.drift_message)
    ));
    out.push_str("    \"lifecycle_preserved\": true,\n");
    out.push_str(&format!(
        "    \"operator_approved\": {}\n",
        bool_json(report.mode.write_requested)
    ));
    out.push_str("  },\n");
    out.push_str("  \"previous_last_seen\": ");
    out.push_str(&render_last_seen_json(
        report.previous_last_seen.as_ref(),
        "  ",
    ));
    out.push_str(",\n");
    out.push_str("  \"refreshed_last_seen\": ");
    out.push_str(&render_last_seen_json(
        report.entry.last_seen.as_ref(),
        "  ",
    ));
    out.push_str(",\n");
    out.push_str("  \"matched_finding\": \"");
    out.push_str(&json_escape(&finding_location_text(report.finding)));
    out.push_str("\",\n");
    out.push_str("  \"mutation_receipt\": ");
    out.push_str(&render_mutation_receipt_json(
        &report.mutation_receipt,
        "  ",
    ));
    out.push_str("}\n");
    out
}
