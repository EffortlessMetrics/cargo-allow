use allow_core::{AllowEntry, Finding, MatchOutcome, MatchStatus};

use crate::worklist;

pub(super) fn explain_next_steps(
    entry: &AllowEntry,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
) -> (Vec<String>, Vec<String>) {
    let attention = outcomes
        .iter()
        .filter(|outcome| outcome.status != MatchStatus::Matched)
        .collect::<Vec<_>>();
    if let Some(outcome) = attention.first() {
        let finding = outcome.finding_index.and_then(|index| findings.get(index));
        let kind = worklist::work_item_kind(outcome, finding, Some(entry));
        return (
            worklist::suggested_actions(&kind)
                .into_iter()
                .take(2)
                .collect(),
            worklist::proof_commands(&kind, finding, Some(entry))
                .into_iter()
                .take(3)
                .collect(),
        );
    }
    if entry.classification == "baseline_debt" {
        let finding = findings.first();
        let kind = "baseline_debt";
        return (
            worklist::suggested_actions(kind)
                .into_iter()
                .take(2)
                .collect(),
            worklist::proof_commands(kind, finding, Some(entry))
                .into_iter()
                .take(3)
                .collect(),
        );
    }
    (Vec::new(), Vec::new())
}
