use super::ListRow;
use crate::evidence_inventory::policy_reference_diagnostics_for_source_tree;
use allow_core::{
    AllowConfig, AllowEntry, Finding, MatchOutcome, MatchStatus, SimpleDate,
    allow_entry_broad_scope,
};
use allow_diff::selector_precision_score;
use std::collections::BTreeSet;
use std::path::Path;

#[cfg(test)]
pub(super) fn list_rows(
    root: &Path,
    cfg: &AllowConfig,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
) -> Vec<ListRow> {
    list_rows_with_source_tree_files(root, cfg, findings, outcomes, None)
}

pub(super) fn list_rows_with_source_tree_files(
    root: &Path,
    cfg: &AllowConfig,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    evidence_source_tree_files: Option<&BTreeSet<String>>,
) -> Vec<ListRow> {
    let today = SimpleDate::today_utc_approx();
    cfg.allow
        .iter()
        .map(|entry| {
            let evidence_diagnostics = entry_reference_diagnostics_for_source_tree(
                root,
                entry,
                evidence_source_tree_files,
            );
            let entry_outcomes = outcomes
                .iter()
                .filter(|outcome| outcome.allow_id.as_deref() == Some(entry.id.as_str()))
                .collect::<Vec<_>>();
            ListRow {
                id: entry.id.clone(),
                status: list_entry_status(entry, &entry_outcomes, today),
                matches: entry_outcomes
                    .iter()
                    .filter(|outcome| outcome.finding_index.is_some())
                    .count(),
                kind: entry.kind,
                family: entry.family.clone(),
                owner: entry.owner.clone(),
                classification: entry.classification.clone(),
                scope: entry.path_or_glob(),
                source_package: entry_outcomes
                    .iter()
                    .filter_map(|outcome| outcome.finding_index)
                    .filter_map(|index| findings.get(index))
                    .find_map(|finding| finding.source_package_name().map(ToOwned::to_owned)),
                evidence_count: entry.evidence.len(),
                broken_evidence_references: evidence_diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.status.is_broken_local_link())
                    .count(),
                weak_evidence_references: evidence_diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.status.is_weak_reference())
                    .count(),
                selector_precision: selector_precision_score(entry),
                broad_scope: allow_entry_broad_scope(entry).is_some(),
                review_after: entry
                    .lifecycle
                    .review_after
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
                expires: entry
                    .lifecycle
                    .expires
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
                reason: entry.reason.clone(),
            }
        })
        .collect()
}

fn entry_reference_diagnostics_for_source_tree(
    root: &Path,
    entry: &AllowEntry,
    evidence_source_tree_files: Option<&BTreeSet<String>>,
) -> Vec<allow_policy::EvidenceReferenceDiagnostic> {
    policy_reference_diagnostics_for_source_tree(root, entry, evidence_source_tree_files)
        .into_iter()
        .map(|reference| reference.diagnostic)
        .collect()
}

fn list_entry_status(
    entry: &AllowEntry,
    outcomes: &[&MatchOutcome],
    today: SimpleDate,
) -> MatchStatus {
    if date_is_before(entry.lifecycle.expires.as_deref(), today) {
        return MatchStatus::Expired;
    }
    if date_is_due(entry.lifecycle.review_after.as_deref(), today) {
        return MatchStatus::ReviewDue;
    }
    for status in [
        MatchStatus::New,
        MatchStatus::Ambiguous,
        MatchStatus::EvidenceMissing,
        MatchStatus::MissingRequiredField,
        MatchStatus::InvalidSelector,
        MatchStatus::Stale,
    ] {
        if outcomes.iter().any(|outcome| outcome.status == status) {
            return status;
        }
    }
    if entry.classification == "baseline_debt" {
        return MatchStatus::BaselineDebt;
    }
    MatchStatus::Matched
}

fn date_is_before(date: Option<&str>, today: SimpleDate) -> bool {
    SimpleDate::is_before_date_str(date, today)
}

fn date_is_due(date: Option<&str>, today: SimpleDate) -> bool {
    SimpleDate::is_due_date_str(date, today)
}

#[cfg(test)]
#[path = "list_rows_tests.rs"]
mod tests;
