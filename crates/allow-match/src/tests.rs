use super::*;
use allow_core::{
    AllowConfig, AllowEntry, Finding, FindingKind, LastSeen, Lifecycle, MatchStatus, Selector,
    Span, StructuralIdentity,
};
use std::path::PathBuf;

mod lint;

#[test]
fn matches_moved_line_by_structure() {
    let mut id = StructuralIdentity::new("rust", "method_call");
    id.container = Some("load".to_string());
    id.callee = Some("unwrap".to_string());
    let finding = Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: Some(Span {
            line: 50,
            column: 12,
        }),
        identity: id,
        message: String::new(),
    };
    let entry = AllowEntry {
        id: "allow-1".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "test".to_string(),
        reason: "reason".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: None,
            review_after: None,
            expires: Some("2026-12-31".to_string()),
        },
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            container: Some("load".to_string()),
            callee: Some("unwrap".to_string()),
            line_hint: Some(12),
            ..Selector::default()
        },
        last_seen: None,
    };
    assert!(score_match(&entry, &finding).unwrap() >= 80);
}

#[test]
fn snippet_hash_selector_rejects_different_source() {
    let finding = finding_with_hash("fnv1a64:actual");
    let entry = entry_with_hash("fnv1a64:expected");

    assert_eq!(score_match(&entry, &finding), None);
}

#[test]
fn snippet_hash_selector_accepts_same_source() {
    let finding = finding_with_hash("fnv1a64:actual");
    let entry = entry_with_hash("fnv1a64:actual");

    assert!(score_match(&entry, &finding).is_some());
}

#[test]
fn structural_field_mismatch_rejects_match() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.selector.container = Some("other_container".to_string());

    assert_eq!(score_match(&entry, &finding), None);
}

#[test]
fn score_match_accepts_selector_glob_when_entry_path_is_absent() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.path = None;
    entry.glob = None;
    entry.selector.glob = Some("src/**/*.rs".to_string());

    assert!(score_match(&entry, &finding).is_some());
}

#[test]
fn top_level_glob_matches_when_entry_path_is_absent() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.path = None;
    entry.glob = Some("src/**/*.rs".to_string());

    assert!(score_match(&entry, &finding).is_some());
}

#[test]
fn receiver_fingerprint_substring_match_scores_less_than_exact_match() {
    let mut finding = finding_with_hash("fnv1a64:actual");
    finding.identity.receiver_fingerprint = Some("workspace.config.requirements".to_string());
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.selector.receiver_fingerprint = Some("config".to_string());

    let substring_score = score_match(&entry, &finding)
        .unwrap_or_else(|| std::panic::panic_any("expected receiver substring match"));

    entry.selector.receiver_fingerprint = Some("workspace.config.requirements".to_string());
    let exact_score = score_match(&entry, &finding)
        .unwrap_or_else(|| std::panic::panic_any("expected exact receiver match"));

    assert_eq!(exact_score - substring_score, 15);
}

#[test]
fn target_fingerprint_selector_accepts_structural_substrings() {
    let mut finding = finding_with_hash("fnv1a64:actual");
    finding.identity.target_fingerprint = Some("safety-comment:present".to_string());
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.selector.target_fingerprint = Some("comment:present".to_string());

    assert!(score_match(&entry, &finding).is_some());

    entry.selector.target_fingerprint = Some("comment:missing".to_string());
    assert_eq!(score_match(&entry, &finding), None);
}

#[test]
fn score_match_rejects_when_no_path_or_glob_matches() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.path = None;
    entry.glob = Some("tests/**/*.rs".to_string());
    entry.selector.glob = Some("examples/**/*.rs".to_string());

    assert_eq!(score_match(&entry, &finding), None);
}

#[test]
fn check_mode_parse_defaults_unknown_values_to_no_new() {
    assert_eq!(CheckMode::parse("audit"), CheckMode::Audit);
    assert_eq!(CheckMode::parse("strict"), CheckMode::Strict);
    assert_eq!(CheckMode::parse("release"), CheckMode::Release);
    assert_eq!(CheckMode::parse("no-new"), CheckMode::NoNew);
    assert_eq!(CheckMode::parse("unexpected"), CheckMode::NoNew);
}

#[test]
fn check_mode_failure_policy_matches_enforcement_levels() {
    assert!(!CheckMode::Audit.fails(MatchStatus::New));
    assert!(CheckMode::NoNew.fails(MatchStatus::New));
    assert!(!CheckMode::NoNew.fails(MatchStatus::Stale));
    assert!(CheckMode::NoNew.fails(MatchStatus::Expired));
    assert!(CheckMode::Strict.fails(MatchStatus::Stale));
    assert!(CheckMode::Release.fails(MatchStatus::BaselineDebt));
    assert!(!CheckMode::Strict.fails(MatchStatus::Matched));
    assert!(!CheckMode::Release.fails(MatchStatus::ReviewDue));
}

