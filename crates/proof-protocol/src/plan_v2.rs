//! Evidence-oriented proof plan contract (#3599 slice A).
//!
//! ProofPlanV2 replaces argv-only planning as the semantic authority:
//! every applicable evidence need receives exactly one disposition, the
//! plan binds the exact repository snapshot and subject identity, and a
//! command is a lowering of a selected item — never the primary
//! representation. This module lands the versioned contract and its
//! validation; the planner rewiring and artifact output are the next
//! slices. Provider-specific request payloads stay namespaced and are
//! not flattened.

use serde::{Deserialize, Serialize};

pub const PROOF_PLAN_V2_SCHEMA_ID: &str = "proof.plan.v2";
pub const PROOF_PLAN_V2_SCHEMA_VERSION: u32 = 1;

/// Exactly one disposition per applicable evidence need (#3599 rules:
/// no edge silently disappears because a provider is absent; a provider
/// unavailable result can never be represented as a command).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofItemDispositionV1 {
    SelectedForExecution,
    SelectedForCapturedIngestion,
    SatisfiedByCurrentReceipt,
    DeferredWithinExplicitPolicy,
    ManualOrNativeOutstanding,
    UnsupportedCapability,
    ProviderUnavailable,
    SelectorMissingOrAmbiguous,
    RepositoryDecisionRequired,
    NotApplicableWithReason,
    NotProven,
}

impl ProofItemDispositionV1 {
    pub const ALL: [Self; 11] = [
        Self::SelectedForExecution,
        Self::SelectedForCapturedIngestion,
        Self::SatisfiedByCurrentReceipt,
        Self::DeferredWithinExplicitPolicy,
        Self::ManualOrNativeOutstanding,
        Self::UnsupportedCapability,
        Self::ProviderUnavailable,
        Self::SelectorMissingOrAmbiguous,
        Self::RepositoryDecisionRequired,
        Self::NotApplicableWithReason,
        Self::NotProven,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SelectedForExecution => "selected_for_execution",
            Self::SelectedForCapturedIngestion => "selected_for_captured_ingestion",
            Self::SatisfiedByCurrentReceipt => "satisfied_by_current_receipt",
            Self::DeferredWithinExplicitPolicy => "deferred_within_explicit_policy",
            Self::ManualOrNativeOutstanding => "manual_or_native_outstanding",
            Self::UnsupportedCapability => "unsupported_capability",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::SelectorMissingOrAmbiguous => "selector_missing_or_ambiguous",
            Self::RepositoryDecisionRequired => "repository_decision_required",
            Self::NotApplicableWithReason => "not_applicable_with_reason",
            Self::NotProven => "not_proven",
        }
    }

    /// A disposition that lowers to an executable command. A provider
    /// unavailable or unsupported item must never carry a command.
    pub const fn lowers_to_command(self) -> bool {
        matches!(self, Self::SelectedForExecution)
    }
}

/// Repository surface class for subject identity (#3599: reuse the
/// shared snapshot/subject contracts; no path-only identity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofSubjectClassV1 {
    Commit,
    Tree,
    Index,
    Worktree,
}

/// Exact evidence subject: selector plus source/body identity where
/// required, with structural limitations stated rather than hidden.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofSubjectV1 {
    pub subject_class: ProofSubjectClassV1,
    pub revision: Option<String>,
    /// Package/target/module/item or test selector where applicable;
    /// namespaced by the provider catalog, not flattened.
    pub selector: Option<String>,
    /// Source/body identity digest where the evidence class requires it.
    pub body_identity: Option<String>,
    /// Structural limitations (partial, ambiguous, generated,
    /// cfg-limited) that bound what this subject can prove.
    pub limitations: Vec<String>,
}

/// Selected provider and capability where a disposition selected one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderSelectionV1 {
    pub provider_id: String,
    pub capability_id: String,
    /// Identity of the namespaced provider request payload this item
    /// lowers to (digest retained; payload stays provider-namespaced).
    pub request_digest: String,
}

/// The receipt contract an executed or ingested item must satisfy:
/// schema/generation plus the load-bearing currentness dimensions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedReceiptContractV1 {
    pub receipt_schema: String,
    pub receipt_generation: u32,
    /// Currentness dimensions the receipt must bind exactly (snapshot,
    /// subject, provider request, config); reuse requires exact
    /// currentness, never filename or exit-code matching.
    pub currentness_dimensions: Vec<String>,
}

