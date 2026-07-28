use super::{
    LaneConfig, LaneEnforcementMode, effective_lane_posture_for_findings,
    lane_enforcement_mode_for_kind,
};
use crate::FindingKind;
use std::collections::BTreeMap;
use std::str::FromStr;

#[test]
fn lane_enforcement_mode_parses_supported_values() {
    assert_eq!(
        LaneEnforcementMode::from_str("shadow").ok(),
        Some(LaneEnforcementMode::Shadow)
    );
}

#[test]
fn default_lane_posture_is_blocking() {
    assert_eq!(
        lane_enforcement_mode_for_kind(&BTreeMap::new(), FindingKind::Panic),
        LaneEnforcementMode::Blocking
    );
}

#[test]
fn shadow_and_advisory_do_not_block_check_failure() {
    assert!(!LaneEnforcementMode::Shadow.blocks_check_failure());
    assert!(!LaneEnforcementMode::Advisory.blocks_check_failure());
    assert!(LaneEnforcementMode::Blocking.blocks_check_failure());
}

#[test]
fn effective_lane_posture_preserves_configured_lanes() {
    let mut lanes = BTreeMap::new();
    lanes.insert(
        "panic".to_string(),
        LaneConfig {
            mode: LaneEnforcementMode::Shadow,
        },
    );
    lanes.insert(
        "unsafe".to_string(),
        LaneConfig {
            mode: LaneEnforcementMode::Advisory,
        },
    );
    let result = effective_lane_posture_for_findings(
        &lanes,
        [
            FindingKind::Panic,
            FindingKind::Unsafe,
            FindingKind::LintException,
        ],
    );
    assert_eq!(result.get("panic"), Some(&LaneEnforcementMode::Shadow));
    assert_eq!(result.get("unsafe"), Some(&LaneEnforcementMode::Advisory));
}

#[test]
fn effective_lane_posture_defaults_unconfigured_kinds_to_blocking() {
    let lanes = BTreeMap::new();
    let result =
        effective_lane_posture_for_findings(&lanes, [FindingKind::Panic, FindingKind::NonRustFile]);
    assert_eq!(result.get("panic"), Some(&LaneEnforcementMode::Blocking));
    assert_eq!(
        result.get("non_rust_file"),
        Some(&LaneEnforcementMode::Blocking)
    );
}
