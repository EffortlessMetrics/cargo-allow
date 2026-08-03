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
            .map(|outcome| outcome.score > 0)
            .unwrap_or(false)
    );
    // Ambiguous candidates must NOT be demoted to Stale (#2042 fix).
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| outcome.status == MatchStatus::Stale)
            .count(),
        0,
        "ambiguous candidates should not be demoted to Stale"
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
fn detailed_evaluation_exposes_occurrence_headroom_and_excess() -> Result<(), String> {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.id = "allow-counted".to_string();
    entry.occurrence_limit = Some(2);
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    let evaluation = evaluate_detailed(
        &cfg,
        &[finding.clone(), finding.clone(), finding],
        CheckMode::NoNew,
    );

    assert_eq!(evaluation.occurrence_accounting.len(), 1);
    let accounting = evaluation
        .occurrence_accounting
        .first()
        .ok_or_else(|| "limited entry should have accounting".to_string())?;
    assert_eq!(accounting.allow_id, "allow-counted");
    assert_eq!(accounting.observed_count, 3);
    assert_eq!(accounting.occurrence_limit, 2);
    assert_eq!(accounting.headroom, 0);
    assert_eq!(accounting.exceeded_count, 1);
    assert!(evaluation.outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::New && outcome.message.contains("occurrence_limit exceeded")
    }));
    Ok(())
}

#[test]
fn detailed_evaluation_reports_headroom_without_rederivation() -> Result<(), String> {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.id = "allow-headroom".to_string();
    entry.occurrence_limit = Some(3);
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    let evaluation = evaluate_detailed(&cfg, &[finding], CheckMode::NoNew);
    let accounting = evaluation
        .occurrence_accounting
        .first()
        .ok_or_else(|| "limited entry should have accounting".to_string())?;

    assert_eq!(accounting.observed_count, 1);
    assert_eq!(accounting.headroom, 2);
    assert_eq!(accounting.exceeded_count, 0);

    let empty_evaluation = evaluate_detailed(&cfg, &[], CheckMode::Audit);
    let empty_accounting = empty_evaluation
        .occurrence_accounting
        .first()
        .ok_or_else(|| "limited entry should report zero-use accounting".to_string())?;
    assert_eq!(empty_accounting.observed_count, 0);
    assert_eq!(empty_accounting.headroom, 3);
    assert_eq!(empty_accounting.exceeded_count, 0);
    Ok(())
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
fn anchored_last_seen_suppresses_drift_for_other_occurrences() {
    // Regression: a multi-occurrence entry (glob or repeated snippet) records
    // one last_seen anchor. The other occurrences must not report perpetual
    // location_drift that refresh can never settle.
    let anchored = finding_with_hash("fnv1a64:actual");
    let mut other = finding_with_hash("fnv1a64:actual");
    other.span = Some(Span { line: 9, column: 3 });
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.last_seen = Some(LastSeen {
        line: 50,
        column: 12,
    });
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    // Anchor discovered after the drifting occurrence: order must not matter.
    let outcomes = evaluate(&cfg, &[other, anchored], CheckMode::NoNew);

    assert_eq!(outcomes.len(), 2);
    assert!(
        outcomes
            .iter()
            .all(|outcome| outcome.status == MatchStatus::Matched),
        "anchored entry must not report drift for its other occurrences: {outcomes:?}"
    );
    assert!(outcomes.iter().any(|outcome| {
        outcome
            .message
            .contains("last_seen anchored by another occurrence")
    }));
}

#[test]
fn drift_is_still_reported_when_no_occurrence_anchors_last_seen() {
    let mut first = finding_with_hash("fnv1a64:actual");
    first.span = Some(Span { line: 9, column: 3 });
    let mut second = finding_with_hash("fnv1a64:actual");
    second.span = Some(Span {
        line: 20,
        column: 5,
    });
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.last_seen = Some(LastSeen {
        line: 50,
        column: 12,
    });
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    let outcomes = evaluate(&cfg, &[first, second], CheckMode::NoNew);

    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| outcome.status == MatchStatus::LocationDrift)
            .count(),
        2,
        "unanchored entry must keep reporting drift: {outcomes:?}"
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

    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::Expired
            && outcome.finding_index == Some(0)
            && outcome.message.contains("expired on 2020-01-01")
    }));
    assert!(outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::Stale
            && outcome.finding_index.is_none()
            && outcome.message.contains("allow-1 is stale")
    }));
}

#[test]
fn live_broad_entry_covers_finding_when_precise_entry_is_expired() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut precise = entry_with_hash("fnv1a64:actual");
    precise.id = "allow-precise-expired".to_string();
    precise.lifecycle.expires = Some("2020-01-01".to_string());

    let mut broad = precise.clone();
    broad.id = "allow-broad-live".to_string();
    broad.selector.normalized_snippet_hash = None;
    broad.lifecycle.expires = Some("2999-12-31".to_string());

    let mut cfg = AllowConfig::empty();
    cfg.allow.push(precise);
    cfg.allow.push(broad);

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    assert!(outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::Matched
            && outcome.allow_id.as_deref() == Some("allow-broad-live")
            && outcome.finding_index == Some(0)
    }));
    assert!(!outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::Expired && outcome.finding_index == Some(0)
    }));
    assert!(outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::Stale
            && outcome.allow_id.as_deref() == Some("allow-precise-expired")
            && outcome.finding_index.is_none()
            && outcome.message.contains("expired on 2020-01-01")
    }));
}

