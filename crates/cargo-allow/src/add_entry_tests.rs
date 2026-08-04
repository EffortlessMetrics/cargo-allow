use super::{
    AddBroadRequest, allow_entry_broad, count_in_scope_findings, ensure_addable_outcome,
    next_allow_id,
};
use allow_core::{
    AllowConfig, AllowEntry, CargoAllowErrorKind, Finding, FindingKind, Lifecycle, MatchStatus,
    Selector, Span, StructuralIdentity,
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
    assert!(ensure_addable_outcome(MatchStatus::New).is_ok());
}

#[test]
fn ensure_addable_outcome_rejects_matched_findings_with_exact_error() {
    let error = ensure_addable_outcome(MatchStatus::Matched)
        .expect_err("matched findings should be rejected");
    assert_eq!(error.kind(), CargoAllowErrorKind::Usage);
}

#[test]
fn ensure_addable_outcome_rejects_stale_findings_with_exact_error() {
    let error =
        ensure_addable_outcome(MatchStatus::Stale).expect_err("stale findings should be rejected");
    assert_eq!(error.kind(), CargoAllowErrorKind::Usage);
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

fn panic_unwrap_finding(path: &str, line: u32) -> Finding {
    let mut identity = StructuralIdentity::new(path, "method_call");
    identity.callee = Some("unwrap".to_string());
    Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from(path),
        span: Some(Span { line, column: 10 }),
        identity,
        message: "panic.unwrap".to_string(),
        ledger: None,
    }
}

#[test]
fn allow_entry_broad_builds_a_no_snippet_hash_selector() {
    // #2056: a broad baseline selector must NOT carry a normalized_snippet_hash
    // (it covers every in-scope occurrence), and must scope to the glob.
    let entry = allow_entry_broad(AddBroadRequest {
        id: "allow-broad".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        callee: Some("unwrap".to_string()),
        glob: "src/foo.rs".to_string(),
        owner: "core".to_string(),
        classification: "baseline_debt".to_string(),
        reason: "baseline".to_string(),
        evidence: Vec::new(),
        review_after: "2026-12-01".to_string(),
        expires: Some("2027-12-31".to_string()),
    });

    assert_eq!(entry.id, "allow-broad");
    assert_eq!(entry.kind, FindingKind::Panic);
    assert_eq!(entry.family.as_deref(), Some("unwrap"));
    assert_eq!(entry.glob.as_deref(), Some("src/foo.rs"));
    assert_eq!(entry.selector.callee.as_deref(), Some("unwrap"));
    assert_eq!(entry.selector.glob.as_deref(), Some("src/foo.rs"));
    assert!(
        entry.selector.normalized_snippet_hash.is_none(),
        "broad selector must not pin a single snippet"
    );
    assert!(
        entry.selector.has_structural_identity(),
        "callee counts as structural identity so Panic matches"
    );
    assert_eq!(entry.occurrence_limit, None, "cmd_add pins the count");
}

#[test]
fn count_in_scope_findings_counts_every_matching_occurrence() {
    // Two unwraps in the scoped file both match a broad callee=unwrap selector.
    let entry = allow_entry_broad(AddBroadRequest {
        id: "allow-broad".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        callee: Some("unwrap".to_string()),
        glob: "src/foo.rs".to_string(),
        owner: "core".to_string(),
        classification: "baseline_debt".to_string(),
        reason: "baseline".to_string(),
        evidence: Vec::new(),
        review_after: "2026-12-01".to_string(),
        expires: Some("2027-12-31".to_string()),
    });
    let findings = vec![
        panic_unwrap_finding("src/foo.rs", 1),
        panic_unwrap_finding("src/foo.rs", 2),
    ];

    assert_eq!(count_in_scope_findings(&findings, &entry), 2);
}

#[test]
fn count_in_scope_findings_excludes_out_of_scope_and_non_matching() {
    let entry = allow_entry_broad(AddBroadRequest {
        id: "allow-broad".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        callee: Some("unwrap".to_string()),
        glob: "src/foo.rs".to_string(),
        owner: "core".to_string(),
        classification: "baseline_debt".to_string(),
        reason: "baseline".to_string(),
        evidence: Vec::new(),
        review_after: "2026-12-01".to_string(),
        expires: Some("2027-12-31".to_string()),
    });
    // One in-scope unwrap, one in a different file (out of scope), one expect
    // in-scope (different callee -> does not match).
    let mut expect_finding = panic_unwrap_finding("src/foo.rs", 5);
    expect_finding.identity.callee = Some("expect".to_string());
    let findings = vec![
        panic_unwrap_finding("src/foo.rs", 1),
        panic_unwrap_finding("src/other.rs", 2),
        expect_finding,
    ];

    assert_eq!(
        count_in_scope_findings(&findings, &entry),
        1,
        "only the in-scope unwrap matches"
    );
}
