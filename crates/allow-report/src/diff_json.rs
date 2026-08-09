use allow_core::json_escape;

use crate::DiffReport;
use crate::diff_movement::append_movement_summary_json;
use crate::diff_posture::{diff_evidence_delta_summary, diff_structural_delta_summary};
use crate::explain_json::structural_identity_json;
use crate::json::{json_string_array, option_json};
use crate::ledger_posture::coverage_movement_from_canonical_fields;
use crate::{REPORT_COMMAND_DIFF, REPORT_SCHEMA_ID};

pub fn render_diff_json_with_posture(report_json: &str, report: DiffReport<'_>) -> Option<String> {
    if !is_diff_report_artifact(report_json) {
        return None;
    }
    let diff_json = render_diff_posture_json(report);
    // Splice the diff field into the report object by locating the top-level
    // closing brace while respecting JSON string literals, so a `}` inside a
    // string value (e.g. a finding message) does not corrupt the splice. The
    // previous `strip_suffix('}')` text surgery stripped the wrong brace in
    // that case and produced invalid JSON (#1852).
    let top_level_close = top_level_object_close(report_json)?;
    let (prefix, suffix) = report_json
        .split_at_checked(top_level_close)
        .unwrap_or((report_json, ""));
    let mut out = format!("{prefix},\n  \"diff\": {diff_json}{suffix}");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Some(out)
}

