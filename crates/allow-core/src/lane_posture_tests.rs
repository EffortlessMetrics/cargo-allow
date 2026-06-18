use super::{
    LaneEnforcementMode, lane_enforcement_mode_for_kind,
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