/// Execution posture of an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofItemExecutionPostureV1 {
    Execute,
    CapturedIngest,
    ManualNative,
    None,
}

/// One evidence need with its single disposition (#3599 contract).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofItemV1 {
    pub proof_item_id: String,
    pub intent_obligation_id: String,
    pub phase: String,
    pub blocking: bool,
    /// Statement/evidence-purpose reference into the intent plan.
    pub evidence_purpose_ref: String,
    /// Required evidence/capability class.
    pub required_capability_class: String,
    /// Exact repository snapshot identity (semantic hash).
    pub snapshot_identity: String,
    pub subject: ProofSubjectV1,
    pub disposition: ProofItemDispositionV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection: Option<ProviderSelectionV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_receipt: Option<ExpectedReceiptContractV1>,
    pub execution_posture: ProofItemExecutionPostureV1,
    /// Ordering/dependency group where required.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependency_group: Option<String>,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

/// The complete semantic plan artifact. Human/JSON summaries derive
/// from this; dry-run/run/status consume this exact artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofPlanV2 {
    pub schema_id: String,
    pub schema_version: u32,
    /// Deterministic semantic identity over the exact intent plan,
    /// snapshot, catalog, and items — a stale input can never preserve
    /// an old plan_id.
    pub plan_id: String,
    pub intent_plan_digest: String,
    pub snapshot_identity: String,
    pub items: Vec<ProofItemV1>,
}

impl ProofPlanV2 {
    pub fn new(
        plan_id: impl Into<String>,
        intent_plan_digest: impl Into<String>,
        snapshot_identity: impl Into<String>,
        items: Vec<ProofItemV1>,
    ) -> Self {
        Self {
            schema_id: PROOF_PLAN_V2_SCHEMA_ID.to_string(),
            schema_version: PROOF_PLAN_V2_SCHEMA_VERSION,
            plan_id: plan_id.into(),
            intent_plan_digest: intent_plan_digest.into(),
            snapshot_identity: snapshot_identity.into(),
            items,
        }
    }

