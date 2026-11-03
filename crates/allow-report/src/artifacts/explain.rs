use allow_core::{AllowEntry, Finding, MatchOutcome, SimpleDate};

use crate::InventoryContext;

#[derive(Debug, Clone, Copy, Default)]
pub struct EvidenceReference<'a> {
    pub raw: &'a str,
    pub prefix: Option<&'a str>,
    pub target: Option<&'a str>,
    pub status: &'a str,
    pub category: &'a str,
    pub message: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct ExplainReport<'a> {
    pub inventory: InventoryContext<'a>,
    pub entry: &'a AllowEntry,
    /// The command-boundary as-of date (#4237). The detailed human/JSON
    /// renderers and the common summary must project `entry` lifecycle from
    /// the one date the command captured before evaluation, never from a
    /// fresh clock read inside a renderer.
    pub as_of: SimpleDate,
    pub selector_precision: u32,
    pub broad_scope: bool,
    pub current_findings: &'a [Finding],
    pub match_outcomes: &'a [MatchOutcome],
    pub evidence_references: &'a [EvidenceReference<'a>],
    pub link_references: &'a [EvidenceReference<'a>],
    pub suggested_actions: &'a [String],
    pub proof_commands: &'a [String],
}
