//! Support tier claim DTOs (#2584-B).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupportTierRow {
    pub surface: String,
    pub tier: SupportTierLevel,
    pub claim: String,
    pub proof_command: String,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportTierLevel {
    Stable,
    Stabilizing,
    Advisory,
}

impl SupportTierLevel {
    pub fn requires_proof_command(self) -> bool {
        matches!(self, Self::Stable | Self::Stabilizing)
    }
}
