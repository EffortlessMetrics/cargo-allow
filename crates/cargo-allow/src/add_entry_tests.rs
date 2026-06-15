use super::{ensure_addable_outcome, next_allow_id};
use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, FindingKind, Lifecycle, MatchStatus, Selector,
};
use std::path::PathBuf;

fn entry_with_id(id: &str) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "owner".to_string(),
        classification: "reviewed".to_string(),
        reason: "fixture".to_string(),
        evidence: vec!["test:fixture".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector::default(),
        last_seen: None,
    }
}

#[test]
fn ensure_addable_outcome_boundary_accepts_new_status() {
    assert_eq!(ensure_addable_outcome(MatchStatus::New), Ok(()));
}

#[test]
fn ensure_addable_outcome_rejects_matched_findings_with_exact_error() {
    assert_eq!(
        ensure_addable_outcome(MatchStatus::Matched),
        Err(CargoAllowError::new(format!(
            "selected finding is already receipted or blocked with status `{}`; use list or explain before editing policy",
            MatchStatus::Matched.as_str()
        )))
    );
}

#[test]
fn ensure_addable_outcome_rejects_stale_findings_with_exact_error() {
    assert_eq!(
        ensure_addable_outcome(MatchStatus::Stale),
        Err(CargoAllowError::new(format!(
            "selected finding is already receipted or blocked with status `{}`; use list or explain before editing policy",
            MatchStatus::Stale.as_str()
        )))
    );
}

#[test]
fn next_allow_id_returns_first_available_candidate() {
    let cfg = AllowConfig::empty();

    assert_eq!(next_allow_id(&cfg), "allow-0001");
}

#[test]
fn next_allow_id_skips_existing_allow_ids() {
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry_with_id("allow-0001"));

    assert_eq!(next_allow_id(&cfg), "allow-0002");
}
