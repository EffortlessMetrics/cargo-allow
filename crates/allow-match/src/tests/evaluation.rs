use super::*;

#[test]
fn unsafe_safety_comment_requirement_fails_without_metadata() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut cfg = AllowConfig::empty();
    cfg.requirements.unsafe_safety_comment_required = true;
    cfg.allow.push(entry_with_hash("fnv1a64:actual"));

    let outcomes = evaluate(
        &cfg,
        &[finding],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

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

    let outcomes = evaluate(
        &cfg,
        &[finding],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

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

    let outcomes = evaluate(
        &cfg,
        &[finding],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

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

    let outcomes = evaluate(
        &cfg,
        &[finding.clone(), finding],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

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
        allow_core::SimpleDate::today_utc_approx(),
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

    let evaluation = evaluate_detailed(
        &cfg,
        &[finding],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );
    let accounting = evaluation
        .occurrence_accounting
        .first()
        .ok_or_else(|| "limited entry should have accounting".to_string())?;

    assert_eq!(accounting.observed_count, 1);
    assert_eq!(accounting.headroom, 2);
    assert_eq!(accounting.exceeded_count, 0);

    let empty_evaluation = evaluate_detailed(
        &cfg,
        &[],
        CheckMode::Audit,
        allow_core::SimpleDate::today_utc_approx(),
    );
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

    let outcomes = evaluate(
        &cfg,
        &[finding.clone(), finding],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

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
    let outcomes = evaluate(
        &cfg,
        &[other, anchored],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

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

    let outcomes = evaluate(
        &cfg,
        &[first, second],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

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

    let outcomes = evaluate(
        &cfg,
        &[finding],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

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

    let outcomes = evaluate(
        &cfg,
        &[],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

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

    let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew, today);

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
fn explicit_as_of_controls_lifecycle_classification() {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.lifecycle.review_after = Some("2026-09-18".to_string());
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    let before = allow_core::SimpleDate {
        year: 2026,
        month: 9,
        day: 17,
    };
    let due = allow_core::SimpleDate {
        year: 2026,
        month: 9,
        day: 18,
    };
    let before_outcomes = evaluate(
        &cfg,
        std::slice::from_ref(&finding),
        CheckMode::NoNew,
        before,
    );
    let due_outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew, due);

    assert!(
        before_outcomes
            .iter()
            .any(|outcome| outcome.status == MatchStatus::Matched)
    );
    assert!(
        due_outcomes
            .iter()
            .any(|outcome| outcome.status == MatchStatus::ReviewDue)
    );
}

#[test]
fn expired_exact_entry_authorizes_directly_and_stays_visible() {
    // #4238 candidate-mode law: an expired exact match keeps authority, the
    // outcome stays `Expired` (annotation-only in no-new), and the entry is
    // consumed as live — so no stale "not currently authorizing" projection
    // fires for it anymore.
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.lifecycle.expires = Some("2020-01-01".to_string());
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    let outcomes = evaluate(
        &cfg,
        &[finding],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

    assert_eq!(outcomes.len(), 1);
    assert!(outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::Expired
            && outcome.finding_index == Some(0)
            && outcome.message.contains("expired on 2020-01-01")
    }));
    assert!(!outcomes.iter().any(|outcome| {
        outcome.finding_index.is_none() && outcome.status == MatchStatus::Stale
    }));
}

#[test]
fn expired_broad_entry_loses_authority_and_raises_new_finding() {
    // #4238 law 2: an expired ScopedFamily-only matcher (no structural
    // identity, no occurrence_limit) does not authorize. The finding becomes
    // a blocking `New` naming the expired broad entry, and the entry still
    // lands in the stale projection so it is never silently hidden.
    let mut broad = entry_with_hash("fnv1a64:actual");
    broad.id = "allow-broad-expired".to_string();
    broad.selector.normalized_snippet_hash = None;
    broad.selector.ast_kind = None;
    broad.selector.container = None;
    // GeneratedCode findings carry no structural-identity requirement, so a
    // kind+path-only entry genuinely matches at the ScopedFamily tier.
    broad.kind = FindingKind::GeneratedCode;
    broad.family = None;
    broad.lifecycle.expires = Some("2020-01-01".to_string());
    let mut finding_broad = finding_with_hash("fnv1a64:actual");
    finding_broad.kind = FindingKind::GeneratedCode;
    finding_broad.family = None;
    finding_broad.identity = StructuralIdentity::new("rust", "generated_code");
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(broad);

    let outcomes = evaluate(
        &cfg,
        &[finding_broad],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

    let new_outcome = outcomes
        .iter()
        .find(|outcome| outcome.finding_index == Some(0))
        .unwrap_or_else(|| std::panic::panic_any("expected a finding-level outcome"));
    assert_eq!(new_outcome.status, MatchStatus::New);
    assert!(
        new_outcome
            .message
            .contains("allow-broad-expired matched but expired on 2020-01-01")
    );
    assert!(
        new_outcome
            .message
            .contains("broad-matcher authority ends at expiry")
    );
    // The entry must stay visible in the stale projection (advisory).
    assert!(outcomes.iter().any(|outcome| {
        outcome.finding_index.is_none()
            && outcome.allow_id.as_deref() == Some("allow-broad-expired")
            && outcome
                .message
                .contains("broad-matcher authority ends at expiry")
    }));
}

#[test]
fn expired_exact_entry_authorizes_over_live_broad_neighbor() {
    // #4238 fixture (d): the EXPIRED EXACT entry authorizes directly — the
    // fallback pop must not fire — while the live broad neighbor stays
    // unused and only appears in the advisory stale projection.
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

    let outcomes = evaluate(
        &cfg,
        &[finding],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

    assert!(outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::Expired
            && outcome.allow_id.as_deref() == Some("allow-precise-expired")
            && outcome.finding_index == Some(0)
    }));
    assert!(!outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::Matched && outcome.finding_index == Some(0)
    }));
    assert!(outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::Stale
            && outcome.allow_id.as_deref() == Some("allow-broad-live")
            && outcome.finding_index.is_none()
            && outcome.message.contains("no current finding matched")
    }));
}

#[test]
fn live_broad_entry_covers_finding_when_no_precise_entry_exists() {
    // Negative-control companion to the expired-exact fixture above: with no
    // precise candidate at all, a live broad entry still authorizes the
    // finding. Broad authority is only demoted by expiry, not by existence.
    let finding = finding_with_hash("fnv1a64:actual");
    let mut broad = entry_with_hash("fnv1a64:actual");
    broad.id = "allow-broad-live".to_string();
    broad.selector.normalized_snippet_hash = None;
    broad.lifecycle.expires = Some("2999-12-31".to_string());

    let mut cfg = AllowConfig::empty();
    cfg.allow.push(broad);

    let outcomes = evaluate(
        &cfg,
        &[finding],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

    assert!(outcomes.iter().any(|outcome| {
        outcome.status == MatchStatus::Matched
            && outcome.allow_id.as_deref() == Some("allow-broad-live")
            && outcome.finding_index == Some(0)
    }));
}

#[test]
fn occurrence_limited_expired_entry_is_not_replaced_by_live_fallback() -> Result<(), String> {
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

    let evaluation = evaluate_detailed(
        &cfg,
        &[finding],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

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
    let accounting = evaluation
        .occurrence_accounting
        .iter()
        .find(|accounting| accounting.allow_id == "allow-precise-limited")
        .ok_or_else(|| "occurrence-limited precise entry should have accounting".to_string())?;
    assert_eq!(accounting.observed_count, 1);
    assert_eq!(accounting.occurrence_limit, 0);
    assert_eq!(accounting.headroom, 0);
    assert_eq!(accounting.exceeded_count, 1);
    Ok(())
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

    let outcomes = evaluate(
        &cfg,
        &[finding],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

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

    let outcomes = evaluate(
        &cfg,
        &[finding],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

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

    let outcomes = evaluate(
        &cfg,
        &[finding],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

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

    let no_new = evaluate(
        &cfg,
        std::slice::from_ref(&finding),
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );
    let strict = evaluate(
        &cfg,
        std::slice::from_ref(&finding),
        CheckMode::Strict,
        allow_core::SimpleDate::today_utc_approx(),
    );
    let release = evaluate(
        &cfg,
        &[finding],
        CheckMode::Release,
        allow_core::SimpleDate::today_utc_approx(),
    );

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
fn unparseable_expires_date_fails_closed_in_no_new() {
    // Regression for #1804, updated by #4238: an unparseable expires date
    // must NOT silently make the entry immortal, and since #4238 it must not
    // collapse into the advisory Expired annotation either — it fails the
    // no-new gate with a visible malformed-date reason.
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.lifecycle.expires = Some("2026-13-40".to_string()); // invalid month/day
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    let outcomes = evaluate(
        &cfg,
        &[finding],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

    let outcome = outcomes
        .iter()
        .find(|outcome| outcome.finding_index == Some(0))
        .unwrap_or_else(|| std::panic::panic_any("expected a finding-level outcome"));
    assert_eq!(outcome.status, MatchStatus::MissingRequiredField);
    assert!(outcome.message.contains("malformed lifecycle date"));
    assert!(outcome.message.contains("expires"));
    assert!(outcome.message.contains("failing closed"));
    assert!(CheckMode::NoNew.fails(outcome.status));
    assert!(CheckMode::Strict.fails(outcome.status));
}

#[test]
fn unparseable_review_after_fails_closed_in_no_new() {
    // Same fail-closed law for review_after (#1804 via #4238): never
    // silently advisory, never silently authorizing.
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.lifecycle.review_after = Some("not-a-date".to_string());
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    let outcomes = evaluate(
        &cfg,
        &[finding],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

    let outcome = outcomes
        .iter()
        .find(|outcome| outcome.finding_index == Some(0))
        .unwrap_or_else(|| std::panic::panic_any("expected a finding-level outcome"));
    assert_eq!(outcome.status, MatchStatus::MissingRequiredField);
    assert!(outcome.message.contains("malformed lifecycle date"));
    assert!(outcome.message.contains("review_after"));
    assert!(CheckMode::NoNew.fails(outcome.status));
}

#[test]
fn malformed_lifecycle_date_fails_closed_even_without_matching_findings() {
    // The unused-entry projection also fails closed: a broken lifecycle date
    // is a policy defect whether or not a finding currently matches.
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.lifecycle.expires = Some("garbage".to_string());
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    let outcomes = evaluate(
        &cfg,
        &[],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

    assert!(outcomes.iter().any(|outcome| {
        outcome.finding_index.is_none()
            && outcome.status == MatchStatus::MissingRequiredField
            && outcome.message.contains("malformed lifecycle date")
    }));
    assert!(CheckMode::NoNew.fails(MatchStatus::MissingRequiredField));
}

#[test]
fn lifecycle_cadence_does_not_change_no_new_verdict_for_exact_matches() {
    // #4238 fixture (a): the deterministic candidate-mode invariant. The
    // same evaluator + subject + policy + findings evaluated at explicit
    // as-of dates before, on, and after both a review_after and an expires
    // date must produce the same PASSING no-new verdict for an exact entry,
    // with the row status visibly progressing (Matched -> ReviewDue ->
    // Expired). No system clock anywhere in the loop.
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.lifecycle.review_after = Some("2026-06-20".to_string());
    entry.lifecycle.expires = Some("2026-06-27".to_string());
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    for (as_of, expected_status) in [
        (
            allow_core::SimpleDate {
                year: 2026,
                month: 6,
                day: 1,
            },
            MatchStatus::Matched,
        ),
        (
            allow_core::SimpleDate {
                year: 2026,
                month: 6,
                day: 19,
            },
            MatchStatus::Matched,
        ),
        (
            allow_core::SimpleDate {
                year: 2026,
                month: 6,
                day: 20,
            },
            MatchStatus::ReviewDue,
        ),
        (
            allow_core::SimpleDate {
                year: 2026,
                month: 6,
                day: 26,
            },
            MatchStatus::ReviewDue,
        ),
        (
            allow_core::SimpleDate {
                year: 2026,
                month: 6,
                day: 28,
            },
            MatchStatus::Expired,
        ),
    ] {
        let outcomes = evaluate(
            &cfg,
            std::slice::from_ref(&finding),
            CheckMode::NoNew,
            as_of,
        );
        let finding_outcome = outcomes
            .iter()
            .find(|outcome| outcome.finding_index == Some(0))
            .unwrap_or_else(|| {
                std::panic::panic_any("exact entry must produce a finding-level outcome")
            });
        assert_eq!(
            finding_outcome.status, expected_status,
            "as_of {as_of}: row status should progress with lifecycle dates"
        );
        assert!(
            !CheckMode::NoNew.fails(finding_outcome.status),
            "as_of {as_of}: date passage must not change the no-new verdict"
        );
    }
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

    let outcomes = evaluate(
        &cfg,
        &[finding],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

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

    let outcomes = evaluate(
        &cfg,
        &[finding],
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );

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

#[test]
fn evaluator_source_has_no_hidden_system_clock() {
    let source = include_str!("../evaluation.rs");
    assert!(
        !source.contains("today_utc_approx"),
        "core matcher evaluation must receive its lifecycle date from the caller"
    );
}
