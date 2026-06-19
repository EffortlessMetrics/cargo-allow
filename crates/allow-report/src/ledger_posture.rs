//! Canonical ledger posture vocabulary for report and artifact renderers.
//!
//! Re-exports `allow-core` ledger posture types and centralizes string mappings
//! for diff rows, receipts, Markdown/human output, and worklist routing.

pub use allow_core::{LedgerPosture, NetPosture, PostureDelta, PresenceMovement};

/// Finding-change labels accepted by diff artifacts (`finding_changes[].change`).
pub const FINDING_CHANGE_LABELS: &[&str] = &[
    PresenceMovement::Introduced.finding_change_label(),
    PresenceMovement::Removed.finding_change_label(),
];

/// Net posture labels accepted by diff artifacts (`diff.net_posture`).
pub const NET_POSTURE_LABELS: &[&str] = &[
    NetPosture::Worse.net_posture_label(),
    NetPosture::ReviewRequired.net_posture_label(),
    NetPosture::Improved.net_posture_label(),
    NetPosture::Unchanged.net_posture_label(),
];

/// Posture delta field names for future dual-summary counts (PR 2).
pub const POSTURE_DELTA_FIELD_NAMES: &[&str] = &[
    PostureDelta::Improved.field_name(),
    PostureDelta::Worsened.field_name(),
    PostureDelta::ReviewRequired.field_name(),
    PostureDelta::Unchanged.field_name(),
];

/// Movement projection labels for future dual-summary counts (PR 2).
pub const MOVEMENT_PROJECTION_LABELS: &[&str] = &[
    PresenceMovement::Introduced.movement_projection(),
    PresenceMovement::Retained.movement_projection(),
    PresenceMovement::Removed.movement_projection(),
];

pub fn finding_change_label_for(movement: PresenceMovement) -> &'static str {
    movement.finding_change_label()
}

pub fn parse_finding_change_label(value: &str) -> Option<PresenceMovement> {
    PresenceMovement::parse_finding_change_label(value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Characterization: registry covers prior diff artifact string sets.
    #[test]
    fn registries_cover_prior_artifact_string_sets() {
        let prior_finding_changes = ["new", "removed"];
        for label in prior_finding_changes {
            assert!(FINDING_CHANGE_LABELS.contains(&label));
            assert_eq!(
                parse_finding_change_label(label).map(finding_change_label_for),
                Some(label)
            );
        }

        let prior_net_posture = ["worse", "review-required", "improved", "unchanged"];
        for label in prior_net_posture {
            assert!(NET_POSTURE_LABELS.contains(&label));
            assert_eq!(
                NetPosture::parse_net_posture_label(label).map(|p| p.as_str()),
                Some(label)
            );
        }

        let prior_posture_delta = ["improved", "worsened", "review_required", "unchanged"];
        for field in prior_posture_delta {
            assert!(POSTURE_DELTA_FIELD_NAMES.contains(&field));
            assert_eq!(
                PostureDelta::parse_field_name(field).map(|d| d.field_name()),
                Some(field)
            );
        }

        let prior_movement = ["new", "inherited", "resolved"];
        for label in prior_movement {
            assert!(MOVEMENT_PROJECTION_LABELS.contains(&label));
        }
    }
}
