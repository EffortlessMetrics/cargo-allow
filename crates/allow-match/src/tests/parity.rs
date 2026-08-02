//! Semantic parity between the two selected-candidate evaluation shapes (#2336).
//!
//! For the same selected entry and finding, adding a weaker matching candidate
//! may add candidate context, but it must not change lifecycle classification,
//! whether the entry is live/used, whether live occurrence headroom is
//! consumed, whether a non-live stale projection is emitted, or blocking
//! posture. These tests run each lifecycle case twice — once as the only
//! candidate, once with a strictly weaker neighboring candidate present — and
//! assert the selected entry's observable outcome is identical. They compare
//! statuses and accounting, not message strings.

use super::*;
use crate::OccurrenceAccounting;

const STRONG_ID: &str = "allow-strong";
const NEIGHBOR_ID: &str = "allow-weaker-neighbor";

/// Build a strictly weaker neighboring candidate for `strong`.
///
/// It matches the same finding surface but drops the `normalized_snippet_hash`
/// anchor, demoting it from `ExactOccurrence` to `Structural` — a lower
/// `MatchStrength`, so the strong entry stays the unique top scorer and the
/// neighbor never wins the tiebreak. Its own lifecycle is kept live and
/// unremarkable so it cannot contribute a confounding projection; every
/// assertion below filters to the strong entry's id.
fn weaker_neighbor(strong: &AllowEntry) -> AllowEntry {
    let mut neighbor = strong.clone();
    neighbor.id = NEIGHBOR_ID.to_string();
    neighbor.selector.normalized_snippet_hash = None;
    neighbor.occurrence_limit = None;
    neighbor.lifecycle.expires = Some("2999-12-31".to_string());
    neighbor.lifecycle.review_after = None;
    neighbor.classification = "reviewed_exception".to_string();
    neighbor
}

fn non_live_weaker_neighbor(strong: &AllowEntry) -> AllowEntry {
    let mut neighbor = weaker_neighbor(strong);
    neighbor.lifecycle.expires = Some("2020-01-01".to_string());
    neighbor
}

/// The strong entry's observable outcome, projected out of a full evaluation so
/// the two call shapes can be compared directly.
#[derive(Debug, PartialEq, Eq)]
struct StrongView {
    /// Status of every finding-level outcome attributed to the strong entry,
    /// in order. Captures lifecycle classification and (via the same statuses
    /// under a fixed mode) blocking posture.
    winner_statuses: Vec<MatchStatus>,
    /// Status of the strong entry's unused-entry projection, if any. `Some`
    /// means a non-live stale/expired/review projection was emitted; `None`
    /// means the entry was consumed as live.
    projection_status: Option<MatchStatus>,
    /// Occurrence accounting for the strong entry, if it declares a limit.
    accounting: Option<OccurrenceAccounting>,
}

fn strong_view(evaluation: &crate::MatchEvaluation) -> StrongView {
    let winner_statuses = evaluation
        .outcomes
        .iter()
        .filter(|outcome| {
            outcome.finding_index.is_some() && outcome.allow_id.as_deref() == Some(STRONG_ID)
        })
        .map(|outcome| outcome.status)
        .collect();
    let projection_status = evaluation
        .outcomes
        .iter()
        .find(|outcome| {
            outcome.finding_index.is_none() && outcome.allow_id.as_deref() == Some(STRONG_ID)
        })
        .map(|outcome| outcome.status);
    let accounting = evaluation
        .occurrence_accounting
        .iter()
        .find(|accounting| accounting.allow_id == STRONG_ID)
        .cloned();
    StrongView {
        winner_statuses,
        projection_status,
        accounting,
    }
}