/// Find the index of the `}` that closes the top-level JSON object, scanning
/// past string literals (so a `}` inside a string is not mistaken for the
/// close). Returns `None` if the document is not a single balanced object.
fn top_level_object_close(json: &str) -> Option<usize> {
    let bytes = json.as_bytes();
    let mut depth: i64 = 0;
    let mut in_string = false;
    let mut escape = false;
    for (index, &byte) in bytes.iter().enumerate() {
        if in_string {
            if escape {
                escape = false;
            } else if byte == b'\\' {
                escape = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn is_diff_report_artifact(report_json: &str) -> bool {
    contains_json_string_field(report_json, "schema_id", REPORT_SCHEMA_ID)
        && contains_json_string_field(report_json, "command", REPORT_COMMAND_DIFF)
}

/// Check whether a JSON string field is present in the document HEAD (first
/// 2 KiB) rather than anywhere in the body. This prevents false positives
/// where a finding message or evidence string contains the schema_id text
/// (#1853).
fn contains_json_string_field(json: &str, field: &str, value: &str) -> bool {
    let head = json.len().min(2048);
    let window = json.get(..head).unwrap_or(json);
    let spaced = format!("\"{field}\": \"{value}\"");
    let compact = format!("\"{field}\":\"{value}\"");
    window.contains(&spaced) || window.contains(&compact)
}

pub(crate) fn render_diff_posture_json(report: DiffReport<'_>) -> String {
    render_diff_posture_json_with_evidence_health(report, 0, 0, 0)
}

pub(crate) fn render_diff_posture_json_with_evidence_health(
    report: DiffReport<'_>,
    broken_evidence_links: usize,
    missing_evidence: usize,
    weak_evidence_references: usize,
) -> String {
    render_diff_posture_json_with_evidence_health_and_context(
        report,
        broken_evidence_links,
        missing_evidence,
        weak_evidence_references,
        crate::ReportContext::default(),
    )
}

pub(crate) fn render_diff_posture_json_with_evidence_health_and_context(
    report: DiffReport<'_>,
    broken_evidence_links: usize,
    missing_evidence: usize,
    weak_evidence_references: usize,
    context: crate::ReportContext<'_>,
) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "    \"net_posture\": \"{}\",\n",
        json_escape(report.net_posture)
    ));
    out.push_str(&format!(
        "    \"reviewer_action\": \"{}\",\n",
        json_escape(report.reviewer_action)
    ));
    if let Some(diff) = context.diff_analysis {
        out.push_str("    \"diff_analysis\": {");
        out.push_str(&format!(
            "\"result_class\": \"{}\", \"base_inventory_complete\": {}, \"base_scanner_complete\": {}, \"head_inventory_complete\": {}, \"head_scanner_complete\": {}, \"introduced\": {}, \"retained\": {}, \"removed\": {}}},\n",
            json_escape(diff.result_class),
            crate::json::bool_json(diff.base_inventory_complete),
            crate::json::bool_json(diff.base_scanner_complete),
            crate::json::bool_json(diff.head_inventory_complete),
            crate::json::bool_json(diff.head_scanner_complete),
            diff.introduced,
            diff.retained,
            diff.removed,
        ));
    }
    append_movement_summary_json(&mut out, report.ledger_movement);
    out.push_str("    \"summary\": {\n");
    out.push_str(&format!(
        "      \"current_failures\": {},\n",
        report.summary.current_failures
    ));
    if broken_evidence_links > 0 {
        out.push_str(&format!(
            "      \"broken_evidence_links\": {},\n",
            broken_evidence_links
        ));
    }
    if missing_evidence > 0 {
        out.push_str(&format!(
            "      \"missing_evidence\": {},\n",
            missing_evidence
        ));
    }
    if weak_evidence_references > 0 {
        out.push_str(&format!(
            "      \"weak_evidence_references\": {},\n",
            weak_evidence_references
        ));
    }
    let structural_delta = diff_structural_delta_summary(report.policy_changes);
    if structural_delta.scope_broadened > 0 {
        out.push_str(&format!(
            "      \"scope_broadened\": {},\n",
            structural_delta.scope_broadened
        ));
    }
    if structural_delta.scope_changed > 0 {
        out.push_str(&format!(
            "      \"scope_changed\": {},\n",
            structural_delta.scope_changed
        ));
    }
    if structural_delta.scope_narrowed > 0 {
        out.push_str(&format!(
            "      \"scope_narrowed\": {},\n",
            structural_delta.scope_narrowed
        ));
    }
    if structural_delta.selector_changed > 0 {
        out.push_str(&format!(
            "      \"selector_changed\": {},\n",
            structural_delta.selector_changed
        ));
    }
    if structural_delta.selector_precision_decreased > 0 {
        out.push_str(&format!(
            "      \"selector_precision_decreased\": {},\n",
            structural_delta.selector_precision_decreased
        ));
    }
    if structural_delta.selector_precision_increased > 0 {
        out.push_str(&format!(
            "      \"selector_precision_increased\": {},\n",
            structural_delta.selector_precision_increased
        ));
    }
    let evidence_delta = diff_evidence_delta_summary(report.policy_changes);
    if evidence_delta.evidence_added > 0 {
        out.push_str(&format!(
            "      \"evidence_added\": {},\n",
            evidence_delta.evidence_added
        ));
    }
    if evidence_delta.weak_evidence_added > 0 {
        out.push_str(&format!(
            "      \"weak_evidence_added\": {},\n",
            evidence_delta.weak_evidence_added
        ));
    }
    if evidence_delta.broken_evidence_added > 0 {
        out.push_str(&format!(
            "      \"broken_evidence_added\": {},\n",
            evidence_delta.broken_evidence_added
        ));
    }
    if evidence_delta.evidence_removed > 0 {
        out.push_str(&format!(
            "      \"evidence_removed\": {},\n",
            evidence_delta.evidence_removed
        ));
    }
    if evidence_delta.evidence_removal_failures > 0 {
        out.push_str(&format!(
            "      \"evidence_removal_failures\": {},\n",
            evidence_delta.evidence_removal_failures
        ));
    }
    if evidence_delta.evidence_removal_review_items > 0 {
        out.push_str(&format!(
            "      \"evidence_removal_review_items\": {},\n",
            evidence_delta.evidence_removal_review_items
        ));
    }
    if evidence_delta.evidence_removal_improvements > 0 {
        out.push_str(&format!(
            "      \"evidence_removal_improvements\": {},\n",
            evidence_delta.evidence_removal_improvements
        ));
    }
    if evidence_delta.link_added > 0 {
        out.push_str(&format!(
            "      \"link_added\": {},\n",
            evidence_delta.link_added
        ));
    }
    if evidence_delta.weak_link_added > 0 {
        out.push_str(&format!(
            "      \"weak_link_added\": {},\n",
            evidence_delta.weak_link_added
        ));
    }
    if evidence_delta.broken_link_added > 0 {
        out.push_str(&format!(
            "      \"broken_link_added\": {},\n",
            evidence_delta.broken_link_added
        ));
    }
    if evidence_delta.link_removed > 0 {
        out.push_str(&format!(
            "      \"link_removed\": {},\n",
            evidence_delta.link_removed
        ));
    }
    if evidence_delta.link_removal_failures > 0 {
        out.push_str(&format!(
            "      \"link_removal_failures\": {},\n",
            evidence_delta.link_removal_failures
        ));
    }
    if evidence_delta.link_removal_review_items > 0 {
        out.push_str(&format!(
            "      \"link_removal_review_items\": {},\n",
            evidence_delta.link_removal_review_items
        ));
    }
    if evidence_delta.link_removal_improvements > 0 {
        out.push_str(&format!(
            "      \"link_removal_improvements\": {},\n",
            evidence_delta.link_removal_improvements
        ));
    }
    out.push_str(&format!(
        "      \"new_findings\": {},\n",
        report.summary.new_findings
    ));
    out.push_str(&format!(
        "      \"removed_findings\": {},\n",
        report.summary.removed_findings
    ));
    out.push_str(&format!(
        "      \"policy_failures\": {},\n",
        report.summary.policy_failures
    ));
    out.push_str(&format!(
        "      \"policy_review_items\": {},\n",
        report.summary.policy_review_items
    ));
    out.push_str(&format!(
        "      \"policy_improvements\": {}\n",
        report.summary.policy_improvements
    ));
    out.push_str("    },\n");
    out.push_str("    \"finding_changes\": [\n");
    for (index, change) in report.finding_changes.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str("      {");
        out.push_str(&format!("\"change\": \"{}\", ", json_escape(change.change)));
        append_row_classification_json(
            &mut out,
            change.movement,
            change.posture_delta,
            change.changed_in_diff,
        );
        append_optional_subject_json(&mut out, change.subject);
        append_optional_provenance_json(&mut out, change.allow_id, change.ledger_id, change.lane);
        out.push_str(&format!("\"key\": \"{}\", ", json_escape(change.key)));
        out.push_str(&format!("\"kind\": \"{}\", ", json_escape(change.kind)));
        out.push_str(&format!("\"family\": {}, ", option_json(change.family)));
        out.push_str(&format!("\"path\": \"{}\"", json_escape(change.path)));
        if let Some(line) = change.line {
            out.push_str(&format!(", \"line\": {line}"));
        }
        if let Some(column) = change.column {
            out.push_str(&format!(", \"column\": {column}"));
        }
        if let Some(source_package) = change.source_package {
            out.push_str(&format!(
                ", \"source_package\": \"{}\"",
                json_escape(source_package)
            ));
        }
        if let Some(identity) = change.identity {
            out.push_str(", \"identity\": ");
            out.push_str(&structural_identity_json(identity, "    "));
        }
        out.push('}');
    }
    out.push_str("\n    ],\n");
    out.push_str("    \"policy_changes\": [\n");
    for (index, change) in report.policy_changes.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str("      {");
        out.push_str(&format!(
            "\"severity\": \"{}\", ",
            json_escape(change.severity)
        ));
        append_row_classification_json(
            &mut out,
            change.movement,
            change.posture_delta,
            change.changed_in_diff,
        );
        append_optional_subject_json(&mut out, change.subject);
        out.push_str(&format!(
            "\"allow_id\": \"{}\", ",
            json_escape(change.allow_id)
        ));
        append_optional_provenance_json(&mut out, None, change.ledger_id, change.lane);
        out.push_str(&format!("\"kind\": \"{}\", ", json_escape(change.kind)));
        out.push_str(&format!("\"message\": \"{}\"", json_escape(change.message)));
        if let Some(exception_identity) = change.exception_identity {
            out.push_str(", ");
            out.push_str(&format!(
                "\"exception_identity\": {{\"field\": \"{}\", \"before\": {}, \"after\": {}}}",
                json_escape(exception_identity.field),
                option_json(exception_identity.before),
                option_json(exception_identity.after)
            ));
        }
        if let Some(selector_identity) = change.selector_identity {
            out.push_str(", ");
            out.push_str(&format!(
                "\"selector_identity\": {{\"changed_fields\": {}}}",
                json_string_array(selector_identity.changed_fields)
            ));
        }
        if let Some(selector_precision) = change.selector_precision {
            out.push_str(", ");
            out.push_str(&format!(
                "\"selector_precision\": {{\"before\": {}, \"after\": {}, \"removed_fields\": {}, \"added_fields\": {}}}",
                selector_precision.before,
                selector_precision.after,
                json_string_array(selector_precision.removed_fields),
                json_string_array(selector_precision.added_fields)
            ));
        }
        if let Some(scope) = change.scope {
            out.push_str(", ");
            out.push_str(&format!(
                "\"scope\": {{\"field\": \"{}\", \"before\": {}, \"after\": {}}}",
                json_escape(scope.field),
                option_json(scope.before),
                option_json(scope.after)
            ));
        }
        if let Some(limit) = change.occurrence_limit {
            out.push_str(", ");
            out.push_str(&format!(
                "\"occurrence_limit\": {{\"before\": {}, \"after\": {}}}",
                option_u32_json(limit.before),
                option_u32_json(limit.after)
            ));
        }
        if let Some(lifecycle) = change.lifecycle {
            out.push_str(", ");
            out.push_str(&format!(
                "\"lifecycle\": {{\"field\": \"{}\", \"before\": {}, \"after\": {}}}",
                json_escape(lifecycle.field),
                option_json(lifecycle.before),
                option_json(lifecycle.after)
            ));
        }
        if let Some(evidence) = change.evidence {
            out.push_str(", ");
            out.push_str(&format!(
                "\"evidence\": {{\"field\": \"{}\", \"removed\": {}, \"added\": {}}}",
                json_escape(evidence.field),
                json_string_array(evidence.removed),
                json_string_array(evidence.added)
            ));
        }
        if let Some(metadata) = change.metadata {
            out.push_str(", ");
            out.push_str(&format!(
                "\"metadata\": {{\"field\": \"{}\", \"before\": {}, \"after\": {}}}",
                json_escape(metadata.field),
                option_json(metadata.before),
                option_json(metadata.after)
            ));
        }
        if let Some(requirement) = change.requirement {
            out.push_str(", ");
            out.push_str(&format!(
                "\"requirement\": {{\"field\": \"{}\", \"before\": {}, \"after\": {}}}",
                json_escape(requirement.field),
                requirement.before,
                requirement.after
            ));
        }
        if let Some(policy_status) = change.policy_status {
            out.push_str(", ");
            out.push_str(&format!(
                "\"policy_status\": {{\"before\": {}, \"after\": {}}}",
                option_json(policy_status.before),
                option_json(policy_status.after)
            ));
        }
        out.push('}');
    }
    out.push_str("\n    ]\n");
    out.push_str("  }");
    out
}