#[test]
fn score_match_scores_exact_receiver_above_partial_receiver() {
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.selector.receiver_fingerprint = Some("value".to_string());

    let mut exact = finding_with_hash("fnv1a64:actual");
    exact.identity.receiver_fingerprint = Some("value".to_string());
    let mut partial = finding_with_hash("fnv1a64:actual");
    partial.identity.receiver_fingerprint = Some("context.value".to_string());

    let exact_score = score_match(&entry, &exact)
        .unwrap_or_else(|| std::panic::panic_any("exact receiver fingerprint should match"));
    let partial_score = score_match(&entry, &partial)
        .unwrap_or_else(|| std::panic::panic_any("partial receiver fingerprint should match"));

    assert!(exact_score > partial_score);
}

#[test]
fn score_match_uses_last_seen_line_when_selector_line_hint_is_absent() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.selector.line_hint = None;

    let without_last_seen = score_match(&entry, &finding)
        .unwrap_or_else(|| std::panic::panic_any("entry should match without last_seen"));
    entry.last_seen = Some(LastSeen {
        line: 50,
        column: 12,
    });
    let with_last_seen = score_match(&entry, &finding)
        .unwrap_or_else(|| std::panic::panic_any("entry should match with last_seen"));

    assert_eq!(with_last_seen, without_last_seen + 15);
}

#[test]
fn unsafe_safety_comment_requirement_fails_without_metadata() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut cfg = AllowConfig::empty();
    cfg.requirements.unsafe_safety_comment_required = true;
    cfg.allow.push(entry_with_hash("fnv1a64:actual"));

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    assert!(outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::EvidenceMissing
            && outcome.message.contains("no nearby SAFETY comment")
    }));
}

#[test]
fn unsafe_safety_comment_requirement_passes_with_metadata() {
    let mut finding = finding_with_hash("fnv1a64:actual");
    finding.identity.target_fingerprint = Some("safety-comment:present".to_string());
    let mut cfg = AllowConfig::empty();
    cfg.requirements.unsafe_safety_comment_required = true;
    cfg.allow.push(entry_with_hash("fnv1a64:actual"));

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    assert!(
        outcomes
            .iter()
            .any(|outcome| outcome.status == MatchStatus::Matched)
    );
}

#[test]
fn evaluate_fails_closed_on_ambiguous_structural_matches() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut first = entry_with_hash("fnv1a64:actual");
    first.id = "allow-1".to_string();
    let mut second = entry_with_hash("fnv1a64:actual");
    second.id = "allow-2".to_string();
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(first);
    cfg.allow.push(second);

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    assert!(
        outcomes
            .iter()
            .any(|outcome| outcome.status == MatchStatus::Ambiguous)
    );
    assert!(
        outcomes
            .iter()
            .find(|outcome| outcome.status == MatchStatus::Ambiguous)
            .map(|outcome| outcome.score >= STRUCTURAL_MATCH_THRESHOLD)
            .unwrap_or(false)
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| outcome.status == MatchStatus::Stale)
            .count(),
        2
    );
}

#[test]
fn entry_glob_matches_finding_path_when_exact_path_is_absent() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.path = None;
    entry.glob = Some("src/**/*.rs".to_string());

    assert!(score_match(&entry, &finding).is_some());
}

#[test]
fn selector_glob_matches_finding_path_when_entry_path_is_absent() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.path = None;
    entry.selector.glob = Some("src/*.rs".to_string());

    assert!(score_match(&entry, &finding).is_some());
}

#[test]
fn source_code_scope_only_selector_does_not_match() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.selector = Selector {
        glob: Some("src/lib.rs".to_string()),
        ..Selector::default()
    };

    assert_eq!(score_match(&entry, &finding), None);
}

#[test]
fn receiver_fingerprint_scores_exact_and_partial_matches() {
    let mut finding = finding_with_hash("fnv1a64:actual");
    finding.identity.receiver_fingerprint = Some("config.loader.result".to_string());
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.selector.receiver_fingerprint = Some("config.loader.result".to_string());
    let exact = score_match(&entry, &finding)
        .unwrap_or_else(|| std::panic::panic_any("exact receiver should match"));

    entry.selector.receiver_fingerprint = Some("loader".to_string());
    let partial = score_match(&entry, &finding)
        .unwrap_or_else(|| std::panic::panic_any("partial receiver should match"));

    entry.selector.receiver_fingerprint = Some("other".to_string());
    assert_eq!(score_match(&entry, &finding), None);
    assert!(exact > partial);
}

