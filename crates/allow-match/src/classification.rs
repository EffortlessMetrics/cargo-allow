use allow_core::{AllowConfig, AllowEntry, Finding, FindingKind, MatchStatus, SimpleDate};

use crate::CheckMode;
use crate::lifecycle::{entry_is_expired, entry_review_is_due, malformed_lifecycle_reason};
use crate::location_drift::last_seen_drift_message;
use crate::scoring::MatchStrength;

pub(crate) fn classify_matched(
    entry: &AllowEntry,
    finding: &Finding,
    strength: MatchStrength,
    today: SimpleDate,
    cfg: &AllowConfig,
    mode: CheckMode,
) -> (MatchStatus, String) {
    let score = strength.as_priority();
    // Malformed lifecycle dates fail closed before any authority decision
    // (#1804): an unparseable expires/review_after must never become silent
    // advisory posture or silent authorization in any mode (#4238).
    if let Some(reason) = malformed_lifecycle_reason(entry) {
        return (MatchStatus::MissingRequiredField, reason);
    }
    // Authorized survival (#4236/#4238): calendar cadence is not candidate
    // authority for matches that carry an exact accepted identity or a
    // bounded denominator. Exact-occurrence and structural matches pin the
    // accepted finding identity; an occurrence_limit bounds how many
    // occurrences the entry can absorb. Only a broad scoped-family match
    // (kind + family + path/glob, no identity fields, no bound) loses
    // authority at expiry — its renewal must be forced, not silently
    // absorbed.
    let authorized_survival = match strength {
        MatchStrength::ExactOccurrence | MatchStrength::Structural => true,
        MatchStrength::ScopedFamily => entry.occurrence_limit.is_some(),
    };
    // Mandatory requirement health precedes lifecycle annotations (#4644):
    // these checks used to run only after the `Expired`/`ReviewDue` returns
    // below, so a due review or a passed expiry masked a missing mandatory
    // SAFETY comment, a missing or mismatched `policy:<allow-id>` reference,
    // or missing mandatory entry evidence. Both annotated statuses are
    // advisory in no-new (#4238), so `check --mode no-new` false-greened
    // exactly when cadence work was most overdue — the opposite direction of
    // the original calendar incident. Requirement failures stay blocking
    // before, on, and after both dates; an otherwise-valid match still
    // receives its lifecycle annotation below, and the read-model projection
    // keeps finding-level hard failures authoritative
    // (`allow_report::ledger_outcome_status`).
    if cfg.requirements.unsafe_evidence_required
        && entry.kind == FindingKind::Unsafe
        && entry.evidence.is_empty()
    {
        return (
            MatchStatus::EvidenceMissing,
            format!("{} matched unsafe finding but has no evidence", entry.id),
        );
    }
    if cfg.requirements.unsafe_safety_comment_required
        && entry.kind == FindingKind::Unsafe
        && finding.identity.target_fingerprint.as_deref() != Some("safety-comment:present")
    {
        return (
            MatchStatus::EvidenceMissing,
            format!(
                "{} matched unsafe finding but has no nearby SAFETY comment",
                entry.id
            ),
        );
    }
    if entry.kind == FindingKind::LintException
        && !cfg.requirements.allow_bare_allow_attributes
        && finding.family.as_deref() == Some("allow_attribute")
    {
        return (
            MatchStatus::InvalidSelector,
            format!(
                "{} matched bare allow attribute while allow_bare_allow_attributes=false",
                entry.id
            ),
        );
    }
    if entry.kind == FindingKind::LintException {
        let policy_id = finding
            .identity
            .target_fingerprint
            .as_deref()
            .and_then(|value| value.strip_prefix("policy:"));
        if cfg.requirements.lint_policy_id_required && policy_id.is_none() {
            return (
                MatchStatus::InvalidSelector,
                format!(
                    "{} matched lint suppression without required policy:<allow-id> reference",
                    entry.id
                ),
            );
        }
        if let Some(policy_id) = policy_id
            && policy_id != entry.id
        {
            return (
                MatchStatus::InvalidSelector,
                format!(
                    "{} matched lint suppression that references policy:{policy_id}",
                    entry.id
                ),
            );
        }
    }
    if entry_is_expired(entry, today)
        && let Some(expires) = &entry.lifecycle.expires
    {
        if !authorized_survival {
            return (
                MatchStatus::New,
                format!(
                    "{} matched but expired on {expires}; broad-matcher authority ends at expiry — new finding",
                    entry.id
                ),
            );
        }
        return (
            MatchStatus::Expired,
            format!("{} matched but expired on {expires}", entry.id),
        );
    }
    if entry_review_is_due(entry, today)
        && let Some(review_after) = &entry.lifecycle.review_after
    {
        return (
            MatchStatus::ReviewDue,
            format!(
                "{} matched but review is due after {review_after}",
                entry.id
            ),
        );
    }
    if entry.classification == "baseline_debt"
        && matches!(mode, CheckMode::Strict | CheckMode::Release)
    {
        let primary_message = format!(
            "{} is baseline debt and cannot pass {} mode",
            entry.id,
            mode.as_str()
        );
        if let Some(drift_message) = last_seen_drift_message(entry, finding) {
            return (
                MatchStatus::BaselineDebt,
                format!("{primary_message}; {drift_message}"),
            );
        }
        return (MatchStatus::BaselineDebt, primary_message);
    }
    if let Some(message) = last_seen_drift_message(entry, finding) {
        return (MatchStatus::LocationDrift, message);
    }
    (
        MatchStatus::Matched,
        format!("{} matched with structural score {score}", entry.id),
    )
}

