use super::WorkItem;
use super::worklist_actions::{proof_commands, suggested_actions};
use super::worklist_item_kind::{BASELINE_DEBT, BROAD_SCOPE, MISSING_EVIDENCE};
use super::worklist_priority::{DIFFICULTY_SMALL, RISK_MEDIUM};
use super::worklist_scoring::{exception_family, work_item_difficulty, work_item_risk};
use allow_core::{
    AllowConfig, AllowEntry, Finding, MatchOutcome, MatchStatus, allow_entry_broad_scope,
    normalize_path,
};

pub(super) fn work_items_from_policy_advisories(
    cfg: &AllowConfig,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    start_index: usize,
    include_missing_evidence: bool,
) -> Vec<WorkItem> {
    let mut items = Vec::new();
    for entry in &cfg.allow {
        let Some(outcome) = matched_outcome_for_entry(outcomes, entry) else {
            continue;
        };
        let finding = outcome.finding_index.and_then(|idx| findings.get(idx));
        if entry.classification == "baseline_debt" {
            let item_index = start_index + items.len();
            let kind = BASELINE_DEBT.to_string();
            items.push(WorkItem {
                id: format!("work-baseline-debt-{item_index:04}"),
                exception_kind: Some(entry.kind.as_str().to_string()),
                family: exception_family(finding, Some(entry)).map(ToOwned::to_owned),
                owner: Some(entry.owner.clone()),
                classification: Some(entry.classification.clone()),
                reason: Some(entry.reason.clone()),
                created: entry.lifecycle.created.clone(),
                review_after: entry.lifecycle.review_after.clone(),
                expires: entry.lifecycle.expires.clone(),
                evidence_count: Some(entry.evidence.len()),
                risk: work_item_risk(&kind, MatchStatus::BaselineDebt, finding, Some(entry)),
                difficulty: work_item_difficulty(&kind, finding, Some(entry)),
                status: MatchStatus::BaselineDebt,
                allow_id: Some(entry.id.clone()),
                finding_index: outcome.finding_index,
                path: finding
                    .map(|finding| normalize_path(&finding.path))
                    .or_else(|| Some(entry.path_or_glob())),
                source_package: source_package_name(finding),
                message: format!(
                    "{} is generated baseline_debt and still needs human review",
                    entry.id
                ),
                suggested_actions: suggested_actions(&kind),
                proof_commands: proof_commands(&kind, finding, Some(entry)),
                kind,
            });
            continue;
        }
        if include_missing_evidence && entry.evidence.is_empty() {
            let item_index = start_index + items.len();
            let kind = MISSING_EVIDENCE.to_string();
            items.push(WorkItem {
                id: format!("work-missing-evidence-{item_index:04}"),
                exception_kind: Some(entry.kind.as_str().to_string()),
                family: exception_family(finding, Some(entry)).map(ToOwned::to_owned),
                owner: Some(entry.owner.clone()),
                classification: Some(entry.classification.clone()),
                reason: Some(entry.reason.clone()),
                created: entry.lifecycle.created.clone(),
                review_after: entry.lifecycle.review_after.clone(),
                expires: entry.lifecycle.expires.clone(),
                evidence_count: Some(0),
                risk: work_item_risk(&kind, MatchStatus::EvidenceMissing, finding, Some(entry)),
                difficulty: work_item_difficulty(&kind, finding, Some(entry)),
                status: MatchStatus::EvidenceMissing,
                allow_id: Some(entry.id.clone()),
                finding_index: outcome.finding_index,
                path: finding
                    .map(|finding| normalize_path(&finding.path))
                    .or_else(|| Some(entry.path_or_glob())),
                source_package: source_package_name(finding),
                message: format!("{} has no evidence references", entry.id),
                suggested_actions: suggested_actions(&kind),
                proof_commands: proof_commands(&kind, finding, Some(entry)),
                kind,
            });
        }
        if let Some(scope) = allow_entry_broad_scope(entry) {
            let item_index = start_index + items.len();
            let kind = BROAD_SCOPE.to_string();
            items.push(WorkItem {
                id: format!("work-broad-scope-{item_index:04}"),
                kind,
                exception_kind: Some(entry.kind.as_str().to_string()),
                family: entry.family.clone(),
                owner: Some(entry.owner.clone()),
                classification: Some(entry.classification.clone()),
                reason: Some(entry.reason.clone()),
                created: entry.lifecycle.created.clone(),
                review_after: entry.lifecycle.review_after.clone(),
                expires: entry.lifecycle.expires.clone(),
                evidence_count: Some(entry.evidence.len()),
                risk: RISK_MEDIUM,
                difficulty: DIFFICULTY_SMALL,
                status: MatchStatus::Matched,
                allow_id: Some(entry.id.clone()),
                finding_index: outcome.finding_index,
                path: Some(scope.clone()),
                source_package: source_package_name(finding),
                message: format!("{} uses a broad source-tree scope `{}`", entry.id, scope),
                suggested_actions: suggested_actions(BROAD_SCOPE),
                proof_commands: proof_commands(BROAD_SCOPE, finding, Some(entry)),
            });
        }
    }
    items
}

fn matched_outcome_for_entry<'a>(
    outcomes: &'a [MatchOutcome],
    entry: &AllowEntry,
) -> Option<&'a MatchOutcome> {
    outcomes.iter().find(|outcome| {
        outcome.status == MatchStatus::Matched
            && outcome.allow_id.as_deref() == Some(entry.id.as_str())
    })
}

fn source_package_name(finding: Option<&Finding>) -> Option<String> {
    finding.and_then(|finding| finding.source_package_name().map(ToOwned::to_owned))
}
