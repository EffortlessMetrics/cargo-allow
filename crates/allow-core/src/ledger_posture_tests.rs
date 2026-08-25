use super::{LedgerPosture, NetPosture, PostureDelta, PresenceMovement};
use std::str::FromStr;

/// Characterization: current finding-change artifact labels before type wiring.
#[test]
fn finding_change_labels_match_current_artifact_strings() {
    let cases = [
        (PresenceMovement::Introduced, "new"),
        (PresenceMovement::Removed, "removed"),
    ];
    for (movement, expected) in cases {
        assert_eq!(movement.finding_change_label(), expected);
        assert_eq!(
            PresenceMovement::parse_finding_change_label(expected).ok(),
            Some(movement)
        );
    }
}

/// Characterization: PR-summary movement projection vocabulary (PR 2 contract).
#[test]
fn movement_projection_labels_match_spec_contract() {
    let cases = [
        (PresenceMovement::Introduced, "new"),
        (PresenceMovement::Retained, "inherited"),
        (PresenceMovement::Removed, "resolved"),
    ];
    for (movement, expected) in cases {
        assert_eq!(movement.movement_projection(), expected);
        assert_eq!(
            PresenceMovement::parse_movement_projection(expected).ok(),
            Some(movement)
        );
    }
}

/// Characterization: movement projection only uses `touched_in_diff` for
/// retained, unchanged entries; introduced and removed entries are stable.
#[test]
fn movement_projection_is_exhaustive_across_movement_delta_and_diff() {
    for &movement in PresenceMovement::ALL {
        for &delta in PostureDelta::ALL {
            for touched_in_diff in [false, true] {
                let posture = LedgerPosture::new(movement, delta);
                let expected = match (movement, delta, touched_in_diff) {
                    (PresenceMovement::Introduced, _, _) => "new",
                    (PresenceMovement::Removed, _, _) => "resolved",
                    (PresenceMovement::Retained, PostureDelta::Unchanged, false) => "inherited",
                    (PresenceMovement::Retained, _, _) => "retained",
                };
                assert_eq!(posture.movement_projection(touched_in_diff), expected);
            }
        }
    }
}

/// Characterization: coverage-movement classification uses the four-value vocabulary.
#[test]
fn coverage_movement_classification_labels_match_spec_contract() {
    let cases = [
        (
            LedgerPosture::new(PresenceMovement::Introduced, PostureDelta::ReviewRequired),
            true,
            "new",
        ),
        (
            LedgerPosture::new(PresenceMovement::Removed, PostureDelta::Improved),
            true,
            "resolved",
        ),
        (
            LedgerPosture::new(PresenceMovement::Retained, PostureDelta::Unchanged),
            false,
            "inherited",
        ),
        (
            LedgerPosture::new(PresenceMovement::Retained, PostureDelta::Worsened),
            true,
            "worsened",
        ),
    ];
    for (posture, touched_in_diff, expected) in cases {
        assert_eq!(
            posture.coverage_movement_classification(touched_in_diff),
            expected
        );
        assert_eq!(
            LedgerPosture::parse_coverage_movement_classification(expected),
            Some(expected)
        );
    }
}

/// Characterization: retained review/improvement rows keep movement projection fallback.
#[test]
fn coverage_movement_classification_falls_back_for_other_retained_deltas() {
    let review = LedgerPosture::new(PresenceMovement::Retained, PostureDelta::ReviewRequired);
    assert_eq!(review.coverage_movement_classification(true), "retained");

    let improved = LedgerPosture::new(PresenceMovement::Retained, PostureDelta::Improved);
    assert_eq!(improved.coverage_movement_classification(true), "retained");
}

/// Characterization: net posture JSON spellings from diff artifacts.
#[test]
fn net_posture_labels_match_current_diff_artifact_strings() {
    let cases = [
        (NetPosture::Worse, "worse"),
        (NetPosture::ReviewRequired, "review-required"),
        (NetPosture::Improved, "improved"),
        (NetPosture::Unchanged, "unchanged"),
    ];
    for (posture, expected) in cases {
        assert_eq!(posture.net_posture_label(), expected);
        assert_eq!(posture.as_str(), expected);
        assert_eq!(NetPosture::parse_net_posture_label(expected), Some(posture));
        assert_eq!(NetPosture::from_str(expected).ok(), Some(posture));
        assert!(!posture.reviewer_action().is_empty());
    }
}

/// Characterization: posture delta receipt/JSON field names.
#[test]
fn posture_delta_field_names_match_spec_contract() {
    let cases = [
        (PostureDelta::Improved, "improved"),
        (PostureDelta::Worsened, "worsened"),
        (PostureDelta::ReviewRequired, "review_required"),
        (PostureDelta::Unchanged, "unchanged"),
    ];
    for (delta, expected) in cases {
        assert_eq!(delta.field_name(), expected);
        assert_eq!(delta.as_str(), expected);
        assert_eq!(delta.to_string(), expected);
        assert_eq!(PostureDelta::parse_field_name(expected), Some(delta));
        assert_eq!(PostureDelta::from_str(expected).ok(), Some(delta));
    }
}

#[test]
fn presence_movement_round_trips_and_orders_exhaustively() {
    for (index, movement) in PresenceMovement::ALL.iter().enumerate() {
        assert_eq!(
            PresenceMovement::from_str(movement.field_name()).ok(),
            Some(*movement)
        );
        assert_eq!(movement.to_string(), movement.display_label());
        if index > 0 {
            assert!(PresenceMovement::ALL[index - 1] < *movement);
        }
    }
    assert_eq!(PresenceMovement::ALL.len(), 3);
}

#[test]
fn posture_delta_round_trips_and_orders_exhaustively() {
    for (index, delta) in PostureDelta::ALL.iter().enumerate() {
        assert_eq!(
            PostureDelta::from_str(delta.field_name()).ok(),
            Some(*delta)
        );
        if index > 0 {
            assert!(PostureDelta::ALL[index - 1] < *delta);
        }
    }
    assert_eq!(PostureDelta::ALL.len(), 4);
}

#[test]
fn net_posture_orders_exhaustively() {
    for index in 1..NetPosture::ALL.len() {
        assert!(NetPosture::ALL[index - 1] < NetPosture::ALL[index]);
    }
}

#[test]
fn net_posture_maps_to_posture_delta_where_aligned() {
    assert_eq!(
        NetPosture::Improved.posture_delta(),
        Some(PostureDelta::Improved)
    );
    assert_eq!(
        NetPosture::Unchanged.posture_delta(),
        Some(PostureDelta::Unchanged)
    );
    assert_eq!(
        NetPosture::ReviewRequired.posture_delta(),
        Some(PostureDelta::ReviewRequired)
    );
    assert_eq!(
        NetPosture::Worse.posture_delta(),
        Some(PostureDelta::Worsened)
    );
}

#[test]
fn internal_presence_movement_does_not_use_inherited() {
    for movement in PresenceMovement::ALL {
        assert_ne!(movement.field_name(), "inherited");
        assert_ne!(movement.as_str(), "inherited");
    }
}