/// Assert that evaluating `strong` as the only candidate produces the same
/// selected-entry outcome as evaluating it alongside a strictly weaker
/// neighbor. Returns the shared `StrongView` for any case-specific assertions.
fn assert_parity(
    label: &str,
    strong: AllowEntry,
    findings: &[Finding],
    cfg_requirements: impl Fn(&mut AllowConfig),
    mode: CheckMode,
) -> StrongView {
    let mut single = AllowConfig::empty();
    cfg_requirements(&mut single);
    single.allow.push(strong.clone());

    let mut paired = AllowConfig::empty();
    cfg_requirements(&mut paired);
    paired.allow.push(strong.clone());
    paired.allow.push(weaker_neighbor(&strong));

    let single_view = strong_view(&evaluate_detailed(&single, findings, mode));
    let paired_view = strong_view(&evaluate_detailed(&paired, findings, mode));

    assert_eq!(
        single_view, paired_view,
        "{label}: a weaker neighboring candidate changed the selected entry's outcome"
    );
    single_view
}

fn strong_entry() -> AllowEntry {
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.id = STRONG_ID.to_string();
    entry
}

fn strong_finding() -> Finding {
    finding_with_hash("fnv1a64:actual")
}

#[test]
fn matched_case_has_parity() {
    let view = assert_parity(
        "matched",
        strong_entry(),
        &[strong_finding()],
        |_| {},
        CheckMode::NoNew,
    );
    assert_eq!(view.winner_statuses, vec![MatchStatus::Matched]);
    assert_eq!(view.projection_status, None);
}

#[test]
fn location_drift_case_has_parity() {
    let mut entry = strong_entry();
    entry.last_seen = Some(LastSeen { line: 7, column: 3 });
    let view = assert_parity(
        "location_drift",
        entry,
        &[strong_finding()],
        |_| {},
        CheckMode::NoNew,
    );
    assert_eq!(view.winner_statuses, vec![MatchStatus::LocationDrift]);
    // LocationDrift is live: it consumes the entry, so no stale projection.
    assert_eq!(view.projection_status, None);
}

#[test]
fn review_due_case_has_parity() {
    let today = allow_core::SimpleDate::today_utc_approx();
    let past = today.add_days(-5).to_string();
    let mut entry = strong_entry();
    entry.lifecycle.review_after = Some(past);
    let view = assert_parity(
        "review_due",
        entry,
        &[strong_finding()],
        |_| {},
        CheckMode::NoNew,
    );
    assert_eq!(view.winner_statuses, vec![MatchStatus::ReviewDue]);
    // ReviewDue is live: no stale projection.
    assert_eq!(view.projection_status, None);
}

#[test]
fn baseline_debt_strict_and_release_have_parity() {
    for mode in [CheckMode::Strict, CheckMode::Release] {
        let mut entry = strong_entry();
        entry.classification = "baseline_debt".to_string();
        let view = assert_parity("baseline_debt", entry, &[strong_finding()], |_| {}, mode);
        assert_eq!(view.winner_statuses, vec![MatchStatus::BaselineDebt]);
        assert_eq!(view.projection_status, None);
    }
}

#[test]
fn expired_case_has_parity_and_stays_non_live() {
    let mut entry = strong_entry();
    entry.lifecycle.expires = Some("2020-01-01".to_string());
    let mut single = AllowConfig::empty();
    single.allow.push(entry.clone());
    let mut paired = AllowConfig::empty();
    paired.allow.push(entry.clone());
    paired.allow.push(non_live_weaker_neighbor(&entry));

    let single_view = strong_view(&evaluate_detailed(
        &single,
        &[strong_finding()],
        CheckMode::NoNew,
    ));
    let paired_view = strong_view(&evaluate_detailed(
        &paired,
        &[strong_finding()],
        CheckMode::NoNew,
    ));
    assert_eq!(single_view, paired_view);
    let view = single_view;
    assert_eq!(view.winner_statuses, vec![MatchStatus::Expired]);
    // Expired is non-live: it must still emit the stale projection even with a
    // weaker neighbor present (the core #2336 regression).
    assert_eq!(view.projection_status, Some(MatchStatus::Stale));
}

