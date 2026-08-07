use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurrentnessV1 {
    Current,
    Stale,
    NotProbed,
    PartialOrUnavailable,
}

impl CurrentnessV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Stale => "stale",
            Self::NotProbed => "not_probed",
            Self::PartialOrUnavailable => "partial_or_unavailable",
        }
    }
}
