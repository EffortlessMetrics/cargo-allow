use crate::{CargoAllowError, FindingKind};
use std::collections::BTreeMap;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum LaneEnforcementMode {
    Advisory,
    Shadow,
    #[default]
    Blocking,
}

impl LaneEnforcementMode {
    pub const ALL: &[Self] = &[Self::Advisory, Self::Shadow, Self::Blocking];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Advisory => "advisory",
            Self::Shadow => "shadow",
            Self::Blocking => "blocking",
        }
    }

    pub fn blocks_check_failure(self) -> bool {
        matches!(self, Self::Blocking)
    }
}

impl FromStr for LaneEnforcementMode {
    type Err = CargoAllowError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();
        let normalized = trimmed.to_ascii_lowercase();
        match normalized.as_str() {
            "advisory" => Ok(Self::Advisory),
            "shadow" => Ok(Self::Shadow),
            "blocking" => Ok(Self::Blocking),
            _ => Err(CargoAllowError::new(format!(
                "unsupported lane enforcement mode `{trimmed}`; valid values: advisory, shadow, blocking"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneConfig {
    pub mode: LaneEnforcementMode,
}

pub fn lane_enforcement_mode_for_kind(
    lanes: &BTreeMap<String, LaneConfig>,
    kind: FindingKind,
) -> LaneEnforcementMode {
    lanes
        .get(kind.as_str())
        .map(|lane| lane.mode)
        .unwrap_or(LaneEnforcementMode::Blocking)
}

pub fn effective_lane_posture_for_findings(
    lanes: &BTreeMap<String, LaneConfig>,
    kinds: impl IntoIterator<Item = FindingKind>,
) -> BTreeMap<String, LaneEnforcementMode> {
    let mut effective = lanes
        .iter()
        .map(|(name, lane)| (name.clone(), lane.mode))
        .collect::<BTreeMap<_, _>>();
    for kind in kinds {
        effective
            .entry(kind.as_str().to_string())
            .or_insert(LaneEnforcementMode::Blocking);
    }
    effective
}

#[cfg(test)]
#[path = "lane_posture_tests.rs"]
mod tests;
