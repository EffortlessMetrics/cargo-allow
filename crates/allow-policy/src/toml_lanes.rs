use allow_core::{CargoAllowResult, LaneConfig, LaneEnforcementMode};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::str::FromStr;

#[derive(Debug, Default, Deserialize)]
pub(crate) struct LanesToml {
    #[serde(flatten)]
    pub(crate) lanes: BTreeMap<String, LaneToml>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LaneToml {
    pub(crate) mode: String,
}

impl LanesToml {
    pub(crate) fn into_lane_configs(self) -> CargoAllowResult<BTreeMap<String, LaneConfig>> {
        self.lanes
            .into_iter()
            .map(|(name, lane)| {
                let mode = LaneEnforcementMode::from_str(&lane.mode).map_err(|err| {
                    allow_core::CargoAllowError::new(format!(
                        "lanes.{name} has invalid mode: {err}"
                    ))
                })?;
                Ok((name, LaneConfig { mode }))
            })
            .collect()
    }
}
