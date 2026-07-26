use super::test_support::{test_entry, test_outcome};
use super::*;

#[test]
fn worklist_filters_by_owner_and_classification() {
    let mut cfg = AllowConfig::empty();
    let mut first = test_entry("allow-first", FindingKind::NonRustFile);
    first.owner = "team-a".to_string();
    first.classification = "baseline_debt".to_string();
    let mut second = test_entry("allow-second", FindingKind::NonRustFile);
    second.owner = "team-b".to_string();
    second.classification = "reviewed_exception".to_string();
    cfg.allow.push(first);
    cfg.allow.push(second);
    let outcomes = vec![
        test_outcome(
            MatchStatus::Stale,
            Some("allow-first"),
            None,
            "allow-first is stale",
        ),
        test_outcome(
            MatchStatus::Stale,
            Some("allow-second"),
            None,
            "allow-second is stale",
        ),
    ];

    let items = work_items_from_outcomes(&cfg, &[], &outcomes);
    let filtered = filter_work_items(
        items,
        WorklistFilters {
            owner: Some("team-a"),
            classification: Some("baseline_debt"),
            ..WorklistFilters::default()
        },
    );

    assert_eq!(filtered.len(), 1);
    let item = filtered
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
    assert_eq!(item.allow_id.as_deref(), Some("allow-first"));
    assert_eq!(item.owner.as_deref(), Some("team-a"));
    assert_eq!(item.classification.as_deref(), Some("baseline_debt"));
}

#[test]
fn worklist_filters_by_advisory_shortcuts() {
    let baseline = WorkItem {
        id: "work-baseline-debt-0001".to_string(),
        kind: "baseline_debt".to_string(),
        exception_kind: Some("panic".to_string()),
        family: Some("unwrap".to_string()),
        owner: Some("runtime".to_string()),
        classification: Some("baseline_debt".to_string()),
        reason: Some("fixture".to_string()),
        created: None,
        review_after: None,
        expires: Some("2026-08-01".to_string()),
        evidence_count: Some(0),
        selector_precision: Some(42),
        risk: "medium",
        difficulty: "medium",
        status: MatchStatus::BaselineDebt,
        allow_id: Some("allow-baseline".to_string()),
        candidate_ids: Vec::new(),
        finding_index: None,
        path: Some("src/lib.rs".to_string()),
        evidence_reference: None,
        source_package: None,
        message: "baseline debt".to_string(),
        suggested_actions: Vec::new(),
        proof_commands: Vec::new(),
        ledger: WorkItemLedger::default(),
        line: None,
        column: None,
    };
    let mut broad = baseline.clone();
    broad.id = "work-broad-scope-0002".to_string();
    broad.kind = "broad_scope".to_string();
    broad.classification = Some("reviewed_exception".to_string());
    broad.status = MatchStatus::Matched;
    broad.allow_id = Some("allow-broad".to_string());
    let mut stale = broad.clone();
    stale.id = "work-stale-0003".to_string();
    stale.kind = "stale_allow".to_string();
    stale.status = MatchStatus::Stale;
    stale.allow_id = Some("allow-stale".to_string());

    let baseline_filtered = filter_work_items(
        vec![baseline.clone(), broad.clone(), stale.clone()],
        WorklistFilters {
            baseline_debt: true,
            ..WorklistFilters::default()
        },
    );
    let broad_filtered = filter_work_items(
        vec![baseline, broad, stale],
        WorklistFilters {
            broad_scope: true,
            ..WorklistFilters::default()
        },
    );

    assert_eq!(baseline_filtered.len(), 1);
    assert_eq!(
        baseline_filtered[0].allow_id.as_deref(),
        Some("allow-baseline")
    );
    assert_eq!(broad_filtered.len(), 1);
    assert_eq!(broad_filtered[0].allow_id.as_deref(), Some("allow-broad"));
}

