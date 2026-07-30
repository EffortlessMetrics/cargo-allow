use crate::contracts::MIGRATE_ARTIFACT;
use crate::evidence_repair::evidence_repair_queues_from_counts;
use crate::json::{bool_json, push_json_fixed_artifact_preamble};
use crate::migrate_closeout::{
    MigrateCloseoutInput, append_migrate_closeout_human, append_migrate_closeout_json,
    migrate_closeout_from_input,
};
use crate::migrate_closeout_queues::migrate_follow_up_queues_for_legacy;
use crate::{
    CLAIM_BOUNDARY_TEXT, MigrateReport, MutationReceipt, Style, render_mutation_receipt_json,
};
use allow_core::json_escape;

const BROKEN_EVIDENCE_LINK_COMMAND: &str =
    "cargo-allow worklist --item-kind broken_evidence_link --format json";
const UNSAFE_BROKEN_EVIDENCE_LINK_COMMAND: &str =
    "cargo-allow worklist --item-kind broken_evidence_link --kind unsafe --format json";
const WEAK_EVIDENCE_REFERENCE_COMMAND: &str =
    "cargo-allow worklist --item-kind weak_evidence_reference --format json";
const UNSAFE_WEAK_EVIDENCE_REFERENCE_COMMAND: &str =
    "cargo-allow worklist --item-kind weak_evidence_reference --kind unsafe --format json";

pub fn render_migrate_human(
    report: MigrateReport<'_>,
    closeout_input: MigrateCloseoutInput<'_>,
) -> String {
    render_migrate_human_styled(report, closeout_input, Style::PLAIN)
}

pub fn render_migrate_human_styled(
    report: MigrateReport<'_>,
    closeout_input: MigrateCloseoutInput<'_>,
    style: Style,
) -> String {
    let closeout = migrate_closeout_from_input(closeout_input);
    let mut out = String::new();
    out.push_str("cargo-allow migrate summary\n");
    out.push_str(&format!("input_kind: {}\n", report.input_kind));
    out.push_str(&format!("input: {}\n", report.input_path));
    out.push_str(&format!("output: {}\n", report.output_path));
    out.push_str(&format!("force: {}\n", report.force));
    out.push_str("migration posture: ");
    if closeout.legacy_retirement.ready {
        out.push_str(&style.status("complete", "ready"));
    } else {
        out.push_str(&style.status("baseline_debt", "blocked"));
    }
    out.push('\n');
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
    out.push_str(&format!("evidence_entries: {}\n", report.evidence_entries));
    out.push_str(&format!(
        "entries_with_links: {}\n",
        report.entries_with_links
    ));
    out.push_str(&format!("link_entries: {}\n", report.link_entries));
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
    append_migrate_follow_up_queues_human(
        report,
        &mut out,
        &closeout_input.legacy_compat_kind_ids(),
    );
    append_migrate_evidence_repair_queues_human(report, &mut out);
    append_migrate_closeout_human(&closeout, &mut out);
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
    out.push_str(report.notes);
    if !report.notes.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(CLAIM_BOUNDARY_TEXT);
    out.push('\n');
    out
}

fn append_migrate_follow_up_queues_human(
    report: MigrateReport<'_>,
    out: &mut String,
    legacy_compat_kinds: &[&str],
) {
    let queues = migrate_follow_up_queues_for_legacy(report, legacy_compat_kinds);
    if queues.is_empty() {
        return;
    }
    out.push_str("follow_up_queues:\n");
    for queue in queues {
        out.push_str(&format!("  {}\n", queue.command));
    }
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
        commands.push(BROKEN_EVIDENCE_LINK_COMMAND);
    }
    if report.unsafe_broken_evidence_links.unwrap_or(0) > 0 {
        commands.push(UNSAFE_BROKEN_EVIDENCE_LINK_COMMAND);
    }
    if report.weak_evidence_references.unwrap_or(0) > 0
        || report.unsafe_weak_evidence_references.unwrap_or(0) > 0
    {
        commands.push(WEAK_EVIDENCE_REFERENCE_COMMAND);
    }
    if report.unsafe_weak_evidence_references.unwrap_or(0) > 0 {
        commands.push(UNSAFE_WEAK_EVIDENCE_REFERENCE_COMMAND);
    }
    commands
}

pub fn render_migrate_json(
    report: MigrateReport<'_>,
    closeout_input: MigrateCloseoutInput<'_>,
    mutation_receipt: &MutationReceipt<'_>,
) -> String {
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
    summary_tail.push(format!(
        "    \"evidence_entries\": {}",
        report.evidence_entries
    ));
    summary_tail.push(format!(
        "    \"entries_with_links\": {}",
        report.entries_with_links
    ));
    summary_tail.push(format!("    \"link_entries\": {}", report.link_entries));
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
    append_migrate_follow_up_queues_json(
        report,
        &mut out,
        &closeout_input.legacy_compat_kind_ids(),
    );
    append_migrate_evidence_repair_queues_json(report, &mut out);
    let closeout = migrate_closeout_from_input(closeout_input);
    append_migrate_closeout_json(&closeout, &mut out);
    out.push_str("  \"mutation_receipt\": ");
    out.push_str(&render_mutation_receipt_json(mutation_receipt, "  "));
    out.push_str(&format!(
        ",\n  \"notes\": \"{}\"\n",
        json_escape(report.notes)
    ));
    out.push_str("}\n");
    out
}