    /// Validate the #3599 planning rules. Fails closed on every
    /// negative-control shape: duplicate item ids, command-lowering on
    /// a non-execution disposition, selection on unselected
    /// dispositions, and an empty plan without explicit no-execution
    /// dispositions.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != PROOF_PLAN_V2_SCHEMA_ID {
            return Err(format!("unexpected schema_id {}", self.schema_id));
        }
        let mut seen = std::collections::BTreeSet::new();
        for item in &self.items {
            if !seen.insert(item.proof_item_id.as_str()) {
                return Err(format!("duplicate proof item id {}", item.proof_item_id));
            }
            if item.disposition.lowers_to_command() && item.selection.is_none() {
                return Err(format!(
                    "item {} selected for execution without a provider selection",
                    item.proof_item_id
                ));
            }
            if !item.disposition.lowers_to_command() && item.selection.is_some() {
                return Err(format!(
                    "item {} carries a selection without the execution disposition",
                    item.proof_item_id
                ));
            }
            if item.disposition == ProofItemDispositionV1::ProviderUnavailable
                && item.execution_posture == ProofItemExecutionPostureV1::Execute
            {
                return Err(format!(
                    "provider-unavailable item {} cannot carry the execute posture",
                    item.proof_item_id
                ));
            }
        }
        if self.items.is_empty() {
            return Err(
                "empty plan is valid only with explicit no-execution dispositions; got zero items"
                    .to_string(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject() -> ProofSubjectV1 {
        ProofSubjectV1 {
            subject_class: ProofSubjectClassV1::Index,
            revision: Some("abc123".to_string()),
            selector: Some("tests::suite_a".to_string()),
            body_identity: Some("sha256:v1:body".to_string()),
            limitations: vec![],
        }
    }

    fn item(id: &str, disposition: ProofItemDispositionV1) -> ProofItemV1 {
        ProofItemV1 {
            proof_item_id: id.to_string(),
            intent_obligation_id: "obl-1".to_string(),
            phase: "precommit".to_string(),
            blocking: true,
            evidence_purpose_ref: "obl-1.purpose".to_string(),
            required_capability_class: "evidence_review".to_string(),
            snapshot_identity: "snap-1".to_string(),
            subject: subject(),
            disposition,
            selection: None,
            expected_receipt: None,
            execution_posture: ProofItemExecutionPostureV1::None,
            dependency_group: None,
            limitations: vec![],
            claim_boundary: "contract test".to_string(),
        }
    }

    fn plan(items: Vec<ProofItemV1>) -> ProofPlanV2 {
        ProofPlanV2::new("plan-1", "sha256:v1:intent", "snap-1", items)
    }

    #[test]
    fn every_disposition_has_a_stable_snake_case_name() -> Result<(), String> {
        let mut seen = std::collections::BTreeSet::new();
        for disposition in ProofItemDispositionV1::ALL {
            if !seen.insert(disposition.as_str()) {
                return Err(format!(
                    "disposition name collision: {}",
                    disposition.as_str()
                ));
            }
        }
        if seen.len() != 11 {
            return Err(format!("expected 11 dispositions, got {}", seen.len()));
        }
        Ok(())
    }

    #[test]
    fn provider_unavailable_never_lowers_to_a_command() {
        assert!(!ProofItemDispositionV1::ProviderUnavailable.lowers_to_command());
        assert!(ProofItemDispositionV1::SelectedForExecution.lowers_to_command());
    }

    #[test]
    fn valid_plan_with_explicit_unavailable_disposition_passes() -> Result<(), String> {
        let plan = plan(vec![item(
            "item-1",
            ProofItemDispositionV1::ProviderUnavailable,
        )]);
        plan.validate()
    }

    #[test]
    fn duplicate_item_ids_fail() -> Result<(), String> {
        let plan = plan(vec![
            item("item-1", ProofItemDispositionV1::NotProven),
            item("item-1", ProofItemDispositionV1::NotProven),
        ]);
        if plan.validate().is_ok() {
            return Err("duplicate ids must fail".to_string());
        }
        Ok(())
    }

    #[test]
    fn selection_without_execution_disposition_fails() -> Result<(), String> {
        let mut selected = item("item-1", ProofItemDispositionV1::ProviderUnavailable);
        selected.selection = Some(ProviderSelectionV1 {
            provider_id: "cargo-allow".to_string(),
            capability_id: "check".to_string(),
            request_digest: "sha256:v1:req".to_string(),
        });
        let plan = plan(vec![selected]);
        if plan.validate().is_ok() {
            return Err("selection on a non-execution disposition must fail".to_string());
        }
        Ok(())
    }

    #[test]
    fn execution_disposition_requires_selection() -> Result<(), String> {
        let plan = plan(vec![item(
            "item-1",
            ProofItemDispositionV1::SelectedForExecution,
        )]);
        if plan.validate().is_ok() {
            return Err("selected-for-execution without a selection must fail".to_string());
        }
        Ok(())
    }

    #[test]
    fn provider_unavailable_with_execute_posture_fails() -> Result<(), String> {
        let mut unavailable = item("item-1", ProofItemDispositionV1::ProviderUnavailable);
        unavailable.execution_posture = ProofItemExecutionPostureV1::Execute;
        let plan = plan(vec![unavailable]);
        if plan.validate().is_ok() {
            return Err("provider-unavailable execute posture must fail".to_string());
        }
        Ok(())
    }

    #[test]
    fn empty_plan_fails_without_explicit_dispositions() -> Result<(), String> {
        let plan = plan(vec![]);
        if plan.validate().is_ok() {
            return Err("zero-item plan must fail".to_string());
        }
        Ok(())
    }

    #[test]
    fn plan_serialization_round_trips() -> Result<(), String> {
        let mut selected = item("item-1", ProofItemDispositionV1::SelectedForExecution);
        selected.selection = Some(ProviderSelectionV1 {
            provider_id: "cargo-allow".to_string(),
            capability_id: "check".to_string(),
            request_digest: "sha256:v1:req".to_string(),
        });
        selected.expected_receipt = Some(ExpectedReceiptContractV1 {
            receipt_schema: "cargo-allow.analysis-receipt.v1".to_string(),
            receipt_generation: 1,
            currentness_dimensions: vec![
                "snapshot".to_string(),
                "subject".to_string(),
                "provider_request".to_string(),
            ],
        });
        selected.execution_posture = ProofItemExecutionPostureV1::Execute;
        let plan = plan(vec![selected]);
        plan.validate()?;
        let json = serde_json::to_string(&plan).map_err(|error| error.to_string())?;
        let back: ProofPlanV2 = serde_json::from_str(&json).map_err(|error| error.to_string())?;
        if back != plan {
            return Err("plan round-trip drifted".to_string());
        }
        Ok(())
    }
}