#[test]
fn worklist_filters_by_missing_evidence() {
    let missing = WorkItem {
        id: "work-missing-evidence-0001".to_string(),
        kind: "missing_evidence".to_string(),
        exception_kind: Some("unsafe".to_string()),
        family: Some("unsafe_block".to_string()),
        owner: Some("runtime".to_string()),
        classification: Some("reviewed_unsafe_boundary".to_string()),
        reason: Some("fixture".to_string()),
        created: None,
        review_after: None,
        expires: None,
        evidence_count: Some(0),
        selector_precision: Some(42),
        risk: "high",
        difficulty: "small",
        status: MatchStatus::EvidenceMissing,
        allow_id: Some("allow-missing".to_string()),
        candidate_ids: Vec::new(),
        finding_index: None,
        path: Some("src/lib.rs".to_string()),
        evidence_reference: None,
        source_package: None,
        message: "allow-missing requires evidence".to_string(),
        suggested_actions: Vec::new(),
        proof_commands: Vec::new(),
        ledger: WorkItemLedger::default(),
        line: None,
        column: None,
    };
    let mut evidenced = missing.clone();
    evidenced.id = "work-review-due-0002".to_string();
    evidenced.kind = "review_due".to_string();
    evidenced.evidence_count = Some(2);
    evidenced.status = MatchStatus::ReviewDue;
    evidenced.allow_id = Some("allow-evidenced".to_string());
    let mut new_finding = missing.clone();
    new_finding.id = "work-new-unreceipted-finding-0003".to_string();
    new_finding.kind = "new_unreceipted_finding".to_string();
    new_finding.evidence_count = None;
    new_finding.status = MatchStatus::New;
    new_finding.allow_id = None;

    let filtered = filter_work_items(
        vec![missing, evidenced, new_finding],
        WorklistFilters {
            missing_evidence: true,
            ..WorklistFilters::default()
        },
    );

    assert_eq!(filtered.len(), 1);
    let item = filtered
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected missing evidence work item"));
    assert_eq!(item.allow_id.as_deref(), Some("allow-missing"));
    assert_eq!(item.evidence_count, Some(0));
}

#[test]
fn worklist_filters_by_evidence_health_shortcuts() {
    let broken = WorkItem {
        id: "work-broken-evidence-link-0001".to_string(),
        kind: "broken_evidence_link".to_string(),
        exception_kind: Some("unsafe".to_string()),
        family: Some("unsafe_block".to_string()),
        owner: Some("runtime".to_string()),
        classification: Some("reviewed_unsafe_boundary".to_string()),
        reason: Some("fixture".to_string()),
        created: None,
        review_after: None,
        expires: None,
        evidence_count: Some(1),
        selector_precision: Some(42),
        risk: "high",
        difficulty: "small",
        status: MatchStatus::Matched,
        allow_id: Some("allow-broken".to_string()),
        candidate_ids: Vec::new(),
        finding_index: None,
        path: Some("docs/missing.md".to_string()),
        evidence_reference: None,
        source_package: None,
        message: "broken evidence".to_string(),
        suggested_actions: Vec::new(),
        proof_commands: Vec::new(),
        ledger: WorkItemLedger::default(),
        line: None,
        column: None,
    };
    let mut weak = broken.clone();
    weak.id = "work-weak-evidence-reference-0002".to_string();
    weak.kind = "weak_evidence_reference".to_string();
    weak.allow_id = Some("allow-weak".to_string());
    weak.path = None;
    weak.message = "weak evidence".to_string();

    let broken_filtered = filter_work_items(
        vec![broken.clone(), weak.clone()],
        WorklistFilters {
            broken_evidence: true,
            ..WorklistFilters::default()
        },
    );
    let weak_filtered = filter_work_items(
        vec![broken, weak],
        WorklistFilters {
            weak_evidence: true,
            ..WorklistFilters::default()
        },
    );

    assert_eq!(broken_filtered.len(), 1);
    assert_eq!(broken_filtered[0].allow_id.as_deref(), Some("allow-broken"));
    assert_eq!(broken_filtered[0].kind, "broken_evidence_link");
    assert_eq!(weak_filtered.len(), 1);
    assert_eq!(weak_filtered[0].allow_id.as_deref(), Some("allow-weak"));
    assert_eq!(weak_filtered[0].kind, "weak_evidence_reference");
}
