use allow_core::{AllowConfig, AllowEntry, Finding, FindingKind, MatchStatus, SimpleDate};

use crate::CheckMode;
use crate::lifecycle::{entry_is_expired, entry_review_is_due};
use crate::location_drift::last_seen_drift_message;

pub(crate) fn classify_matched(
    entry: &AllowEntry,
    finding: &Finding,
    score: u32,
    today: SimpleDate,
    cfg: &AllowConfig,
    mode: CheckMode,
) -> (MatchStatus, String) {
    if entry_is_expired(entry, today)
        && let Some(expires) = &entry.lifecycle.expires
    {
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
    use crate::CheckMode;
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
        let (status, message) =
            classify_matched(&review_due, &finding, 91, today(), &cfg, CheckMode::NoNew);
        assert_eq!(status, MatchStatus::ReviewDue);
        assert!(message.contains("review is due after 2020-01-01"));
    }

    #[test]
    fn classify_matched_reports_expiry_and_unsafe_evidence_requirements() {
        let cfg = AllowConfig::empty();
        let finding = test_finding(FindingKind::Unsafe);
        let mut expired = entry(FindingKind::Unsafe);
        expired.lifecycle.expires = Some("2020-01-01".to_string());
        let (status, message) =
            classify_matched(&expired, &finding, 98, today(), &cfg, CheckMode::NoNew);
        assert_eq!(status, MatchStatus::Expired);
        assert!(message.contains("expired on 2020-01-01"));

        let mut missing_evidence = entry(FindingKind::Unsafe);
        missing_evidence.evidence.clear();
        let (status, message) = classify_matched(
            &missing_evidence,
            &finding,
            98,
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
            98,
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
            98,
            today(),
            &safety_cfg,
            CheckMode::NoNew,
        );
        assert_eq!(status, MatchStatus::Matched);
        assert!(message.contains("matched with structural score 98"));
    }

    #[test]
    fn classify_matched_reports_lint_policy_requirement_failures() {
        let mut cfg = AllowConfig::empty();
        cfg.requirements.allow_bare_allow_attributes = false;
        cfg.requirements.lint_policy_id_required = true;
        let entry = entry(FindingKind::LintException);
        let mut finding = test_finding(FindingKind::LintException);
        finding.family = Some("allow_attribute".to_string());

        let (status, message) =
            classify_matched(&entry, &finding, 87, today(), &cfg, CheckMode::NoNew);
        assert_eq!(status, MatchStatus::InvalidSelector);
        assert!(message.contains("allow_bare_allow_attributes=false"));

        finding.family = Some("expect_attribute".to_string());
        let (status, message) =
            classify_matched(&entry, &finding, 87, today(), &cfg, CheckMode::NoNew);
        assert_eq!(status, MatchStatus::InvalidSelector);
        assert!(message.contains("without required policy:<allow-id> reference"));

        finding.identity.target_fingerprint = Some("policy:allow-other".to_string());
        let (status, message) =
            classify_matched(&entry, &finding, 87, today(), &cfg, CheckMode::NoNew);
        assert_eq!(status, MatchStatus::InvalidSelector);
        assert!(message.contains("policy:allow-other"));

        finding.identity.target_fingerprint = Some("policy:allow-1".to_string());
        let (status, message) =
            classify_matched(&entry, &finding, 87, today(), &cfg, CheckMode::NoNew);
        assert_eq!(status, MatchStatus::Matched);
        assert!(message.contains("matched with structural score 87"));
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
        let (status, message) =
            classify_matched(&drift_entry, &finding, 91, today(), &cfg, CheckMode::NoNew);
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

        let (status, message) =
            classify_matched(&baseline, &finding, 91, today(), &cfg, CheckMode::Release);

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
        let (status, message) =
            classify_matched(&baseline, &finding, 91, today(), &cfg, CheckMode::Release);
        assert_eq!(status, MatchStatus::BaselineDebt);
        assert!(message.contains("cannot pass release mode"));

        let mut never = entry(FindingKind::Panic);
        never.lifecycle.expires = Some("never".to_string());
        let (status, message) =
            classify_matched(&never, &finding, 91, today(), &cfg, CheckMode::NoNew);
        assert_eq!(status, MatchStatus::Matched);
        assert!(message.contains("matched with structural score 91"));
    }
}
