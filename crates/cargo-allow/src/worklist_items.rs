use super::WorkItem;
use super::worklist_actions::{proof_commands, suggested_actions_for_context};
use super::worklist_scoring::{
    exception_family, work_item_difficulty, work_item_kind_for_status, work_item_risk,
};
use super::worklist_types::WorkItemLedger;
use allow_core::{AllowConfig, AllowEntry, Finding, MatchOutcome, MatchStatus, normalize_path};
use allow_diff::selector_precision_score;

pub(super) fn work_items_from_outcomes(
    cfg: &AllowConfig,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
) -> Vec<WorkItem> {
    let projected_statuses = allow_report::ledger_read_statuses(
        cfg,
        outcomes,
        allow_core::SimpleDate::today_utc_approx(),
    );

    outcomes
        .iter()
        .filter_map(|outcome| {
            let status = outcome
                .allow_id
                .as_deref()
                .and_then(|id| projected_statuses.get(id).copied())
                .unwrap_or(outcome.status);
            let actionable = outcome.status != MatchStatus::Matched
                || matches!(status, MatchStatus::Expired | MatchStatus::ReviewDue);
            actionable.then_some((status, outcome))
        })
        .enumerate()
        .map(|(index, (status, outcome))| {
            work_item_from_outcome_with_status(index + 1, cfg, findings, outcome, status)
        })
        .collect()
}

#[cfg(test)]
fn work_item_from_outcome(
    item_index: usize,
    cfg: &AllowConfig,
    findings: &[Finding],
    outcome: &MatchOutcome,
) -> WorkItem {
    work_item_from_outcome_with_status(item_index, cfg, findings, outcome, outcome.status)
}

