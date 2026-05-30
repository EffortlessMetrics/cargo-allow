use super::WorkItem;
use super::worklist_actions::{proof_commands, suggested_actions};
use super::worklist_scoring::{
    exception_family, work_item_difficulty, work_item_kind, work_item_risk,
};
use allow_core::{AllowConfig, AllowEntry, Finding, MatchOutcome, MatchStatus, normalize_path};

pub(super) fn work_items_from_outcomes(
    cfg: &AllowConfig,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
) -> Vec<WorkItem> {
    outcomes
        .iter()
        .filter(|outcome| outcome.status != MatchStatus::Matched)
        .enumerate()
        .map(|(index, outcome)| work_item_from_outcome(index + 1, cfg, findings, outcome))
        .collect()
}

fn work_item_from_outcome(
    item_index: usize,
    cfg: &AllowConfig,
    findings: &[Finding],
    outcome: &MatchOutcome,
) -> WorkItem {
    let finding = outcome.finding_index.and_then(|idx| findings.get(idx));
    let entry = outcome
        .allow_id
        .as_deref()
        .and_then(|id| cfg.allow.iter().find(|entry| entry.id == id));
    let kind = work_item_kind(outcome, finding, entry);
    let path = finding
        .map(|finding| normalize_path(&finding.path))
        .or_else(|| entry.map(|entry| entry.path_or_glob()));
    let source_package =
        finding.and_then(|finding| finding.source_package_name().map(ToOwned::to_owned));
    let exception_kind = work_item_exception_kind(finding, entry);
    let family = exception_family(finding, entry).map(ToOwned::to_owned);
    let mut suggested_actions = suggested_actions(&kind);
    if let Some(package) = &source_package {
        suggested_actions.push(format!(
            "focus source-tree review on package `{package}` without assuming Cargo metadata"
        ));
    }
    WorkItem {
        id: format!("work-{}-{item_index:04}", kind.replace('_', "-")),
        exception_kind,
        family,
        owner: entry.map(|entry| entry.owner.clone()),
        classification: entry.map(|entry| entry.classification.clone()),
        reason: entry.map(|entry| entry.reason.clone()),
        created: entry.and_then(|entry| entry.lifecycle.created.clone()),
        review_after: entry.and_then(|entry| entry.lifecycle.review_after.clone()),
        expires: entry.and_then(|entry| entry.lifecycle.expires.clone()),
        evidence_count: entry.map(|entry| entry.evidence.len()),
        risk: work_item_risk(&kind, outcome.status, finding, entry),
        difficulty: work_item_difficulty(&kind, finding, entry),
        status: outcome.status,
        allow_id: outcome.allow_id.clone(),
        finding_index: outcome.finding_index,
        path,
        evidence_reference: None,
        source_package,
        message: outcome.message.clone(),
        suggested_actions,
        proof_commands: proof_commands(&kind, finding, entry),
        kind,
    }
}

fn work_item_exception_kind(
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
) -> Option<String> {
    finding
        .map(|finding| finding.kind.as_str())
        .or_else(|| entry.map(|entry| entry.kind.as_str()))
        .map(ToOwned::to_owned)
}