fn option_u32_json(value: Option<u32>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn append_row_classification_json(
    out: &mut String,
    movement: &str,
    posture_delta: &str,
    changed_in_diff: bool,
) {
    let coverage_movement =
        coverage_movement_from_canonical_fields(movement, posture_delta, changed_in_diff)
            .unwrap_or("retained");
    out.push_str(&format!(
        "\"movement\": \"{}\", \"posture_delta\": \"{}\", \"changed_in_diff\": {}, \"coverage_movement\": \"{}\", ",
        json_escape(movement),
        json_escape(posture_delta),
        changed_in_diff,
        json_escape(coverage_movement)
    ));
}

fn append_optional_subject_json(out: &mut String, subject: Option<&str>) {
    if let Some(subject) = subject {
        out.push_str(&format!("\"subject\": \"{}\", ", json_escape(subject)));
    }
}

fn append_optional_provenance_json(
    out: &mut String,
    allow_id: Option<&str>,
    ledger_id: Option<&str>,
    lane: Option<&str>,
) {
    if let Some(allow_id) = allow_id {
        out.push_str(&format!("\"allow_id\": \"{}\", ", json_escape(allow_id)));
    }
    if let Some(ledger_id) = ledger_id {
        out.push_str(&format!("\"ledger_id\": \"{}\", ", json_escape(ledger_id)));
    }
    if let Some(lane) = lane {
        out.push_str(&format!("\"lane\": \"{}\", ", json_escape(lane)));
    }
}
