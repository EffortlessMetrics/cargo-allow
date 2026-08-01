use allow_core::{Finding, MatchOutcome};

use crate::InventoryContext;

/// Scope evidence carried by `why` and its optional add-finding plan.
#[derive(Debug, Clone, Copy)]
pub struct EvaluationContext<'a> {
    pub scope: &'a str,
    pub locality: &'a str,
    pub reasons: &'a [String],
}

#[derive(Debug, Clone, Copy)]
pub struct WhyCandidateEntry<'a> {
    pub id: &'a str,
    pub kind: &'a str,
    pub family: Option<&'a str>,
    pub path: Option<&'a str>,
    pub glob: Option<&'a str>,
    pub selector_glob: Option<&'a str>,
    pub mismatch_reasons: &'a [String],
}

/// Structured proof-plan argv for machine consumers of `cargo-allow.why.v1`.
#[derive(Debug, Clone, Copy)]
pub struct WhyProofPlan<'a> {
    pub program: &'a str,
    pub args: &'a [String],
}

#[derive(Debug, Clone, Copy)]
pub struct WhyReport<'a> {
    pub inventory: InventoryContext<'a>,
    pub evaluation: EvaluationContext<'a>,
    pub finding: &'a Finding,
    pub outcome: &'a MatchOutcome,
    pub candidate_entries: &'a [WhyCandidateEntry<'a>],
    pub suggested_actions: &'a [String],
    pub proof_commands: &'a [String],
    pub proof_plans: &'a [WhyProofPlan<'a>],
}