fn append_migrate_follow_up_queues_json(
    report: MigrateReport<'_>,
    out: &mut String,
    legacy_compat_kinds: &[&str],
) {
    let queues = migrate_follow_up_queues_for_legacy(report, legacy_compat_kinds);
    if queues.is_empty() {
        return;
    }

    out.push_str("  \"follow_up_queues\": [\n");
    for (index, queue) in queues.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str("    {\n");
        out.push_str(&format!(
            "      \"signal\": \"{}\",\n",
            json_escape(queue.signal)
        ));
        out.push_str(&format!(
            "      \"label\": \"{}\",\n",
            json_escape(queue.label)
        ));
        out.push_str(&format!(
            "      \"route_kind\": \"{}\",\n",
            json_escape(queue.route_kind)
        ));
        out.push_str(&format!(
            "      \"item_kind\": \"{}\",\n",
            json_escape(queue.item_kind)
        ));
        out.push_str(&format!("      \"count\": {},\n", queue.count));
        out.push_str(&format!(
            "      \"command\": \"{}\"\n",
            json_escape(queue.command)
        ));
        out.push_str("    }");
    }
    out.push_str("\n  ],\n");
}

fn append_migrate_evidence_repair_queues_json(report: MigrateReport<'_>, out: &mut String) {
    let queues = migrate_evidence_repair_queues(report);
    if queues.is_empty() {
        return;
    }

    out.push_str("  \"evidence_repair_queues\": [\n");
    for (index, queue) in queues.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str("    {\n");
        out.push_str(&format!(
            "      \"signal\": \"{}\",\n",
            json_escape(queue.signal)
        ));
        out.push_str(&format!(
            "      \"label\": \"{}\",\n",
            json_escape(queue.label)
        ));
        out.push_str(&format!(
            "      \"route_kind\": \"{}\",\n",
            json_escape(queue.route_kind)
        ));
        out.push_str(&format!(
            "      \"item_kind\": \"{}\",\n",
            json_escape(queue.item_kind)
        ));
        out.push_str(&format!("      \"count\": {},\n", queue.count));
        out.push_str(&format!(
            "      \"unsafe_count\": {},\n",
            queue.unsafe_count
        ));
        out.push_str(&format!(
            "      \"command\": \"{}\"",
            json_escape(queue.command)
        ));
        if let Some(unsafe_command) = queue.unsafe_command {
            out.push_str(",\n");
            out.push_str(&format!(
                "      \"unsafe_command\": \"{}\"",
                json_escape(unsafe_command)
            ));
        }
        out.push('\n');
        out.push_str("    }");
    }
    out.push_str("\n  ],\n");
}

pub(crate) fn migrate_evidence_repair_queues(
    report: MigrateReport<'_>,
) -> Vec<MigrateEvidenceRepairQueue> {
    let mut queues = Vec::new();
    let broken_count = report.broken_evidence_links.unwrap_or(0);
    let unsafe_broken_count = report.unsafe_broken_evidence_links.unwrap_or(0);
    let weak_count = report.weak_evidence_references.unwrap_or(0);
    let unsafe_weak_count = report.unsafe_weak_evidence_references.unwrap_or(0);
    let broken_total = broken_count.max(unsafe_broken_count);
    let weak_total = weak_count.max(unsafe_weak_count);
    for queue in evidence_repair_queues_from_counts(broken_total, 0, weak_total, 0) {
        let Some(item_kind) = queue.item_kind else {
            continue;
        };
        let (count, unsafe_count) = match item_kind {
            "broken_evidence_link" => (broken_total, unsafe_broken_count),
            "weak_evidence_reference" => (weak_total, unsafe_weak_count),
            _ => (queue.count, 0),
        };
        let unsafe_command = match item_kind {
            "broken_evidence_link" if unsafe_count > 0 => Some(UNSAFE_BROKEN_EVIDENCE_LINK_COMMAND),
            "weak_evidence_reference" if unsafe_count > 0 => {
                Some(UNSAFE_WEAK_EVIDENCE_REFERENCE_COMMAND)
            }
            _ => None,
        };
        queues.push(MigrateEvidenceRepairQueue {
            signal: queue.signal,
            label: queue.label,
            route_kind: "worklist_item_kind",
            item_kind,
            count,
            unsafe_count,
            command: migrate_evidence_repair_command(item_kind),
            unsafe_command,
        });
    }
    queues
}

fn migrate_evidence_repair_command(item_kind: &str) -> &'static str {
    match item_kind {
        "broken_evidence_link" => BROKEN_EVIDENCE_LINK_COMMAND,
        "weak_evidence_reference" => WEAK_EVIDENCE_REFERENCE_COMMAND,
        _ => "cargo-allow worklist --format json",
    }
}

pub(crate) struct MigrateFollowUpQueue {
    pub(crate) signal: &'static str,
    pub(crate) label: &'static str,
    pub(crate) route_kind: &'static str,
    pub(crate) item_kind: &'static str,
    pub(crate) count: usize,
    pub(crate) command: &'static str,
}

pub(crate) struct MigrateEvidenceRepairQueue {
    pub(crate) signal: &'static str,
    pub(crate) label: &'static str,
    pub(crate) route_kind: &'static str,
    pub(crate) item_kind: &'static str,
    pub(crate) count: usize,
    pub(crate) unsafe_count: usize,
    pub(crate) command: &'static str,
    pub(crate) unsafe_command: Option<&'static str>,
}
