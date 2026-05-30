use super::*;
use allow_core::{
    AllowConfig, AllowEntry, Finding, FindingKind, LastSeen, Lifecycle, MatchStatus, Selector,
    Span, StructuralIdentity,
};
use std::path::PathBuf;

mod lint;
mod mode;
mod scoring;

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
