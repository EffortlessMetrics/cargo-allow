use crate::CargoAllowError;
use std::fmt;
use std::str::FromStr;

/// Canonical presence movement for ledger entries and findings in a diff context.
///
/// Internal model uses `Introduced`, `Retained`, and `Removed`. Artifact
/// projections for PR summaries use [`PresenceMovement::movement_projection`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PresenceMovement {
    Introduced,
    Retained,
    Removed,
}

impl PresenceMovement {
    pub const ALL: &[Self] = &[Self::Introduced, Self::Retained, Self::Removed];

    /// Stable snake_case field name for machine-readable ledger-state records.
    pub const fn field_name(self) -> &'static str {
        match self {
            Self::Introduced => "introduced",
            Self::Retained => "retained",
            Self::Removed => "removed",
        }
    }

    /// PR-summary movement projection (PR 2). Not used as internal storage.
    pub const fn movement_projection(self) -> &'static str {
        match self {
            Self::Introduced => "new",
            Self::Retained => "inherited",
            Self::Removed => "resolved",
        }
    }

    /// Current finding-change artifact label (`finding_changes[].change`).
    pub const fn finding_change_label(self) -> &'static str {
        match self {
            Self::Introduced => "new",
            Self::Retained => "retained",
            Self::Removed => "removed",
        }
    }

    pub const fn display_label(self) -> &'static str {
        match self {
            Self::Introduced => "introduced",
            Self::Retained => "retained",
            Self::Removed => "removed",
        }
    }

    pub const fn as_str(self) -> &'static str {
        self.field_name()
    }

    pub fn parse_field_name(value: &str) -> Result<Self, CargoAllowError> {
        Self::from_str(value)
    }

    pub fn parse_finding_change_label(value: &str) -> Result<Self, CargoAllowError> {
        match value.trim() {
            "new" => Ok(Self::Introduced),
            "removed" => Ok(Self::Removed),
            "retained" => Ok(Self::Retained),
            other => Err(CargoAllowError::new(format!(
                "unsupported finding posture change `{other}`"
            ))),
        }
    }

    pub fn parse_movement_projection(value: &str) -> Result<Self, CargoAllowError> {
        match value.trim() {
            "new" => Ok(Self::Introduced),
            "inherited" => Ok(Self::Retained),
            "resolved" => Ok(Self::Removed),
            other => Err(CargoAllowError::new(format!(
                "unsupported movement projection `{other}`"
            ))),
        }
    }
}

impl fmt::Display for PresenceMovement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_label())
    }
}

impl FromStr for PresenceMovement {
    type Err = CargoAllowError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "introduced" => Ok(Self::Introduced),
            "retained" => Ok(Self::Retained),
            "removed" => Ok(Self::Removed),
            other => Err(CargoAllowError::new(format!(
                "unsupported presence movement `{other}`"
            ))),
        }
    }
}

/// Canonical posture quality delta for a retained ledger entry or finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PostureDelta {
    Improved,
    Worsened,
    ReviewRequired,
    Unchanged,
}

impl PostureDelta {
    pub const ALL: &[Self] = &[
        Self::Improved,
        Self::Worsened,
        Self::ReviewRequired,
        Self::Unchanged,
    ];

    pub const fn field_name(self) -> &'static str {
        match self {
            Self::Improved => "improved",
            Self::Worsened => "worsened",
            Self::ReviewRequired => "review_required",
            Self::Unchanged => "unchanged",
        }
    }

    pub const fn display_label(self) -> &'static str {
        self.field_name()
    }

    pub const fn as_str(self) -> &'static str {
        self.field_name()
    }

    pub fn parse_field_name(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|delta| delta.field_name() == value.trim())
    }
}

impl fmt::Display for PostureDelta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_label())
    }
}

impl FromStr for PostureDelta {
    type Err = CargoAllowError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse_field_name(value)
            .ok_or_else(|| CargoAllowError::new(format!("unsupported posture delta `{value}`")))
    }
}

