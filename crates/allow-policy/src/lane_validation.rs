use allow_core::{
    AllowConfig, CargoAllowError, CargoAllowResult, FindingKind, LaneEnforcementMode,
};
use std::str::FromStr;

use crate::text_validation::validate_required_text;

pub(crate) fn validate_lanes(cfg: &AllowConfig) -> CargoAllowResult<()> {
    for (name, lane) in &cfg.lanes {
        validate_required_text(&format!("lanes.{name}.mode"), lane.mode.as_str())?;
        if lane.mode.as_str().trim() != lane.mode.as_str() {
            return Err(CargoAllowError::new(format!(
                "lanes.{name}.mode must not have leading or trailing whitespace"
            )));
        }
        // Validate the mode value against the known LaneEnforcementMode enum
        // so a typo like mode = "blockng" is caught at validation time, not
        // silently stored and only failing later at match time (#1830).
        LaneEnforcementMode::from_str(lane.mode.as_str())
            .map_err(|e| CargoAllowError::new(format!("lanes.{name}.mode: {e}")))?;
        FindingKind::from_str(name).map_err(|_| {
            CargoAllowError::new(format!(
                "unsupported lane name `{name}`; expected a finding kind such as panic or unsafe"
            ))
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{LaneConfig, LaneEnforcementMode};
    use std::collections::BTreeMap;

    #[test]
    fn accepts_configured_lane_posture() {
        let mut lanes = BTreeMap::new();
        lanes.insert(
            "panic".to_string(),
            LaneConfig {
                mode: LaneEnforcementMode::Blocking,
            },
        );
        lanes.insert(
            "unsafe".to_string(),
            LaneConfig {
                mode: LaneEnforcementMode::Shadow,
            },
        );
        let cfg = AllowConfig {
            lanes,
            ..AllowConfig::empty()
        };

        assert!(validate_lanes(&cfg).is_ok());
    }

    #[test]
    fn rejects_unknown_lane_name() {
        let mut lanes = BTreeMap::new();
        lanes.insert(
            "ripr".to_string(),
            LaneConfig {
                mode: LaneEnforcementMode::Shadow,
            },
        );
        let cfg = AllowConfig {
            lanes,
            ..AllowConfig::empty()
        };

        let err = validate_lanes(&cfg).expect_err("unknown lane should fail");
        assert!(err.to_string().contains("unsupported lane name `ripr`"));
    }
}