#[test]
fn evidence_missing_case_has_parity_and_stays_non_live() {
    let mut entry = strong_entry();
    entry.evidence.clear();
    let view = assert_parity(
        "evidence_missing",
        entry,
        &[strong_finding()],
        |cfg| cfg.requirements.unsafe_evidence_required = true,
        CheckMode::NoNew,
    );
    assert_eq!(view.winner_statuses, vec![MatchStatus::EvidenceMissing]);
    assert_eq!(view.projection_status, Some(MatchStatus::Stale));
}

#[test]
fn invalid_selector_case_has_parity_and_stays_non_live() {
    // Lint suppression missing the required policy:<allow-id> reference.
    let mut entry = lint_entry(STRONG_ID);
    entry.selector.normalized_snippet_hash = Some("fnv1a64:lint".to_string());
    let mut finding = lint_finding("expect_attribute");
    finding.identity.normalized_snippet_hash = Some("fnv1a64:lint".to_string());

    let view = assert_parity(
        "invalid_selector",
        entry,
        &[finding],
        |cfg| cfg.requirements.lint_policy_id_required = true,
        CheckMode::NoNew,
    );
    assert_eq!(view.winner_statuses, vec![MatchStatus::InvalidSelector]);
    assert_eq!(view.projection_status, Some(MatchStatus::Stale));
}

#[test]
fn occurrence_limit_available_has_parity() -> Result<(), String> {
    let mut entry = strong_entry();
    entry.occurrence_limit = Some(2);
    let view = assert_parity(
        "occurrence_available",
        entry,
        &[strong_finding()],
        |_| {},
        CheckMode::NoNew,
    );
    assert_eq!(view.winner_statuses, vec![MatchStatus::Matched]);
    let accounting = view
        .accounting
        .ok_or_else(|| "limited entry should have accounting".to_string())?;
    assert_eq!(accounting.observed_count, 1);
    assert_eq!(accounting.headroom, 1);
    assert_eq!(accounting.exceeded_count, 0);
    Ok(())
}

#[test]
fn occurrence_limit_exceeded_has_parity() -> Result<(), String> {
    let mut entry = strong_entry();
    entry.occurrence_limit = Some(1);
    let finding = strong_finding();
    let view = assert_parity(
        "occurrence_exceeded",
        entry,
        &[finding.clone(), finding],
        |_| {},
        CheckMode::NoNew,
    );
    assert_eq!(
        view.winner_statuses,
        vec![MatchStatus::Matched, MatchStatus::New]
    );
    let accounting = view
        .accounting
        .ok_or_else(|| "limited entry should have accounting".to_string())?;
    assert_eq!(accounting.observed_count, 2);
    assert_eq!(accounting.exceeded_count, 1);
    Ok(())
}

#[test]
fn expired_unique_strongest_does_not_consume_live_occurrence_headroom() {
    // The named minimum acceptance case (#2336): an expired unique-strongest
    // candidate with a limit of 1, hit by two findings, must classify both as
    // Expired and must NOT spend a live occurrence slot on the first (which
    // would spuriously trip occurrence_limit-exceeded on the second). This is
    // exactly the divergence the old duplicated branch introduced.
    let mut entry = strong_entry();
    entry.lifecycle.expires = Some("2020-01-01".to_string());
    entry.occurrence_limit = Some(1);
    let finding = strong_finding();
    let mut single = AllowConfig::empty();
    single.allow.push(entry.clone());
    let mut paired = AllowConfig::empty();
    paired.allow.push(entry.clone());
    paired.allow.push(non_live_weaker_neighbor(&entry));

    let single_view = strong_view(&evaluate_detailed(
        &single,
        &[finding.clone(), finding.clone()],
        CheckMode::NoNew,
    ));
    let paired_view = strong_view(&evaluate_detailed(
        &paired,
        &[finding.clone(), finding],
        CheckMode::NoNew,
    ));
    assert_eq!(single_view, paired_view);
    let view = single_view;
    // Both findings classify as Expired; neither becomes an occurrence-exceeded
    // New outcome, because a non-live status consumes no live headroom.
    assert_eq!(
        view.winner_statuses,
        vec![MatchStatus::Expired, MatchStatus::Expired]
    );
    assert_eq!(view.projection_status, Some(MatchStatus::Stale));
    assert!(
        !view.winner_statuses.contains(&MatchStatus::New),
        "expired unique-strongest candidate must not trip occurrence_limit exceeded"
    );
}

