use allow_core::json_escape;

use crate::DiffReport;
use crate::json::{json_string_array, option_json};
use crate::{REPORT_COMMAND_DIFF, REPORT_SCHEMA_ID};

pub fn render_diff_json_with_posture(report_json: &str, report: DiffReport<'_>) -> Option<String> {
    if !is_diff_report_artifact(report_json) {
        return None;
    }
    let diff_json = render_diff_posture_json(report);
    let trimmed = report_json.trim_end();
    trimmed
        .strip_suffix('}')
        .map(|prefix| format!("{prefix},\n  \"diff\": {diff_json}\n}}\n"))
}

fn is_diff_report_artifact(report_json: &str) -> bool {
    contains_json_string_field(report_json, "schema_id", REPORT_SCHEMA_ID)
        && contains_json_string_field(report_json, "command", REPORT_COMMAND_DIFF)
}

fn contains_json_string_field(json: &str, field: &str, value: &str) -> bool {
    let spaced = format!("\"{field}\": \"{value}\"");
    let compact = format!("\"{field}\":\"{value}\"");
    json.contains(&spaced) || json.contains(&compact)
}

pub(crate) fn render_diff_posture_json(report: DiffReport<'_>) -> String {
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
    out.push_str("    \"summary\": {\n");
    out.push_str(&format!(
        "      \"current_failures\": {},\n",
        report.summary.current_failures
    ));
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
        out.push_str(&format!("\"key\": \"{}\", ", json_escape(change.key)));
        out.push_str(&format!("\"kind\": \"{}\", ", json_escape(change.kind)));
        out.push_str(&format!("\"family\": {}, ", option_json(change.family)));
        out.push_str(&format!("\"path\": \"{}\"", json_escape(change.path)));
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
        out.push_str(&format!(
            "\"allow_id\": \"{}\", ",
            json_escape(change.allow_id)
        ));
        out.push_str(&format!("\"kind\": \"{}\", ", json_escape(change.kind)));
        out.push_str(&format!("\"message\": \"{}\"", json_escape(change.message)));
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
        out.push('}');
    }
    out.push_str("\n    ]\n");
    out.push_str("  }");
    out
}
