use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultClassV1 {
    Completed,
    Findings,
    NotProven,
    PartialData,
    StaleInput,
    Unsupported,
    MalformedInput,
    InstrumentFailure,
    Cancelled,
    Conflict,
}

impl ResultClassV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Findings => "findings",
            Self::NotProven => "not_proven",
            Self::PartialData => "partial_data",
            Self::StaleInput => "stale_input",
            Self::Unsupported => "unsupported",
            Self::MalformedInput => "malformed_input",
            Self::InstrumentFailure => "instrument_failure",
            Self::Cancelled => "cancelled",
            Self::Conflict => "conflict",
        }
    }

    /// Whether this class must not deserialize as a clean completed success.
    pub fn denies_clean_completion(self) -> bool {
        !matches!(self, Self::Completed)
    }
}