#[test]
fn evidence_missing_unique_strongest_does_not_consume_live_occurrence_headroom() {
    // Same headroom-parity property as the expired case, for EvidenceMissing.
    // EvidenceMissing shares the non-live `status_consumes_entry` branch today,
    // but a status that ever leaked into live consumption only through the
    // many-candidate path is exactly the #2336 failure mode, so pin it.
    let mut entry = strong_entry();
    entry.evidence.clear();
    entry.occurrence_limit = Some(1);
    let finding = strong_finding();
    let view = assert_parity(
        "evidence_missing_limited_two_findings",
        entry,
        &[finding.clone(), finding],
        |cfg| cfg.requirements.unsafe_evidence_required = true,
        CheckMode::NoNew,
    );
    assert_eq!(
        view.winner_statuses,
        vec![MatchStatus::EvidenceMissing, MatchStatus::EvidenceMissing]
    );
    assert_eq!(view.projection_status, Some(MatchStatus::Stale));
    assert!(!view.winner_statuses.contains(&MatchStatus::New));
}

#[test]
fn invalid_selector_unique_strongest_does_not_consume_live_occurrence_headroom() {
    // Same headroom-parity property for InvalidSelector (lint suppression
    // missing the required policy reference), through the unique-strongest path.
    let mut entry = lint_entry(STRONG_ID);
    entry.selector.normalized_snippet_hash = Some("fnv1a64:lint".to_string());
    entry.occurrence_limit = Some(1);
    let mut finding = lint_finding("expect_attribute");
    finding.identity.normalized_snippet_hash = Some("fnv1a64:lint".to_string());
    let view = assert_parity(
        "invalid_selector_limited_two_findings",
        entry,
        &[finding.clone(), finding],
        |cfg| cfg.requirements.lint_policy_id_required = true,
        CheckMode::NoNew,
    );
    assert_eq!(
        view.winner_statuses,
        vec![MatchStatus::InvalidSelector, MatchStatus::InvalidSelector]
    );
    assert_eq!(view.projection_status, Some(MatchStatus::Stale));
    assert!(!view.winner_statuses.contains(&MatchStatus::New));
}

#[test]
fn anchored_drift_suppression_has_parity() {
    // Two occurrences of the strong entry: one drifts from `last_seen`, one
    // sits exactly on it (anchoring). The anchor must retroactively promote the
    // drifting occurrence from LocationDrift back to Matched — and that
    // bookkeeping (drift_outcomes / anchored_entries) must run identically
    // whether the entry is the only candidate or the unique strongest among
    // many. The pre-#2336 many-branch never populated this bookkeeping at all,
    // so this fixture locks the previously-divergent path into parity.
    let mut entry = strong_entry();
    entry.last_seen = Some(LastSeen {
        line: 50,
        column: 12,
    });

    let mut drift = strong_finding();
    drift.span = Some(Span { line: 9, column: 3 });
    let anchor = strong_finding(); // span defaults to 50:12, exactly last_seen

    let view = assert_parity(
        "anchored_drift_suppression",
        entry,
        &[drift, anchor],
        |_| {},
        CheckMode::NoNew,
    );
    // The anchor promotes the drifting occurrence back to Matched, so both
    // occurrences read Matched and the entry stays live (no stale projection).
    assert_eq!(
        view.winner_statuses,
        vec![MatchStatus::Matched, MatchStatus::Matched]
    );
    assert_eq!(view.projection_status, None);
}
