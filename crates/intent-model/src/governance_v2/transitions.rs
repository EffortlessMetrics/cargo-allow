//! V2 transition reference DTOs (#2942 step 1 / #3327).
//!
//! Pure authored references to moves, shims, parity cases, and cutover
//! stages, plus transition expiry/removal/rollback facts. These are
//! references into the move/shim/parity authorities, not re-authoring of
//! their full records.

use serde::{Deserialize, Serialize};

/// Reference to a move-ledger entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveReferenceV2 {
    pub entry_id: String,
    pub source_kind: String,
    pub current_product: String,
    pub current_crate: String,
    pub target_product: String,
    pub target_crate: String,
}

impl MoveReferenceV2 {
    pub fn validate(&self) -> Result<(), String> {
        if self.entry_id.trim().is_empty() {
            return Err("move reference requires a non-empty entry_id".into());
        }
        if self.current_crate.trim().is_empty() || self.target_crate.trim().is_empty() {
            return Err(format!(
                "move reference `{}` requires current and target crates",
                self.entry_id
            ));
        }
        Ok(())
    }
}

/// Status of an extraction shim in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShimStatusV2 {
    Planned,
    Active,
    Retired,
}

impl ShimStatusV2 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Active => "active",
            Self::Retired => "retired",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "planned" => Ok(Self::Planned),
            "active" => Ok(Self::Active),
            "retired" => Ok(Self::Retired),
            other => Err(format!("unsupported shim status `{other}`")),
        }
    }
}

/// Reference to an extraction shim record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShimReferenceV2 {
    pub shim_id: String,
    pub old_identity: String,
    pub new_identity: String,
    pub status: ShimStatusV2,
    pub move_ledger_entry: String,
    pub controlling_issue: u32,
    pub latest_allowed_stage: u32,
}

impl ShimReferenceV2 {
    pub fn validate(&self) -> Result<(), String> {
        if self.shim_id.trim().is_empty() {
            return Err("shim reference requires a non-empty shim_id".into());
        }
        if self.old_identity.trim().is_empty() || self.new_identity.trim().is_empty() {
            return Err(format!(
                "shim reference `{}` requires old and new identities",
                self.shim_id
            ));
        }
        Ok(())
    }
}

/// Disposition of a parity case in the extraction-parity authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParityDispositionV2 {
    ContractOnly,
    EvidenceBacked,
    Retired,
}

impl ParityDispositionV2 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ContractOnly => "contract_only",
            Self::EvidenceBacked => "evidence_backed",
            Self::Retired => "retired",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "contract_only" => Ok(Self::ContractOnly),
            "evidence_backed" => Ok(Self::EvidenceBacked),
            "retired" => Ok(Self::Retired),
            other => Err(format!("unsupported parity disposition `{other}`")),
        }
    }
}

/// Reference to a parity case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParityReferenceV2 {
    pub case_id: String,
    pub stage: String,
    pub move_ledger_entry: String,
    #[serde(default)]
    pub shim_id: Option<String>,
    pub disposition: ParityDispositionV2,
}

impl ParityReferenceV2 {
    pub fn validate(&self) -> Result<(), String> {
        if self.case_id.trim().is_empty() {
            return Err("parity reference requires a non-empty case_id".into());
        }
        Ok(())
    }
}

/// Reference to a cutover stage receipt (#2606 authority).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CutoverReferenceV2 {
    pub stage: u32,
    pub product: String,
    pub receipt_id: String,
}

impl CutoverReferenceV2 {
    pub fn validate(&self) -> Result<(), String> {
        if self.receipt_id.trim().is_empty() {
            return Err("cutover reference requires a non-empty receipt_id".into());
        }
        Ok(())
    }
}

/// Transition expiry, removal condition, and rollback note for a shim or
/// compatibility adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionExpiryV2 {
    pub component_id: String,
    pub removal_condition: String,
    #[serde(default)]
    pub rollback_note: String,
}

impl TransitionExpiryV2 {
    pub fn validate(&self) -> Result<(), String> {
        if self.component_id.trim().is_empty() {
            return Err("transition expiry requires a non-empty component_id".into());
        }
        if self.removal_condition.trim().is_empty() {
            return Err(format!(
                "transition expiry for `{}` requires a removal condition",
                self.component_id
            ));
        }
        Ok(())
    }
}
