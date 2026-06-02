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
        "lint_exception_entries: {}\n",
        report.lint_exception_entries
    ));
    out.push_str(&format!(
        "entries_with_evidence: {}\n",
        report.entries_with_evidence
    ));
    if let Some(count) = report.broken_evidence_links.filter(|count| *count > 0) {
        out.push_str(&format!("broken_evidence_links: {count}\n"));
    }
    if let Some(count) = report
        .unsafe_broken_evidence_links
        .filter(|count| *count > 0)
    {
        out.push_str(&format!("unsafe_broken_evidence_links: {count}\n"));
    }
    if let Some(count) = report.weak_evidence_references.filter(|count| *count > 0) {
        out.push_str(&format!("weak_evidence_references: {count}\n"));
    }
    if let Some(count) = report
        .unsafe_weak_evidence_references
        .filter(|count| *count > 0)
    {
        out.push_str(&format!("unsafe_weak_evidence_references: {count}\n"));
    }
    append_migrate_evidence_repair_queues_human(report, &mut out);
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

fn append_migrate_evidence_repair_queues_human(report: MigrateReport<'_>, out: &mut String) {
    let commands = migrate_evidence_repair_commands(report);
    if commands.is_empty() {
        return;
    }
    out.push_str("evidence_repair_queues:\n");
    for command in commands {
        out.push_str(&format!("  {command}\n"));
    }
}

fn migrate_evidence_repair_commands(report: MigrateReport<'_>) -> Vec<&'static str> {
    let mut commands = Vec::new();
    if report.broken_evidence_links.unwrap_or(0) > 0
        || report.unsafe_broken_evidence_links.unwrap_or(0) > 0
    {
        commands.push("cargo-allow worklist --item-kind broken_evidence_link --format json");
    }
    if report.weak_evidence_references.unwrap_or(0) > 0
        || report.unsafe_weak_evidence_references.unwrap_or(0) > 0
    {
        commands.push("cargo-allow worklist --item-kind weak_evidence_reference --format json");
    }
    commands
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
        "    \"lint_exception_entries\": {},\n",
        report.lint_exception_entries
    ));
    let mut summary_tail = vec![format!(
        "    \"entries_with_evidence\": {}",
        report.entries_with_evidence
    )];
    for (name, count) in [
        ("broken_evidence_links", report.broken_evidence_links),
        (
            "unsafe_broken_evidence_links",
            report.unsafe_broken_evidence_links,
        ),
        ("weak_evidence_references", report.weak_evidence_references),
        (
            "unsafe_weak_evidence_references",
            report.unsafe_weak_evidence_references,
        ),
    ] {
        if let Some(count) = count.filter(|count| *count > 0) {
            summary_tail.push(format!("    \"{name}\": {count}"));
        }
    }
    out.push_str(&summary_tail.join(",\n"));
    out.push('\n');
    out.push_str("  },\n");
    out.push_str(&format!("  \"notes\": \"{}\"\n", json_escape(report.notes)));
    out.push_str("}\n");
    out
}