/// Aggregate PR diff net posture for summary surfaces (`diff.net_posture`).
///
/// Distinct from per-row [`PostureDelta`]: aggregate summaries use `worse` and
/// hyphenated `review-required` spellings required by existing artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NetPosture {
    Worse,
    ReviewRequired,
    Improved,
    Unchanged,
}

impl NetPosture {
    pub const ALL: &[Self] = &[
        Self::Worse,
        Self::ReviewRequired,
        Self::Improved,
        Self::Unchanged,
    ];

    pub const fn net_posture_label(self) -> &'static str {
        match self {
            Self::Worse => "worse",
            Self::ReviewRequired => "review-required",
            Self::Improved => "improved",
            Self::Unchanged => "unchanged",
        }
    }

    pub const fn as_str(self) -> &'static str {
        self.net_posture_label()
    }

    pub const fn reviewer_action(self) -> &'static str {
        match self {
            Self::Worse => {
                "block until failing source exception changes are fixed, narrowed, or receipted."
            }
            Self::ReviewRequired => "review the source exception posture change before merging.",
            Self::Improved => "verify the cleanup was intentional and keep the narrower posture.",
            Self::Unchanged => "no source exception posture change detected.",
        }
    }

    pub fn parse_net_posture_label(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|posture| posture.net_posture_label() == value.trim())
    }

    pub const fn posture_delta(self) -> Option<PostureDelta> {
        match self {
            Self::Worse => Some(PostureDelta::Worsened),
            Self::ReviewRequired => Some(PostureDelta::ReviewRequired),
            Self::Improved => Some(PostureDelta::Improved),
            Self::Unchanged => Some(PostureDelta::Unchanged),
        }
    }
}

impl fmt::Display for NetPosture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.net_posture_label())
    }
}

impl FromStr for NetPosture {
    type Err = CargoAllowError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse_net_posture_label(value)
            .ok_or_else(|| CargoAllowError::new(format!("unsupported net posture `{value}`")))
    }
}

/// Orthogonal movement and posture delta for a ledger-state projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedgerPosture {
    pub movement: PresenceMovement,
    pub delta: PostureDelta,
}

impl LedgerPosture {
    pub const fn new(movement: PresenceMovement, delta: PostureDelta) -> Self {
        Self { movement, delta }
    }

    /// Projects a ledger row into the PR-summary movement vocabulary.
    ///
    /// `touched_in_diff` only affects retained rows whose posture delta is
    /// unchanged: an untouched row is inherited, while a touched row is
    /// retained. Introduced and removed rows remain `new` and `resolved`
    /// regardless of the flag or posture delta.
    pub fn movement_projection(self, touched_in_diff: bool) -> &'static str {
        match self.movement {
            PresenceMovement::Introduced => "new",
            PresenceMovement::Removed => "resolved",
            PresenceMovement::Retained
                if self.delta == PostureDelta::Unchanged && !touched_in_diff =>
            {
                "inherited"
            }
            PresenceMovement::Retained => "retained",
        }
    }

    /// Per-entry coverage-movement classification for diff/posture surfaces.
    ///
    /// Collapses orthogonal movement and posture delta into the shared
    /// `new` / `worsened` / `resolved` / `inherited` vocabulary. Retained rows
    /// with other posture deltas fall back to [`Self::movement_projection`].
    pub fn coverage_movement_classification(self, touched_in_diff: bool) -> &'static str {
        match self.movement {
            PresenceMovement::Introduced => "new",
            PresenceMovement::Removed => "resolved",
            PresenceMovement::Retained if self.delta == PostureDelta::Worsened => "worsened",
            PresenceMovement::Retained
                if self.delta == PostureDelta::Unchanged && !touched_in_diff =>
            {
                "inherited"
            }
            PresenceMovement::Retained => self.movement_projection(touched_in_diff),
        }
    }

    /// Parse a coverage-movement classification label.
    pub fn parse_coverage_movement_classification(value: &str) -> Option<&'static str> {
        match value.trim() {
            "new" => Some("new"),
            "worsened" => Some("worsened"),
            "resolved" => Some("resolved"),
            "inherited" => Some("inherited"),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "ledger_posture_tests.rs"]
mod tests;