#[cfg(test)]
mod tests {
    use super::classify_matched;
    use crate::{CheckMode, scoring::MatchStrength};
    use allow_core::{
        AllowConfig, AllowEntry, Finding, FindingKind, LastSeen, Lifecycle, MatchStatus, Selector,
        SimpleDate, Span, StructuralIdentity,
    };
    use std::path::PathBuf;

    fn today() -> SimpleDate {
        SimpleDate {
            year: 2026,
            month: 6,
            day: 14,
        }
    }

    fn entry(kind: FindingKind) -> AllowEntry {
        AllowEntry {
            id: "allow-1".to_string(),
            kind,
            family: Some("unsafe_fn".to_string()),
            path: Some(PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "core".to_string(),
            classification: "reviewed_exception".to_string(),
            reason: "fixture".to_string(),
            evidence: vec!["test:fixture".to_string()],
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle {
                created: None,
                review_after: None,
                expires: Some("2026-12-31".to_string()),
            },
            selector: Selector::default(),
            last_seen: None,
        }
    }

    fn test_finding(kind: FindingKind) -> Finding {
        Finding {
            kind,
            family: Some("unsafe_fn".to_string()),
            path: PathBuf::from("src/lib.rs"),
            span: Some(Span {
                line: 50,
                column: 12,
            }),
            identity: StructuralIdentity::new("rust", "unsafe_fn"),
            message: String::new(),
            ledger: None,
        }
    }

    #[test]
    fn classify_matched_reports_review_due_before_other_posture_checks() {
        let cfg = AllowConfig::empty();
        let finding = test_finding(FindingKind::Panic);
        let mut review_due = entry(FindingKind::Panic);
        review_due.lifecycle.review_after = Some("2020-01-01".to_string());
        let (status, message) = classify_matched(
            &review_due,
            &finding,
            MatchStrength::ScopedFamily,
            today(),
            &cfg,
            CheckMode::NoNew,
        );
        assert_eq!(status, MatchStatus::ReviewDue);
        assert!(message.contains("review is due after 2020-01-01"));
    }

    #[test]
    fn classify_matched_reports_expiry_and_unsafe_evidence_requirements() {
        let cfg = AllowConfig::empty();
        let finding = test_finding(FindingKind::Unsafe);
        let mut expired = entry(FindingKind::Unsafe);
        expired.lifecycle.expires = Some("2020-01-01".to_string());
        // Expired broad matcher (no structural identity, no bound) loses
        // authority at expiry (#4238): the finding is re-raised as new.
        let (status, message) = classify_matched(
            &expired,
            &finding,
            MatchStrength::ScopedFamily,
            today(),
            &cfg,
            CheckMode::NoNew,
        );
        assert_eq!(status, MatchStatus::New);
        assert!(message.contains("expired on 2020-01-01"));
        assert!(message.contains("broad-matcher authority ends at expiry"));

        // The same expired entry keeps authorized survival when the match
        // carries a structural identity: status stays Expired for visibility.
        let (status, message) = classify_matched(
            &expired,
            &finding,
            MatchStrength::Structural,
            today(),
            &cfg,
            CheckMode::NoNew,
        );
        assert_eq!(status, MatchStatus::Expired);
        assert!(message.contains("expired on 2020-01-01"));

        // A bounded broad matcher (occurrence_limit) also survives expiry.
        let mut expired_bounded = entry(FindingKind::Unsafe);
        expired_bounded.lifecycle.expires = Some("2020-01-01".to_string());
        expired_bounded.occurrence_limit = Some(1);
        let (status, message) = classify_matched(
            &expired_bounded,
            &finding,
            MatchStrength::ScopedFamily,
            today(),
            &cfg,
            CheckMode::NoNew,
        );
        assert_eq!(status, MatchStatus::Expired);
        assert!(message.contains("expired on 2020-01-01"));

        let mut missing_evidence = entry(FindingKind::Unsafe);
        missing_evidence.evidence.clear();
        let (status, message) = classify_matched(
            &missing_evidence,
            &finding,
            MatchStrength::ScopedFamily,
            today(),
            &cfg,
            CheckMode::NoNew,
        );
        assert_eq!(status, MatchStatus::EvidenceMissing);
        assert!(message.contains("has no evidence"));

        let mut safety_cfg = AllowConfig::empty();
        safety_cfg.requirements.unsafe_safety_comment_required = true;
        let (status, message) = classify_matched(
            &entry(FindingKind::Unsafe),
            &finding,
            MatchStrength::ScopedFamily,
            today(),
            &safety_cfg,
            CheckMode::NoNew,
        );
        assert_eq!(status, MatchStatus::EvidenceMissing);
        assert!(message.contains("no nearby SAFETY comment"));

        let mut safe_finding = test_finding(FindingKind::Unsafe);
        safe_finding.identity.target_fingerprint = Some("safety-comment:present".to_string());
        let (status, message) = classify_matched(
            &entry(FindingKind::Unsafe),
            &safe_finding,
            MatchStrength::ScopedFamily,
            today(),
            &safety_cfg,
            CheckMode::NoNew,
        );
        assert_eq!(status, MatchStatus::Matched);
        assert!(message.contains("matched with structural score 100"));
    }

    #[test]
    fn classify_matched_reports_lint_policy_requirement_failures() {
        let mut cfg = AllowConfig::empty();
        cfg.requirements.allow_bare_allow_attributes = false;
        cfg.requirements.lint_policy_id_required = true;
        let entry = entry(FindingKind::LintException);
        let mut finding = test_finding(FindingKind::LintException);
        finding.family = Some("allow_attribute".to_string());

        let (status, message) = classify_matched(
            &entry,
            &finding,
            MatchStrength::ScopedFamily,
            today(),
            &cfg,
            CheckMode::NoNew,
        );
        assert_eq!(status, MatchStatus::InvalidSelector);
        assert!(message.contains("allow_bare_allow_attributes=false"));

        finding.family = Some("expect_attribute".to_string());
        let (status, message) = classify_matched(
            &entry,
            &finding,
            MatchStrength::ScopedFamily,
            today(),
            &cfg,
            CheckMode::NoNew,
        );
        assert_eq!(status, MatchStatus::InvalidSelector);
        assert!(message.contains("without required policy:<allow-id> reference"));

        finding.identity.target_fingerprint = Some("policy:allow-other".to_string());
        let (status, message) = classify_matched(
            &entry,
            &finding,
            MatchStrength::ScopedFamily,
            today(),
            &cfg,
            CheckMode::NoNew,
        );
        assert_eq!(status, MatchStatus::InvalidSelector);
        assert!(message.contains("policy:allow-other"));

        finding.identity.target_fingerprint = Some("policy:allow-1".to_string());
        let (status, message) = classify_matched(
            &entry,
            &finding,
            MatchStrength::ScopedFamily,
            today(),
            &cfg,
            CheckMode::NoNew,
        );
        assert_eq!(status, MatchStatus::Matched);
        assert!(message.contains("matched with structural score 100"));
    }

    #[test]
    fn classify_matched_preserves_mandatory_requirements_across_lifecycle_dates() {
        // #4644 regression matrix: mandatory requirement health must not
        // depend on calendar position. Identical inputs are evaluated at the
        // issue's probe dates — 2026-10-09 (before review_after), 2026-10-10
        // (review due, inclusive), 2026-10-20 (expiry day, not yet passed),
        // and 2026-10-21 (expires passed) — for entries carrying
        // review_after = 2026-10-10 and expires = 2026-10-20. A requirement
        // failure must block at every date; an otherwise-valid match must
        // keep receiving its lifecycle annotation (Matched → ReviewDue →
        // Expired) so cadence work stays visible.
        let review_after = "2026-10-10";
        let expires = "2026-10-20";
        let dates = [
            (
                SimpleDate {
                    year: 2026,
                    month: 10,
                    day: 9,
                },
                "before both dates",
            ),
            (
                SimpleDate {
                    year: 2026,
                    month: 10,
                    day: 10,
                },
                "review due",
            ),
            (
                SimpleDate {
                    year: 2026,
                    month: 10,
                    day: 20,
                },
                "expiry day",
            ),
            (
                SimpleDate {
                    year: 2026,
                    month: 10,
                    day: 21,
                },
                "expired",
            ),
        ];

        let mut safety_cfg = AllowConfig::empty();
        safety_cfg.requirements.unsafe_safety_comment_required = true;
        let mut unsafe_entry = entry(FindingKind::Unsafe);
        unsafe_entry.lifecycle.review_after = Some(review_after.to_string());
        unsafe_entry.lifecycle.expires = Some(expires.to_string());
        assert_eq!(unsafe_entry.evidence, vec!["test:fixture".to_string()]);
        let mut unsafe_finding = test_finding(FindingKind::Unsafe);
        unsafe_finding.identity.target_fingerprint = Some("safety-comment:present".to_string());
        let unsafe_finding_missing_safety = test_finding(FindingKind::Unsafe);

        let mut lint_cfg = AllowConfig::empty();
        lint_cfg.requirements.lint_policy_id_required = true;
        let mut lint_entry = entry(FindingKind::LintException);
        lint_entry.lifecycle.review_after = Some(review_after.to_string());
        lint_entry.lifecycle.expires = Some(expires.to_string());
        let mut lint_finding_self = test_finding(FindingKind::LintException);
        lint_finding_self.family = Some("expect_attribute".to_string());
        lint_finding_self.identity.target_fingerprint = Some("policy:allow-1".to_string());
        let mut lint_finding_missing_policy = test_finding(FindingKind::LintException);
        lint_finding_missing_policy.family = Some("expect_attribute".to_string());
        let mut lint_finding_other_policy = test_finding(FindingKind::LintException);
        lint_finding_other_policy.family = Some("expect_attribute".to_string());
        lint_finding_other_policy.identity.target_fingerprint =
            Some("policy:allow-other".to_string());

        for (as_of, phase) in dates {
            // Required SAFETY metadata missing stays blocking at every date.
            let (status, message) = classify_matched(
                &unsafe_entry,
                &unsafe_finding_missing_safety,
                MatchStrength::Structural,
                as_of,
                &safety_cfg,
                CheckMode::NoNew,
            );
            assert_eq!(status, MatchStatus::EvidenceMissing, "{phase}: {message}");
            assert!(
                message.contains("no nearby SAFETY comment"),
                "{phase}: {message}"
            );
            assert!(CheckMode::NoNew.fails(status), "{phase}");

            // Required lint policy ID absent stays blocking at every date.
            let (status, message) = classify_matched(
                &lint_entry,
                &lint_finding_missing_policy,
                MatchStrength::Structural,
                as_of,
                &lint_cfg,
                CheckMode::NoNew,
            );
            assert_eq!(status, MatchStatus::InvalidSelector, "{phase}: {message}");
            assert!(
                message.contains("without required policy:<allow-id> reference"),
                "{phase}: {message}"
            );
            assert!(CheckMode::NoNew.fails(status), "{phase}");

            // A policy ID naming another entry stays blocking at every date.
            let (status, message) = classify_matched(
                &lint_entry,
                &lint_finding_other_policy,
                MatchStrength::Structural,
                as_of,
                &lint_cfg,
                CheckMode::NoNew,
            );
            assert_eq!(status, MatchStatus::InvalidSelector, "{phase}: {message}");
            assert!(message.contains("policy:allow-other"), "{phase}: {message}");
            assert!(CheckMode::NoNew.fails(status), "{phase}");

            // Otherwise-valid matches keep their lifecycle annotation so the
            // cadence surface stays visible: annotated before both dates,
            // review due on/after review_after, expired after expires.
            let (status, message) = classify_matched(
                &unsafe_entry,
                &unsafe_finding,
                MatchStrength::Structural,
                as_of,
                &safety_cfg,
                CheckMode::NoNew,
            );
            let expected = match as_of.day {
                9 => MatchStatus::Matched,
                21 => MatchStatus::Expired,
                _ => MatchStatus::ReviewDue,
            };
            assert_eq!(status, expected, "{phase}: {message}");
            assert!(!CheckMode::NoNew.fails(status), "{phase}: {message}");

            let (status, message) = classify_matched(
                &lint_entry,
                &lint_finding_self,
                MatchStrength::Structural,
                as_of,
                &lint_cfg,
                CheckMode::NoNew,
            );
            assert_eq!(status, expected, "{phase}: {message}");
            assert!(!CheckMode::NoNew.fails(status), "{phase}: {message}");
        }
    }

    #[test]
    fn classify_matched_reports_location_drift_as_advisory_signal() {
        let cfg = AllowConfig::empty();
        let finding = test_finding(FindingKind::Panic);
        let mut drift_entry = entry(FindingKind::Panic);
        drift_entry.last_seen = Some(LastSeen {
            line: 7,
            column: 12,
        });
        let (status, message) = classify_matched(
            &drift_entry,
            &finding,
            MatchStrength::ScopedFamily,
            today(),
            &cfg,
            CheckMode::NoNew,
        );
        assert_eq!(status, MatchStatus::LocationDrift);
        assert!(message.contains("last_seen changed from 7:12 to 50:12"));
        assert!(!CheckMode::NoNew.fails(status));
    }

    #[test]
    fn classify_matched_retains_location_drift_when_baseline_debt_blocks_release() {
        let cfg = AllowConfig::empty();
        let finding = test_finding(FindingKind::Panic);
        let mut baseline = entry(FindingKind::Panic);
        baseline.classification = "baseline_debt".to_string();
        baseline.last_seen = Some(LastSeen {
            line: 7,
            column: 12,
        });

        let (status, message) = classify_matched(
            &baseline,
            &finding,
            MatchStrength::ScopedFamily,
            today(),
            &cfg,
            CheckMode::Release,
        );

        assert_eq!(status, MatchStatus::BaselineDebt);
        assert!(message.contains("cannot pass release mode"));
        assert!(message.contains("last_seen changed from 7:12 to 50:12"));
    }

    #[test]
    fn classify_matched_reports_release_baseline_debt_and_regular_match() {
        let cfg = AllowConfig::empty();
        let finding = test_finding(FindingKind::Panic);
        let mut baseline = entry(FindingKind::Panic);
        baseline.classification = "baseline_debt".to_string();
        let (status, message) = classify_matched(
            &baseline,
            &finding,
            MatchStrength::ScopedFamily,
            today(),
            &cfg,
            CheckMode::Release,
        );
        assert_eq!(status, MatchStatus::BaselineDebt);
        assert!(message.contains("cannot pass release mode"));

        let mut never = entry(FindingKind::Panic);
        never.lifecycle.expires = Some("never".to_string());
        let (status, message) = classify_matched(
            &never,
            &finding,
            MatchStrength::ScopedFamily,
            today(),
            &cfg,
            CheckMode::NoNew,
        );
        assert_eq!(status, MatchStatus::Matched);
        assert!(message.contains("matched with structural score 100"));
    }

    #[test]
    fn classify_matched_fails_closed_on_malformed_lifecycle_dates() {
        // #1804 fail-safe must remain a blocking failure under the #4238
        // candidate-mode law: an unparseable date must never collapse into
        // the now-advisory Expired annotation or into silent authority.
        let cfg = AllowConfig::empty();
        let finding = test_finding(FindingKind::Panic);
        for (field, value) in [
            ("expires", "2026-13-40"),
            ("expires", "not-a-date"),
            ("review_after", "not-a-date"),
        ] {
            let mut malformed = entry(FindingKind::Panic);
            if field == "expires" {
                malformed.lifecycle.expires = Some(value.to_string());
            } else {
                malformed.lifecycle.review_after = Some(value.to_string());
            }
            for strength in [
                MatchStrength::ScopedFamily,
                MatchStrength::Structural,
                MatchStrength::ExactOccurrence,
            ] {
                let (status, message) = classify_matched(
                    &malformed,
                    &finding,
                    strength,
                    today(),
                    &cfg,
                    CheckMode::NoNew,
                );
                assert_eq!(status, MatchStatus::MissingRequiredField, "{field}={value}");
                assert!(
                    message.contains("malformed lifecycle date"),
                    "{field}={value}: {message}"
                );
                assert!(message.contains("failing closed"), "{message}");
                assert!(CheckMode::NoNew.fails(status));
                assert!(CheckMode::Strict.fails(status));
            }
        }

        // `expires = "never"` stays the documented immortal form.
        let mut immortal = entry(FindingKind::Panic);
        immortal.lifecycle.expires = Some("never".to_string());
        let (status, _) = classify_matched(
            &immortal,
            &finding,
            MatchStrength::ScopedFamily,
            today(),
            &cfg,
            CheckMode::NoNew,
        );
        assert_eq!(status, MatchStatus::Matched);
    }
}