#[test]
fn target_and_symbol_selectors_require_substring_matches() {
    let mut finding = finding_with_hash("fnv1a64:actual");
    finding.identity.symbol =
        Some(r#"#[expect(clippy::unwrap_used, reason = "policy:allow-1")]"#.to_string());
    finding.identity.target_fingerprint = Some("policy:allow-1".to_string());
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.selector.symbol = Some("clippy::unwrap_used".to_string());
    entry.selector.target_fingerprint = Some("allow-1".to_string());

    assert!(score_match(&entry, &finding).is_some());

    entry.selector.symbol = Some("clippy::panic".to_string());
    assert_eq!(score_match(&entry, &finding), None);

    entry.selector.symbol = Some("clippy::unwrap_used".to_string());
    entry.selector.target_fingerprint = Some("allow-other".to_string());
    assert_eq!(score_match(&entry, &finding), None);
}

#[test]
fn last_seen_line_hint_contributes_when_selector_hint_is_absent() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    let base = score_match(&entry, &finding)
        .unwrap_or_else(|| std::panic::panic_any("baseline match should score"));

    entry.last_seen = Some(allow_core::LastSeen {
        line: 50,
        column: 12,
    });
    let with_last_seen = score_match(&entry, &finding)
        .unwrap_or_else(|| std::panic::panic_any("last seen match should score"));

    assert_eq!(with_last_seen, base + 15);
}

#[test]
fn occurrence_limit_caps_matched_findings() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.occurrence_limit = Some(1);
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    let outcomes = evaluate(&cfg, &[finding.clone(), finding], CheckMode::NoNew);

    assert_eq!(outcomes.len(), 2);
    assert!(matches!(
        outcomes.first().map(|outcome| outcome.status),
        Some(MatchStatus::Matched)
    ));
    assert!(outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::New && outcome.message.contains("occurrence_limit exceeded")
    }));
}

#[test]
fn unlimited_entry_matches_repeated_findings() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry_with_hash("fnv1a64:actual"));

    let outcomes = evaluate(&cfg, &[finding.clone(), finding], CheckMode::NoNew);

    assert_eq!(outcomes.len(), 2);
    assert!(
        outcomes
            .iter()
            .all(|outcome| outcome.status == MatchStatus::Matched)
    );
}

#[test]
fn unmatched_finding_is_reported_as_new_with_location() {
    let finding = finding_with_hash("fnv1a64:actual");
    let cfg = AllowConfig::empty();

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    assert_eq!(outcomes.len(), 1);
    let outcome = outcomes
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one new outcome"));
    assert_eq!(outcome.status, MatchStatus::New);
    assert_eq!(outcome.allow_id, None);
    assert_eq!(outcome.finding_index, Some(0));
    assert!(outcome.message.contains("unreceipted unsafe.unsafe_fn"));
    assert!(outcome.message.contains("src/lib.rs:50:12"));
}

#[test]
fn unmatched_allow_entry_is_reported_as_stale_with_scope() {
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry_with_hash("fnv1a64:actual"));

    let outcomes = evaluate(&cfg, &[], CheckMode::NoNew);

    assert_eq!(outcomes.len(), 1);
    let outcome = outcomes
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one stale outcome"));
    assert_eq!(outcome.status, MatchStatus::Stale);
    assert_eq!(outcome.allow_id.as_deref(), Some("allow-1"));
    assert_eq!(outcome.finding_index, None);
    assert!(outcome.message.contains("allow-1 is stale"));
    assert!(outcome.message.contains("src/lib.rs"));
}

#[test]
fn expired_entry_reports_expired_even_when_structure_matches() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.lifecycle.expires = Some("2020-01-01".to_string());
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    assert!(outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::Expired && outcome.message.contains("expired on 2020-01-01")
    }));
}

#[test]
fn never_expiring_entry_can_match() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.lifecycle.expires = Some("never".to_string());
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    assert!(
        outcomes
            .iter()
            .any(|outcome| outcome.status == MatchStatus::Matched)
    );
}

#[test]
fn unsafe_evidence_requirement_fails_without_entry_evidence() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.evidence.clear();
    let mut cfg = AllowConfig::empty();
    cfg.requirements.unsafe_evidence_required = true;
    cfg.allow.push(entry);

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    assert!(outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::EvidenceMissing
            && outcome.message.contains("has no evidence")
    }));
}

#[test]
fn baseline_debt_fails_only_in_release_mode() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.classification = "baseline_debt".to_string();
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    let no_new = evaluate(&cfg, std::slice::from_ref(&finding), CheckMode::NoNew);
    let release = evaluate(&cfg, &[finding], CheckMode::Release);

    assert!(
        no_new
            .iter()
            .any(|outcome| outcome.status == MatchStatus::Matched)
    );
    assert!(release.iter().any(|outcome| {
        outcome.status == MatchStatus::BaselineDebt
            && outcome.message.contains("cannot pass release mode")
    }));
}