fn work_item_from_outcome_with_status(
    item_index: usize,
    cfg: &AllowConfig,
    findings: &[Finding],
    outcome: &MatchOutcome,
    status: MatchStatus,
) -> WorkItem {
    let finding = outcome.finding_index.and_then(|idx| findings.get(idx));
    let entry = outcome
        .allow_id
        .as_deref()
        .and_then(|id| cfg.allow.iter().find(|entry| entry.id == id));
    let kind = work_item_kind_for_status(status, outcome, finding, entry);
    let path = finding
        .map(|finding| normalize_path(&finding.path))
        .or_else(|| entry.map(|entry| entry.path_or_glob()));
    let (line, column) = finding
        .and_then(|finding| finding.span.as_ref())
        .map(|span| (Some(span.line), Some(span.column)))
        .unwrap_or((None, None));
    let source_package =
        finding.and_then(|finding| finding.source_package_name().map(ToOwned::to_owned));
    let exception_kind = work_item_exception_kind(finding, entry);
    let family = exception_family(finding, entry).map(ToOwned::to_owned);
    let mut suggested_actions = suggested_actions_for_context(&kind, finding, entry);
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
        selector_precision: entry.map(selector_precision_score),
        risk: work_item_risk(&kind, status, finding, entry),
        difficulty: work_item_difficulty(&kind, finding, entry),
        status,
        allow_id: outcome.allow_id.clone(),
        candidate_ids: outcome.candidate_ids.clone(),
        finding_index: outcome.finding_index,
        path,
        line,
        column,
        evidence_reference: None,
        source_package,
        message: if status == outcome.status {
            outcome.message.clone()
        } else {
            format!(
                "{} is {}",
                outcome.allow_id.as_deref().unwrap_or("policy entry"),
                status.as_str()
            )
        },
        suggested_actions,
        proof_commands: proof_commands(&kind, finding, entry),
        ledger: WorkItemLedger::from_finding(finding),
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

#[cfg(test)]
mod tests {
    use super::super::test_support::{test_entry, test_finding, test_outcome};
    use super::work_item_from_outcome;
    use allow_core::{AllowConfig, FindingKind, Lifecycle, MatchStatus};
    use std::path::PathBuf;

    #[test]
    fn work_item_from_outcome_carries_entry_metadata_and_finding_context() {
        let mut cfg = AllowConfig::empty();
        let mut entry = test_entry("allow-process", FindingKind::PolicyException);
        entry.family = Some("process_spawn".to_string());
        entry.path = Some(PathBuf::from(".github/workflows/ci.yml"));
        entry.owner = "security".to_string();
        entry.classification = "reviewed_exception".to_string();
        entry.reason = "CI workflow needs a process policy exception.".to_string();
        entry.evidence = vec![
            "spec:CARGO-ALLOW-SPEC-0001".to_string(),
            "test:ci-policy".to_string(),
        ];
        entry.lifecycle = Lifecycle {
            created: Some("2026-06-01".to_string()),
            review_after: Some("2026-07-01".to_string()),
            expires: Some("2026-09-01".to_string()),
        };
        cfg.allow.push(entry);

        let mut finding = test_finding(
            FindingKind::PolicyException,
            Some("process_spawn"),
            ".github/workflows/ci.yml",
            "process_spawn",
        );
        finding.identity.crate_name = Some("workflow".to_string());
        let findings = vec![finding];
        let mut outcome = test_outcome(
            MatchStatus::EvidenceMissing,
            Some("allow-process"),
            Some(0),
            "allow-process is missing typed evidence",
        );
        outcome.candidate_ids = vec!["allow-process".to_string()];

        let item = work_item_from_outcome(7, &cfg, &findings, &outcome);

        assert_eq!(item.id, "work-missing-evidence-0007");
        assert_eq!(item.kind, "missing_evidence");
        assert_eq!(item.exception_kind.as_deref(), Some("policy_exception"));
        assert_eq!(item.family.as_deref(), Some("process_spawn"));
        assert_eq!(item.owner.as_deref(), Some("security"));
        assert_eq!(item.classification.as_deref(), Some("reviewed_exception"));
        assert_eq!(
            item.reason.as_deref(),
            Some("CI workflow needs a process policy exception.")
        );
        assert_eq!(item.created.as_deref(), Some("2026-06-01"));
        assert_eq!(item.review_after.as_deref(), Some("2026-07-01"));
        assert_eq!(item.expires.as_deref(), Some("2026-09-01"));
        assert_eq!(item.evidence_count, Some(2));
        assert_eq!(item.risk, "high");
        assert_eq!(item.difficulty, "small");
        assert_eq!(item.status, MatchStatus::EvidenceMissing);
        assert_eq!(item.allow_id.as_deref(), Some("allow-process"));
        assert_eq!(item.candidate_ids, vec!["allow-process".to_string()]);
        assert_eq!(item.finding_index, Some(0));
        assert_eq!(item.path.as_deref(), Some(".github/workflows/ci.yml"));
        assert_eq!(item.source_package.as_deref(), Some("workflow"));
        assert_eq!(item.message, "allow-process is missing typed evidence");
        assert!(
            item.suggested_actions
                .iter()
                .any(|action| action.contains("policy_exception.process_spawn"))
        );
        assert!(
            item.suggested_actions
                .iter()
                .any(|action| action.contains("package `workflow`"))
        );
    }

    #[test]
    fn work_item_from_outcome_uses_entry_path_when_finding_is_absent() {
        let mut cfg = AllowConfig::empty();
        let mut entry = test_entry("allow-stale", FindingKind::GeneratedCode);
        entry.family = Some("checked_in_fixture".to_string());
        entry.path = None;
        entry.glob = Some("fixtures/generated/**".to_string());
        cfg.allow.push(entry);
        let outcome = test_outcome(
            MatchStatus::Stale,
            Some("allow-stale"),
            None,
            "allow-stale is stale",
        );

        let item = work_item_from_outcome(2, &cfg, &[], &outcome);

        assert_eq!(item.id, "work-stale-allow-0002");
        assert_eq!(item.kind, "stale_allow");
        assert_eq!(item.exception_kind.as_deref(), Some("generated_code"));
        assert_eq!(item.family.as_deref(), Some("checked_in_fixture"));
        assert_eq!(item.risk, "low");
        assert_eq!(item.difficulty, "small");
        assert_eq!(item.status, MatchStatus::Stale);
        assert_eq!(item.allow_id.as_deref(), Some("allow-stale"));
        assert_eq!(item.finding_index, None);
        assert_eq!(item.path.as_deref(), Some("fixtures/generated/**"));
        assert_eq!(item.source_package, None);
        assert_eq!(item.message, "allow-stale is stale");
    }

    #[test]
    fn matched_outcome_for_expired_entry_remains_actionable() {
        let mut cfg = AllowConfig::empty();
        let mut entry = test_entry("allow-expired", FindingKind::Panic);
        entry.lifecycle.expires = Some("2020-01-01".to_string());
        cfg.allow.push(entry);
        let outcome = test_outcome(
            MatchStatus::Matched,
            Some("allow-expired"),
            Some(0),
            "allow-expired matched",
        );

        let items = super::super::work_items_from_outcomes(
            &cfg,
            &[test_finding(
                FindingKind::Panic,
                Some("unwrap"),
                "tracked.file",
                "unwrap",
            )],
            &[outcome],
        );

        match items.as_slice() {
            [item] => {
                assert_eq!(item.status, MatchStatus::Expired);
                assert_eq!(item.kind, "expired_allow");
            }
            items => assert_eq!(items.len(), 1),
        }
    }

    #[test]
    fn matched_outcome_for_review_due_entry_remains_actionable() {
        let mut cfg = AllowConfig::empty();
        let mut entry = test_entry("allow-review", FindingKind::Panic);
        entry.lifecycle.review_after = Some("2020-01-01".to_string());
        cfg.allow.push(entry);
        let outcome = test_outcome(
            MatchStatus::Matched,
            Some("allow-review"),
            Some(0),
            "allow-review matched",
        );

        let items = super::super::work_items_from_outcomes(
            &cfg,
            &[test_finding(
                FindingKind::Panic,
                Some("unwrap"),
                "tracked.file",
                "unwrap",
            )],
            &[outcome],
        );

        match items.as_slice() {
            [item] => {
                assert_eq!(item.status, MatchStatus::ReviewDue);
                assert_eq!(item.kind, "review_due");
            }
            items => assert_eq!(items.len(), 1),
        }
    }
}
