use allow_core::{AllowEntry, Finding, MatchOutcome, MatchStatus};

use crate::worklist;

pub(super) fn explain_next_steps(
    entry: &AllowEntry,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    has_broken_evidence: bool,
    has_weak_evidence: bool,
    has_evidence_outside_default_inventory: bool,
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
                .take(5)
                .collect(),
        );
    }
    if has_evidence_outside_default_inventory {
        let finding = findings.first();
        let kind = "broken_evidence_link";
        return (
            vec![
                "commit the referenced evidence file if it should support repository policy"
                    .to_string(),
                "or rerun with --include-untracked when intentionally reviewing local receipt artifacts"
                    .to_string(),
            ],
            untracked_evidence_proof_commands(entry, finding, kind),
        );
    }
    if has_broken_evidence {
        let finding = findings.first();
        let kind = "broken_evidence_link";
        return (
            worklist::suggested_actions(kind)
                .into_iter()
                .take(2)
                .collect(),
            worklist::proof_commands(kind, finding, Some(entry))
                .into_iter()
                .take(5)
                .collect(),
        );
    }
    if has_weak_evidence {
        let finding = findings.first();
        let kind = "weak_evidence_reference";
        return (
            worklist::suggested_actions(kind)
                .into_iter()
                .take(2)
                .collect(),
            worklist::proof_commands(kind, finding, Some(entry))
                .into_iter()
                .take(5)
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
                .take(5)
                .collect(),
        );
    }
    (Vec::new(), Vec::new())
}

fn untracked_evidence_proof_commands(
    entry: &AllowEntry,
    finding: Option<&Finding>,
    kind: &str,
) -> Vec<String> {
    let mut commands = vec![
        format!("cargo-allow explain {}", entry.id),
        format!("cargo-allow explain {} --include-untracked", entry.id),
        format!("cargo-allow worklist --allow-id {} --format json", entry.id),
        "cargo-allow check --include-untracked --mode no-new".to_string(),
    ];
    for command in worklist::proof_commands(kind, finding, Some(entry)) {
        if !commands.iter().any(|existing| existing == &command) {
            commands.push(command);
        }
    }
    commands
}
