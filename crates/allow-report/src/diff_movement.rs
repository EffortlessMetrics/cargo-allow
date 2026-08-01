use crate::DiffLedgerMovementSummary;

pub fn append_movement_summary_json(out: &mut String, summary: DiffLedgerMovementSummary) {
    out.push_str("    \"movement\": {\n");
    out.push_str(&format!(
        "      \"introduced\": {},\n",
        summary.movement.introduced
    ));
    out.push_str(&format!(
        "      \"retained\": {},\n",
        summary.movement.retained
    ));
    out.push_str(&format!(
        "      \"removed\": {}\n",
        summary.movement.removed
    ));
    out.push_str("    },\n");
    out.push_str("    \"posture_delta\": {\n");
    out.push_str(&format!(
        "      \"improved\": {},\n",
        summary.posture_delta.improved
    ));
    out.push_str(&format!(
        "      \"worsened\": {},\n",
        summary.posture_delta.worsened
    ));
    out.push_str(&format!(
        "      \"review_required\": {},\n",
        summary.posture_delta.review_required
    ));
    out.push_str(&format!(
        "      \"unchanged\": {}\n",
        summary.posture_delta.unchanged
    ));
    out.push_str("    },\n");
}

pub fn append_movement_summary_human(out: &mut String, summary: DiffLedgerMovementSummary) {
    append_movement_summary_human_styled(out, summary, crate::Style::PLAIN);
}

pub fn append_movement_summary_human_styled(
    out: &mut String,
    summary: DiffLedgerMovementSummary,
    style: crate::Style,
) {
    out.push_str("  movement:\n");
    out.push_str(&format!(
        "    {}: {}\n",
        style_diff_status(style, "introduced"),
        summary.movement.introduced
    ));
    out.push_str(&format!(
        "    {}: {}\n",
        style_diff_status(style, "retained"),
        summary.movement.retained
    ));
    out.push_str(&format!(
        "    {}: {}\n",
        style_diff_status(style, "removed"),
        summary.movement.removed
    ));
    out.push_str("  posture_delta:\n");
    out.push_str(&format!(
        "    {}: {}\n",
        style_diff_status(style, "improved"),
        summary.posture_delta.improved
    ));
    out.push_str(&format!(
        "    {}: {}\n",
        style_diff_status(style, "worsened"),
        summary.posture_delta.worsened
    ));
    out.push_str(&format!(
        "    {}: {}\n",
        style_diff_status(style, "review_required"),
        summary.posture_delta.review_required
    ));
    out.push_str(&format!(
        "    {}: {}\n",
        style_diff_status(style, "unchanged"),
        summary.posture_delta.unchanged
    ));
}

fn style_diff_status(style: crate::Style, status: &str) -> String {
    match status {
        "introduced" | "worsened" => style.blocking(status),
        "removed" | "improved" => style.ok(status),
        _ => style.advisory(status),
    }
}

pub fn append_movement_summary_markdown(out: &mut String, summary: DiffLedgerMovementSummary) {
    out.push_str(&format!(
        "| Movement introduced | {} |\n",
        summary.movement.introduced
    ));
    out.push_str(&format!(
        "| Movement retained | {} |\n",
        summary.movement.retained
    ));
    out.push_str(&format!(
        "| Movement removed | {} |\n",
        summary.movement.removed
    ));
    out.push_str(&format!(
        "| Posture improved | {} |\n",
        summary.posture_delta.improved
    ));
    out.push_str(&format!(
        "| Posture worsened | {} |\n",
        summary.posture_delta.worsened
    ));
    out.push_str(&format!(
        "| Posture review required | {} |\n",
        summary.posture_delta.review_required
    ));
    out.push_str(&format!(
        "| Posture unchanged | {} |\n",
        summary.posture_delta.unchanged
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiffMovementCounts, DiffPostureDeltaCounts};

    #[test]
    fn movement_summary_json_emits_dual_blocks() {
        let summary = DiffLedgerMovementSummary {
            movement: DiffMovementCounts {
                introduced: 1,
                retained: 2,
                removed: 0,
            },
            posture_delta: DiffPostureDeltaCounts {
                improved: 0,
                worsened: 1,
                review_required: 1,
                unchanged: 1,
            },
        };
        let mut out = String::new();
        append_movement_summary_json(&mut out, summary);
        assert!(out.contains("\"introduced\": 1"));
        assert!(out.contains("\"review_required\": 1"));
    }
}
