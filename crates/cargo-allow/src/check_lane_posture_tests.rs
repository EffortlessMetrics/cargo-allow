use super::check_failed_for_outcomes;
use allow_core::{
    AllowConfig, Finding, FindingKind, LaneConfig, LaneEnforcementMode, MatchOutcome, MatchStatus,
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