#[test]
fn scoring_accepts_entry_glob_and_selector_glob() {
    let mut finding = finding_with_hash("fnv1a64:actual");
    finding.path = PathBuf::from("crates/parser/src/lib.rs");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.path = None;
    entry.glob = Some("crates/*/src/*.rs".to_string());
    assert!(score_match(&entry, &finding).is_some());

    entry.glob = None;
    entry.selector.glob = Some("crates/**/lib.rs".to_string());
    assert!(score_match(&entry, &finding).is_some());
}

#[test]
fn scoring_rejects_kind_family_and_path_mismatches() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.kind = FindingKind::Panic;
    assert_eq!(score_match(&entry, &finding), None);

    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.family = Some("unsafe_block".to_string());
    assert_eq!(score_match(&entry, &finding), None);

    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.path = Some(PathBuf::from("src/other.rs"));
    assert_eq!(score_match(&entry, &finding), None);
}

#[test]
fn scoring_weights_exact_and_partial_fingerprints() {
    let mut finding = finding_with_hash("fnv1a64:actual");
    finding.identity.receiver_fingerprint = Some("parser.load_result".to_string());
    finding.identity.target_fingerprint = Some("safety-comment:present".to_string());
    let mut exact = entry_with_hash("fnv1a64:actual");
    exact.selector.receiver_fingerprint = Some("parser.load_result".to_string());
    exact.selector.target_fingerprint = Some("safety-comment".to_string());
    let exact_score = score_match(&exact, &finding).unwrap_or_default();

    let mut partial = exact.clone();
    partial.selector.receiver_fingerprint = Some("load_result".to_string());
    let partial_score = score_match(&partial, &finding).unwrap_or_default();

    let mut mismatch = exact;
    mismatch.selector.receiver_fingerprint = Some("other_result".to_string());

    assert!(exact_score > partial_score);
    assert!(partial_score >= STRUCTURAL_MATCH_THRESHOLD);
    assert_eq!(score_match(&mismatch, &finding), None);
}

#[test]
fn check_mode_parsing_and_failure_policy_are_covered() {
    assert_eq!(CheckMode::parse("audit"), CheckMode::Audit);
    assert_eq!(CheckMode::parse("strict"), CheckMode::Strict);
    assert_eq!(CheckMode::parse("release"), CheckMode::Release);
    assert_eq!(CheckMode::parse("unknown"), CheckMode::NoNew);

    assert!(!CheckMode::Audit.fails(MatchStatus::New));
    assert!(CheckMode::NoNew.fails(MatchStatus::New));
    assert!(CheckMode::Strict.fails(MatchStatus::Stale));
    assert!(CheckMode::Release.fails(MatchStatus::BaselineDebt));
    assert!(!CheckMode::Release.fails(MatchStatus::Matched));
}

fn entry_with_hash(hash: &str) -> AllowEntry {
    AllowEntry {
        id: "allow-1".to_string(),
        kind: FindingKind::Unsafe,
        family: Some("unsafe_fn".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "test".to_string(),
        reason: "reason".to_string(),
        evidence: vec!["unsafe-review".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: None,
            review_after: None,
            expires: Some("2026-12-31".to_string()),
        },
        selector: Selector {
            ast_kind: Some("unsafe_fn".to_string()),
            container: Some("scan_line".to_string()),
            normalized_snippet_hash: Some(hash.to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn finding_with_hash(hash: &str) -> Finding {
    let mut id = StructuralIdentity::new("rust", "unsafe_fn");
    id.container = Some("scan_line".to_string());
    id.normalized_snippet_hash = Some(hash.to_string());
    Finding {
        kind: FindingKind::Unsafe,
        family: Some("unsafe_fn".to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: Some(Span {
            line: 50,
            column: 12,
        }),
        identity: id,
        message: String::new(),
    }
}

fn lint_entry(id: &str) -> AllowEntry {
    lint_entry_with_family(id, "expect_attribute")
}

fn lint_entry_with_family(id: &str, family: &str) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::LintException,
        family: Some(family.to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Lint suppression is linked to policy.".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: None,
            review_after: None,
            expires: Some("2026-12-31".to_string()),
        },
        selector: Selector {
            ast_kind: Some("attribute".to_string()),
            lint: Some("clippy::unwrap_used".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn lint_finding_with_policy(policy_id: &str) -> Finding {
    let mut finding = lint_finding("expect_attribute");
    finding.identity.target_fingerprint = Some(format!("policy:{policy_id}"));
    finding
}

fn lint_finding(family: &str) -> Finding {
    let mut id = StructuralIdentity::new("rust", "attribute");
    id.lint = Some("clippy::unwrap_used".to_string());
    Finding {
        kind: FindingKind::LintException,
        family: Some(family.to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: Some(Span {
            line: 10,
            column: 1,
        }),
        identity: id,
        message: String::new(),
    }
}
