use super::check_failed_for_outcomes;
use allow_core::{
    AllowConfig, AllowEntry, Finding, FindingKind, LaneConfig, LaneEnforcementMode, Lifecycle,
    MatchOutcome, MatchStatus, Selector,
};
use allow_match::CheckMode;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn shadow_lane_does_not_fail_no_new_on_new_findings() {
    let mut lanes = BTreeMap::new();
    lanes.insert(
        "unsafe".to_string(),
        LaneConfig {
            mode: LaneEnforcementMode::Shadow,
        },
    );
    let cfg = AllowConfig {
        lanes,
        ..AllowConfig::empty()
    };
    let findings = vec![unsafe_finding()];
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::New,
        allow_id: None,
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "unreceipted unsafe".to_string(),
        score: 0,
    }];

    assert!(!check_failed_for_outcomes(
        &outcomes,
        &findings,
        &cfg,
        CheckMode::NoNew
    ));
}

#[test]
fn blocking_lane_still_fails_no_new_on_new_findings() {
    let cfg = AllowConfig::empty();
    let findings = vec![panic_finding()];
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::New,
        allow_id: None,
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "unreceipted panic".to_string(),
        score: 0,
    }];

    assert!(check_failed_for_outcomes(
        &outcomes,
        &findings,
        &cfg,
        CheckMode::NoNew
    ));
}

#[test]
fn expired_matched_policy_fails_no_new() {
    let entry = lifecycle_entry("allow-expired", Some("2020-01-01"), None);
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry.clone());
    let findings = vec![panic_finding()];
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Matched,
        allow_id: Some(entry.id),
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "matched expired entry".to_string(),
        score: 100,
    }];

    assert!(check_failed_for_outcomes(
        &outcomes,
        &findings,
        &cfg,
        CheckMode::NoNew
    ));
}

#[test]
fn review_due_matched_policy_is_advisory_in_no_new_but_fails_strict() {
    let entry = lifecycle_entry("allow-review", None, Some("2020-01-01"));
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry.clone());
    let findings = vec![panic_finding()];
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Matched,
        allow_id: Some(entry.id),
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "matched review-due entry".to_string(),
        score: 100,
    }];

    assert!(!check_failed_for_outcomes(
        &outcomes,
        &findings,
        &cfg,
        CheckMode::NoNew
    ));
    assert!(check_failed_for_outcomes(
        &outcomes,
        &findings,
        &cfg,
        CheckMode::Strict
    ));
}

#[test]
fn unresolved_outcome_does_not_inherit_policy_exception_shadow_lane() {
    let mut lanes = BTreeMap::new();
    lanes.insert(
        "policy_exception".to_string(),
        LaneConfig {
            mode: LaneEnforcementMode::Shadow,
        },
    );
    let cfg = AllowConfig {
        lanes,
        ..AllowConfig::empty()
    };
    let findings = vec![panic_finding()];
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::New,
        allow_id: Some("allow-deleted".to_string()),
        candidate_ids: Vec::new(),
        finding_index: Some(99),
        message: "unreceipted panic with stale outcome links".to_string(),
        score: 0,
    }];

    assert!(check_failed_for_outcomes(
        &outcomes,
        &findings,
        &cfg,
        CheckMode::NoNew
    ));
}

#[test]
fn stale_outcome_is_advisory_in_no_new_by_default() {
    let cfg = AllowConfig::empty();
    let findings = vec![panic_finding()];
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Stale,
        allow_id: Some("allow-stale".to_string()),
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "stale entry".to_string(),
        score: 0,
    }];

    assert!(!check_failed_for_outcomes(
        &outcomes,
        &findings,
        &cfg,
        CheckMode::NoNew
    ));
}

#[test]
fn stale_outcome_fails_no_new_when_stale_entries_fail_enabled() {
    let mut cfg = AllowConfig::empty();
    cfg.requirements.stale_entries_fail = true;
    let findings = vec![panic_finding()];
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Stale,
        allow_id: Some("allow-stale".to_string()),
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "stale entry".to_string(),
        score: 0,
    }];

    assert!(check_failed_for_outcomes(
        &outcomes,
        &findings,
        &cfg,
        CheckMode::NoNew
    ));
}

#[test]
fn stale_outcome_fails_audit_when_stale_entries_fail_enabled() {
    let mut cfg = AllowConfig::empty();
    cfg.requirements.stale_entries_fail = true;
    let findings = vec![panic_finding()];
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Stale,
        allow_id: Some("allow-stale".to_string()),
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "stale entry".to_string(),
        score: 0,
    }];

    assert!(check_failed_for_outcomes(
        &outcomes,
        &findings,
        &cfg,
        CheckMode::Audit
    ));
}

#[test]
fn stale_outcome_is_advisory_in_audit_by_default() {
    let cfg = AllowConfig::empty();
    let findings = vec![panic_finding()];
    let outcomes = vec![MatchOutcome {
        status: MatchStatus::Stale,
        allow_id: Some("allow-stale".to_string()),
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "stale entry".to_string(),
        score: 0,
    }];

    assert!(!check_failed_for_outcomes(
        &outcomes,
        &findings,
        &cfg,
        CheckMode::Audit
    ));
}

fn lifecycle_entry(id: &str, expires: Option<&str>, review_after: Option<&str>) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "owner".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "lifecycle test entry".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: None,
            review_after: review_after.map(str::to_string),
            expires: expires.map(str::to_string),
        },
        selector: Selector::default(),
        last_seen: None,
    }
}

fn panic_finding() -> Finding {
    Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: None,
        identity: allow_core::StructuralIdentity::new("rust", "method_call"),
        message: "unwrap".to_string(),
        ledger: None,
    }
}

fn unsafe_finding() -> Finding {
    Finding {
        kind: FindingKind::Unsafe,
        family: Some("unsafe_block".to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: None,
        identity: allow_core::StructuralIdentity::new("rust", "unsafe_block"),
        message: "unsafe".to_string(),
        ledger: None,
    }
}