#[test]
fn occurrence_limited_expired_entry_is_not_replaced_by_live_fallback() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut precise = entry_with_hash("fnv1a64:actual");
    precise.id = "allow-precise-limited".to_string();
    precise.lifecycle.expires = Some("2020-01-01".to_string());
    precise.occurrence_limit = Some(0);

    let mut broad = precise.clone();
    broad.id = "allow-broad-live".to_string();
    broad.selector.normalized_snippet_hash = None;
    broad.lifecycle.expires = Some("2999-12-31".to_string());
    broad.occurrence_limit = None;

    let mut cfg = AllowConfig::empty();
    cfg.allow.push(precise);
    cfg.allow.push(broad);

    let evaluation = evaluate_detailed(&cfg, &[finding], CheckMode::NoNew);

    assert!(evaluation.outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::New
            && outcome.allow_id.as_deref() == Some("allow-precise-limited")
            && outcome.message.contains("occurrence_limit exceeded")
    }));
    assert!(!evaluation.outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::Matched
            && outcome.allow_id.as_deref() == Some("allow-broad-live")
            && outcome.finding_index == Some(0)
    }));
    assert!(evaluation.outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::Stale
            && outcome.allow_id.as_deref() == Some("allow-broad-live")
            && outcome.finding_index.is_none()
    }));
}

#[test]
fn live_broad_entry_covers_finding_when_precise_entry_lacks_evidence() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut precise = entry_with_hash("fnv1a64:actual");
    precise.id = "allow-precise-unproven".to_string();
    precise.evidence.clear();

    let mut broad = precise.clone();
    broad.id = "allow-broad-live".to_string();
    broad.selector.normalized_snippet_hash = None;
    broad.evidence.push("test:broad-fallback".to_string());

    let mut cfg = AllowConfig::empty();
    cfg.requirements.unsafe_evidence_required = true;
    cfg.allow.push(precise);
    cfg.allow.push(broad);

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    assert!(outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::Matched
            && outcome.allow_id.as_deref() == Some("allow-broad-live")
            && outcome.finding_index == Some(0)
    }));
    assert!(!outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::EvidenceMissing && outcome.finding_index == Some(0)
    }));
    assert!(outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::Stale
            && outcome.allow_id.as_deref() == Some("allow-precise-unproven")
            && outcome.finding_index.is_none()
            && outcome.message.contains("has no evidence")
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

#[test]
fn ambiguity_tiebreak_picks_unique_top_scorer() {
    // Two entries match the same finding. Entry A has more selector fields
    // (higher score). Entry B has fewer (lower score). The unique top scorer
    // (A) should be taken as the match, NOT Ambiguous (#1802).
    let mut finding = finding_with_hash("fnv1a64:actual");
    finding.identity.callee = Some("load".to_string());

    // Entry A: full selector identity (ast_kind + container + hash)
    let mut entry_a = entry_with_hash("fnv1a64:actual");
    entry_a.id = "allow-high-score".to_string();
    entry_a.selector.callee = Some("load".to_string());

    // Entry B: no occurrence hash, so it is only Structural (lower than the
    // ExactOccurrence entry above).
    let mut entry_b = entry_with_hash("fnv1a64:actual");
    entry_b.id = "allow-low-score".to_string();
    entry_b.selector.normalized_snippet_hash = None;

    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry_a);
    cfg.allow.push(entry_b);

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    // The finding should be Matched (not Ambiguous) — the unique top scorer wins.
    assert!(
        outcomes.iter().any(|o| {
            o.status == MatchStatus::Matched && o.allow_id.as_deref() == Some("allow-high-score")
        }),
        "unique top scorer should be taken as the match, not Ambiguous"
    );
    // No Ambiguous outcome should be produced.
    assert!(
        !outcomes.iter().any(|o| o.status == MatchStatus::Ambiguous),
        "no Ambiguous when there's a unique top scorer"
    );
}

#[test]
fn genuine_tie_reports_ambiguous_without_demoting_to_stale() -> Result<(), String> {
    // Two entries with identical scores match the same finding. This is a
    // genuine tie → Ambiguous. But neither entry should be demoted to Stale
    // (they DID match — they're just ambiguous, not stale) (#2042).
    let finding = finding_with_hash("fnv1a64:actual");

    let mut entry_a = entry_with_hash("fnv1a64:actual");
    entry_a.id = "allow-tied-a".to_string();

    let mut entry_b = entry_with_hash("fnv1a64:actual");
    entry_b.id = "allow-tied-b".to_string();

    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry_a);
    cfg.allow.push(entry_b);

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

    // The finding should be Ambiguous (genuine tie).
    assert!(
        outcomes.iter().any(|o| o.status == MatchStatus::Ambiguous),
        "genuine score tie should produce Ambiguous"
    );
    let ambiguous = outcomes
        .iter()
        .find(|outcome| outcome.status == MatchStatus::Ambiguous)
        .ok_or_else(|| "ambiguous outcome should be present".to_string())?;
    assert_eq!(
        ambiguous.candidate_ids,
        vec!["allow-tied-a".to_string(), "allow-tied-b".to_string()]
    );
    assert_eq!(ambiguous.allow_id, None);
    // Neither entry should be Stale (they matched, they're just ambiguous).
    assert!(
        !outcomes.iter().any(|o| {
            o.status == MatchStatus::Stale
                && (o.allow_id.as_deref() == Some("allow-tied-a")
                    || o.allow_id.as_deref() == Some("allow-tied-b"))
        }),
        "ambiguous candidates must not be demoted to Stale"
    );
    Ok(())
}
