use super::{ExplainContext, explain_steps::explain_next_steps};
use crate::evidence_inventory::{
    DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE, evidence_reference_diagnostics_for_source_tree,
};
use crate::evidence_render::evidence_reference_target_text;
use allow_core::{AllowEntry, Finding, MatchOutcome, allow_entry_broad_scope};
use allow_diff::selector_precision_score;
use std::collections::BTreeSet;
use std::path::Path;

pub(super) fn render_explain_entry(
    root: &Path,
    entry: &AllowEntry,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    evidence_source_tree_files: Option<&BTreeSet<String>>,
) -> String {
    render_explain_report(
        root,
        entry,
        findings,
        outcomes,
        evidence_source_tree_files,
        ExplainContext::default(),
        allow_report::render_explain_human,
    )
}

pub(super) fn render_explain_entry_json(
    root: &Path,
    entry: &AllowEntry,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    evidence_source_tree_files: Option<&BTreeSet<String>>,
    context: ExplainContext<'_>,
) -> String {
    render_explain_report(
        root,
        entry,
        findings,
        outcomes,
        evidence_source_tree_files,
        context,
        allow_report::render_explain_json,
    )
}

fn render_explain_report<R>(
    root: &Path,
    entry: &AllowEntry,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    evidence_source_tree_files: Option<&BTreeSet<String>>,
    context: ExplainContext<'_>,
    render: impl FnOnce(allow_report::ExplainReport<'_>) -> R,
) -> R {
    let evidence_diagnostics =
        evidence_reference_diagnostics_for_source_tree(root, entry, evidence_source_tree_files);
    let has_broken_evidence = evidence_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.status.is_broken_local_link());
    let has_weak_evidence = evidence_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.status.is_weak_reference());
    let has_evidence_outside_default_inventory = evidence_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message == DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE);
    let (suggested_actions, proof_commands) = explain_next_steps(
        entry,
        findings,
        outcomes,
        has_broken_evidence,
        has_weak_evidence,
        has_evidence_outside_default_inventory,
    );
    let normalized_targets = evidence_diagnostics
        .iter()
        .map(evidence_reference_target_text)
        .collect::<Vec<_>>();
    let evidence_references = evidence_diagnostics
        .iter()
        .zip(normalized_targets.iter())
        .map(|(diagnostic, target)| allow_report::EvidenceReference {
            raw: &diagnostic.raw,
            prefix: diagnostic.prefix.as_deref(),
            target: target.as_deref(),
            status: diagnostic.status.as_str(),
            category: diagnostic.category.as_str(),
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
