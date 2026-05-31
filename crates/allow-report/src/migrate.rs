use crate::contracts::MIGRATE_ARTIFACT;
use crate::json::{bool_json, push_json_fixed_artifact_preamble};
use crate::{CLAIM_BOUNDARY_TEXT, MigrateReport};
use allow_core::json_escape;

pub fn render_migrate_human(report: MigrateReport<'_>) -> String {
    let mut out = String::new();
    out.push_str("cargo-allow migrate summary\n");
    out.push_str(&format!("input_kind: {}\n", report.input_kind));
    out.push_str(&format!("input: {}\n", report.input_path));
    out.push_str(&format!("output: {}\n", report.output_path));
    out.push_str(&format!("force: {}\n", report.force));
    out.push_str(&format!("allow_entries: {}\n", report.allow_entries));
    out.push_str(&format!("baseline_debt: {}\n", report.baseline_debt));
    out.push_str(&format!("unsafe_entries: {}\n", report.unsafe_entries));
    out.push_str(&format!(
        "inventory: {}/{} via {}{}\n",
        report.inventory.scope,
        report.inventory.scanner,
        report.inventory.source,
        migrate_inventory_files_suffix(report.inventory)
    ));
    if let Some(root) = report.inventory.root {
        out.push_str(&format!("source_tree_root: {root}\n"));
    }
    out.push_str(report.notes);
    if !report.notes.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(CLAIM_BOUNDARY_TEXT);
    out.push('\n');
    out
}

fn migrate_inventory_files_suffix(inventory: crate::InventoryContext<'_>) -> String {
    inventory
        .files_scanned
        .map(|files| format!("; files scanned: {files}"))
        .unwrap_or_default()
}

pub fn render_migrate_json(report: MigrateReport<'_>) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    push_json_fixed_artifact_preamble(&mut out, MIGRATE_ARTIFACT, report.inventory);
    out.push_str("  \"input\": {\n");
    out.push_str(&format!(
        "    \"kind\": \"{}\",\n",
        json_escape(report.input_kind)
    ));
    out.push_str(&format!(
        "    \"path\": \"{}\"\n",
        json_escape(report.input_path)
    ));
    out.push_str("  },\n");
    out.push_str("  \"output\": {\n");
    out.push_str(&format!(
        "    \"path\": \"{}\",\n",
        json_escape(report.output_path)
    ));
    out.push_str(&format!("    \"force\": {}\n", bool_json(report.force)));
    out.push_str("  },\n");
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!(
        "    \"allow_entries\": {},\n",
        report.allow_entries
    ));
    out.push_str(&format!(
        "    \"baseline_debt\": {},\n",
        report.baseline_debt
    ));
    out.push_str(&format!(
        "    \"unsafe_entries\": {},\n",
        report.unsafe_entries
    ));
    out.push_str(&format!(
        "    \"entries_with_evidence\": {}\n",
        report.entries_with_evidence
    ));
    out.push_str("  },\n");
    out.push_str(&format!("  \"notes\": \"{}\"\n", json_escape(report.notes)));
    out.push_str("}\n");
    out
}
