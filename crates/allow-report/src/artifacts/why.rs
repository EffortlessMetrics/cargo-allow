use allow_core::{Finding, MatchOutcome};

use crate::InventoryContext;

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

#[derive(Debug, Clone, Copy)]
pub struct WhyReport<'a> {
    pub inventory: InventoryContext<'a>,
    pub finding: &'a Finding,
    pub outcome: &'a MatchOutcome,
    pub candidate_entries: &'a [WhyCandidateEntry<'a>],
    pub suggested_actions: &'a [String],
    pub proof_commands: &'a [String],
}
