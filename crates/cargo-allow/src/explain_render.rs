use super::{ExplainContext, explain_steps::explain_next_steps};
use allow_core::{AllowEntry, Finding, MatchOutcome, allow_entry_broad_scope, normalize_path};
use allow_diff::selector_precision_score;
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
    let evidence_diagnostics = evidence_reference_diagnostics(root, entry);
    let has_broken_evidence = evidence_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.status.is_broken_local_link());
    let has_weak_evidence = evidence_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.status.is_weak_reference());
    let (suggested_actions, proof_commands) = explain_next_steps(
        entry,
        findings,
        outcomes,
        has_broken_evidence,
        has_weak_evidence,
    );
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
        inventory: context.inventory,
        entry,
        selector_precision: selector_precision_score(entry),
        broad_scope: allow_entry_broad_scope(entry).is_some(),
        current_findings: findings,
        match_outcomes: outcomes,
        evidence_references: &evidence_references,
        suggested_actions: &suggested_actions,
        proof_commands: &proof_commands,
    })
}
