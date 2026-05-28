use super::ExplainContext;
use crate::worklist;
use allow_core::{AllowEntry, Finding, MatchOutcome, MatchStatus, normalize_path};
use allow_policy::evidence_reference_diagnostics;
use std::path::Path;

pub(super) fn render_explain_entry(
    root: &Path,
    entry: &AllowEntry,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
) -> String {
    render_explain_report(
        root,
        entry,
        findings,
        outcomes,
        ExplainContext::default(),
        allow_report::render_explain_human,
    )
}

pub(super) fn render_explain_entry_json(
    root: &Path,
    entry: &AllowEntry,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    context: ExplainContext<'_>,
) -> String {
    render_explain_report(
        root,
        entry,
        findings,
        outcomes,
        context,
        allow_report::render_explain_json,
    )
}

fn render_explain_report<R>(
    root: &Path,
    entry: &AllowEntry,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    context: ExplainContext<'_>,
    render: impl FnOnce(allow_report::ExplainReport<'_>) -> R,
) -> R {
    let (suggested_actions, proof_commands) = explain_next_steps(entry, findings, outcomes);
    let evidence_diagnostics = evidence_reference_diagnostics(root, entry);
    let normalized_targets = evidence_diagnostics
        .iter()
        .map(|diagnostic| diagnostic.target.as_ref().map(normalize_path))
        .collect::<Vec<_>>();
    let evidence_references = evidence_diagnostics
        .iter()
        .zip(normalized_targets.iter())
        .map(|(diagnostic, target)| allow_report::EvidenceReference {
            raw: &diagnostic.raw,
            prefix: diagnostic.prefix.as_deref(),
            target: target.as_deref(),
            status: diagnostic.status.as_str(),
            message: &diagnostic.message,
        })
        .collect::<Vec<_>>();

    render(allow_report::ExplainReport {
        inventory: allow_report::InventoryContext::source_syntax(
            context.inventory_source,
            context.source_tree_root,
            context.inventory_files,
        ),
        entry,
        current_findings: findings,
        match_outcomes: outcomes,
        evidence_references: &evidence_references,
        suggested_actions: &suggested_actions,
        proof_commands: &proof_commands,
    })
}

fn explain_next_steps(
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
