use allow_core::{LedgerPosture, PostureDelta, PresenceMovement};

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
    out.push_str("  movement:\n");
    out.push_str(&format!(
        "    introduced: {}\n",
        summary.movement.introduced
    ));
    out.push_str(&format!("    retained: {}\n", summary.movement.retained));
    out.push_str(&format!("    removed: {}\n", summary.movement.removed));
    out.push_str("  posture_delta:\n");
    out.push_str(&format!(
        "    improved: {}\n",
        summary.posture_delta.improved
    ));
    out.push_str(&format!(
        "    worsened: {}\n",
        summary.posture_delta.worsened
    ));
    out.push_str(&format!(
        "    review_required: {}\n",
        summary.posture_delta.review_required
    ));
    out.push_str(&format!(
        "    unchanged: {}\n",
        summary.posture_delta.unchanged
    ));
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

pub fn movement_projection_label(movement: &str, posture_delta: &str, changed_in_diff: bool) -> &'static str {
    let movement = PresenceMovement::parse_field_name(movement)
        .unwrap_or(PresenceMovement::Retained);
    let delta = PostureDelta::parse_field_name(posture_delta).unwrap_or(PostureDelta::Unchanged);
    LedgerPosture::new(movement, delta).movement_projection(changed_in_diff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiffMovementCounts, DiffPostureDeltaCounts};

    #[test]
    fn movement_projection_collapses_unchanged_retained_rows_to_inherited() {
        assert_eq!(
            movement_projection_label("retained", "unchanged", false),
            "inherited"
        );
        assert_eq!(
            movement_projection_label("retained", "unchanged", true),
            "retained"
        );
        assert_eq!(
            movement_projection_label("introduced", "review_required", true),
            "new"
        );
        assert_eq!(
            movement_projection_label("removed", "improved", true),
            "resolved"
        );
    }

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
