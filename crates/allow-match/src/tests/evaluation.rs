use super::*;

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
fn evaluate_counts_review_due_for_matched_and_unused_entries() {
    let today = allow_core::SimpleDate::today_utc_approx();
    let past = today.add_days(-5).to_string();
    let finding = finding_with_hash("fnv1a64:actual");
    let mut matched = entry_with_hash("fnv1a64:actual");
    matched.lifecycle.review_after = Some(past.clone());
    let mut unused = entry_with_hash("fnv1a64:unused");
    unused.id = "allow-unused".to_string();
    unused.lifecycle.review_after = Some(past);
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(matched);
    cfg.allow.push(unused);

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| outcome.status == MatchStatus::ReviewDue)
            .count(),
        2
    );
    assert!(!CheckMode::NoNew.fails(MatchStatus::ReviewDue));
    assert!(CheckMode::Strict.fails(MatchStatus::ReviewDue));
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
fn baseline_debt_fails_in_strict_and_release_mode() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.classification = "baseline_debt".to_string();
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    let no_new = evaluate(&cfg, std::slice::from_ref(&finding), CheckMode::NoNew);
    let strict = evaluate(&cfg, std::slice::from_ref(&finding), CheckMode::Strict);
    let release = evaluate(&cfg, &[finding], CheckMode::Release);

    // no-new: baseline debt is allowed (debt exists, just not new debt)
    assert!(
        no_new
            .iter()
            .any(|outcome| outcome.status == MatchStatus::Matched)
    );
    // strict: baseline debt must fail (strict is at least as restrictive as release)
    assert!(strict.iter().any(|outcome| {
        outcome.status == MatchStatus::BaselineDebt
            && outcome.message.contains("cannot pass strict mode")
    }));
    // release: baseline debt must fail
    assert!(release.iter().any(|outcome| {
        outcome.status == MatchStatus::BaselineDebt
            && outcome.message.contains("cannot pass release mode")
    }));
}

#[test]
fn unparseable_expires_date_is_treated_as_expired_fail_safe() {
    // Regression for #1804: an unparseable expires date must NOT silently
    // make the entry immortal. Fail-safe: treat as expired.
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.lifecycle.expires = Some("2026-13-40".to_string()); // invalid month/day
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    assert!(
        outcomes
            .iter()
            .any(|outcome| outcome.status == MatchStatus::Expired),
        "unparseable expires must be treated as expired (fail-safe), not immortal"
    );
}

#[test]
fn unparseable_review_after_is_treated_as_due_fail_safe() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.lifecycle.review_after = Some("not-a-date".to_string());
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    assert!(
        outcomes
            .iter()
            .any(|outcome| outcome.status == MatchStatus::ReviewDue),
        "unparseable review_after must be treated as review-due (fail-safe)"
    );
}
