use allow_core::{AllowEntry, Finding, FindingKind, MatchOutcome, MatchStatus};

use super::worklist_item_kind::{
    AMBIGUOUS_SELECTOR, BASELINE_DEBT, EXPIRED_ALLOW, INVALID_SELECTOR, MATCHED, MISSING_EVIDENCE,
    MISSING_REQUIRED_FIELD, NEW_UNRECEIPTED_FINDING, OCCURRENCE_HEADROOM,
    OCCURRENCE_LIMIT_EXCEEDED, REVIEW_DUE, STALE_ALLOW, UNSAFE_MISSING_EVIDENCE,
};
use super::worklist_priority::{
    DIFFICULTY_MEDIUM, DIFFICULTY_SMALL, RISK_HIGH, RISK_LOW, RISK_MEDIUM,
};

pub(crate) fn work_item_kind(
    outcome: &MatchOutcome,
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
) -> String {
    work_item_kind_for_status(outcome.status, outcome, finding, entry)
}

pub(crate) fn work_item_kind_for_status(
    status: MatchStatus,
    outcome: &MatchOutcome,
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
) -> String {
    match status {
        MatchStatus::New if outcome.allow_id.is_some() => OCCURRENCE_LIMIT_EXCEEDED.to_string(),
        MatchStatus::New => NEW_UNRECEIPTED_FINDING.to_string(),
        MatchStatus::Expired => EXPIRED_ALLOW.to_string(),
        MatchStatus::Stale => STALE_ALLOW.to_string(),
        MatchStatus::Ambiguous => AMBIGUOUS_SELECTOR.to_string(),
        MatchStatus::EvidenceMissing
            if finding
                .map(|finding| finding.kind == FindingKind::Unsafe)
                .or_else(|| entry.map(|entry| entry.kind == FindingKind::Unsafe))
                .unwrap_or(false) =>
        {
            UNSAFE_MISSING_EVIDENCE.to_string()
        }
        MatchStatus::EvidenceMissing => MISSING_EVIDENCE.to_string(),
        MatchStatus::MissingRequiredField => MISSING_REQUIRED_FIELD.to_string(),
        MatchStatus::InvalidSelector => INVALID_SELECTOR.to_string(),
        MatchStatus::BaselineDebt => BASELINE_DEBT.to_string(),
        MatchStatus::ReviewDue | MatchStatus::LocationDrift => REVIEW_DUE.to_string(),
        MatchStatus::Matched => MATCHED.to_string(),
        _ => "unknown_match_status".to_string(),
    }
}

pub(super) fn work_item_risk(
    kind: &str,
    status: MatchStatus,
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
) -> &'static str {
    let exception_kind = finding
        .map(|finding| finding.kind)
        .or_else(|| entry.map(|entry| entry.kind));
    let family = exception_family(finding, entry);
    if matches!(status, MatchStatus::Stale) {
        return RISK_LOW;
    }
    if matches!(
        (exception_kind, family),
        (
            Some(FindingKind::PolicyException),
            Some("process_spawn" | "network_destination")
        )
    ) {
        return RISK_HIGH;
    }
    if matches!(exception_kind, Some(FindingKind::Unsafe)) {
        return RISK_HIGH;
    }
    match (kind, status) {
        (AMBIGUOUS_SELECTOR, _) | (_, MatchStatus::Expired) => RISK_HIGH,
        (NEW_UNRECEIPTED_FINDING, _) | (OCCURRENCE_LIMIT_EXCEEDED, _) => RISK_MEDIUM,
        (MISSING_EVIDENCE, _) | (MISSING_REQUIRED_FIELD, _) | (INVALID_SELECTOR, _) => RISK_MEDIUM,
        (BASELINE_DEBT, _) | (REVIEW_DUE, _) => RISK_MEDIUM,
        (OCCURRENCE_HEADROOM, _) => RISK_LOW,
        (STALE_ALLOW, _) => RISK_LOW,
        _ => RISK_MEDIUM,
    }
}

pub(super) fn work_item_difficulty(
    kind: &str,
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
) -> &'static str {
    let exception_kind = finding
        .map(|finding| finding.kind)
        .or_else(|| entry.map(|entry| entry.kind));
    match kind {
        STALE_ALLOW => DIFFICULTY_SMALL,
        OCCURRENCE_HEADROOM => DIFFICULTY_SMALL,
        AMBIGUOUS_SELECTOR | INVALID_SELECTOR => DIFFICULTY_SMALL,
        MISSING_REQUIRED_FIELD | MISSING_EVIDENCE => DIFFICULTY_SMALL,
        REVIEW_DUE | BASELINE_DEBT => DIFFICULTY_MEDIUM,
        UNSAFE_MISSING_EVIDENCE => DIFFICULTY_MEDIUM,
        NEW_UNRECEIPTED_FINDING
            if matches!(
                exception_kind,
                Some(FindingKind::NonRustFile | FindingKind::GeneratedCode)
            ) =>
        {
            DIFFICULTY_SMALL
        }
        NEW_UNRECEIPTED_FINDING | OCCURRENCE_LIMIT_EXCEEDED => DIFFICULTY_MEDIUM,
        _ => DIFFICULTY_MEDIUM,
    }
}

pub(super) fn exception_family<'a>(
    finding: Option<&'a Finding>,
    entry: Option<&'a AllowEntry>,
) -> Option<&'a str> {
    finding
        .and_then(|finding| finding.family.as_deref())
        .or_else(|| entry.and_then(|entry| entry.family.as_deref()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use allow_core::{Lifecycle, Selector, Span, StructuralIdentity};

    use super::*;

    #[test]
    fn work_item_kind_maps_status_and_unsafe_evidence_cases() {
        let cases = vec![
            (outcome(MatchStatus::Matched, None), None, None, MATCHED),
            (
                outcome(MatchStatus::New, None),
                None,
                None,
                NEW_UNRECEIPTED_FINDING,
            ),
            (
                outcome(MatchStatus::New, Some("allow-limit")),
                None,
                None,
                OCCURRENCE_LIMIT_EXCEEDED,
            ),
            (
                outcome(MatchStatus::Expired, Some("allow-expired")),
                None,
                None,
                EXPIRED_ALLOW,
            ),
            (
                outcome(MatchStatus::Stale, Some("allow-stale")),
                None,
                None,
                STALE_ALLOW,
            ),
            (
                outcome(MatchStatus::Ambiguous, Some("allow-ambiguous")),
                None,
                None,
                AMBIGUOUS_SELECTOR,
            ),
            (
                outcome(MatchStatus::InvalidSelector, Some("allow-invalid")),
                None,
                None,
                INVALID_SELECTOR,
            ),
            (
                outcome(MatchStatus::MissingRequiredField, Some("allow-missing")),
                None,
                None,
                MISSING_REQUIRED_FIELD,
            ),
            (
                outcome(MatchStatus::BaselineDebt, Some("allow-baseline")),
                None,
                None,
                BASELINE_DEBT,
            ),
            (
                outcome(MatchStatus::ReviewDue, Some("allow-review")),
                None,
                None,
                REVIEW_DUE,
            ),
            (
                outcome(MatchStatus::EvidenceMissing, Some("allow-panic")),
                Some(finding(FindingKind::Panic, Some("unwrap"))),
                None,
                MISSING_EVIDENCE,
            ),
            (
                outcome(MatchStatus::EvidenceMissing, Some("allow-unsafe")),
                Some(finding(FindingKind::Unsafe, Some("unsafe_fn"))),
                None,
                UNSAFE_MISSING_EVIDENCE,
            ),
            (
                outcome(MatchStatus::EvidenceMissing, Some("allow-entry-unsafe")),
                None,
                Some(entry(FindingKind::Unsafe, Some("unsafe_block"))),
                UNSAFE_MISSING_EVIDENCE,
            ),
        ];

        for (outcome, finding, entry, expected) in cases {
            assert_eq!(
                work_item_kind(&outcome, finding.as_ref(), entry.as_ref()),
                expected
            );
        }
    }

    #[test]
    fn work_item_risk_follows_status_and_exception_boundaries() {
        let process = finding(FindingKind::PolicyException, Some("process_spawn"));
        let network = entry(FindingKind::PolicyException, Some("network_destination"));
        let unsafe_finding = finding(FindingKind::Unsafe, Some("unsafe_fn"));
        let panic = finding(FindingKind::Panic, Some("unwrap"));

        let cases = vec![
            (
                NEW_UNRECEIPTED_FINDING,
                MatchStatus::Stale,
                Some(unsafe_finding.clone()),
                None,
                RISK_LOW,
            ),
            (
                NEW_UNRECEIPTED_FINDING,
                MatchStatus::New,
                Some(process),
                None,
                RISK_HIGH,
            ),
            (
                NEW_UNRECEIPTED_FINDING,
                MatchStatus::New,
                None,
                Some(network),
                RISK_HIGH,
            ),
            (
                NEW_UNRECEIPTED_FINDING,
                MatchStatus::New,
                Some(unsafe_finding),
                None,
                RISK_HIGH,
            ),
            (
                AMBIGUOUS_SELECTOR,
                MatchStatus::Ambiguous,
                Some(panic.clone()),
                None,
                RISK_HIGH,
            ),
            (
                EXPIRED_ALLOW,
                MatchStatus::Expired,
                None,
                Some(entry(FindingKind::Panic, Some("unwrap"))),
                RISK_HIGH,
            ),
            (
                NEW_UNRECEIPTED_FINDING,
                MatchStatus::New,
                Some(panic.clone()),
                None,
                RISK_MEDIUM,
            ),
            (
                OCCURRENCE_LIMIT_EXCEEDED,
                MatchStatus::New,
                Some(panic.clone()),
                None,
                RISK_MEDIUM,
            ),
            (
                MISSING_REQUIRED_FIELD,
                MatchStatus::MissingRequiredField,
                None,
                Some(entry(FindingKind::Panic, Some("unwrap"))),
                RISK_MEDIUM,
            ),
            (
                STALE_ALLOW,
                MatchStatus::Stale,
                None,
                Some(entry(FindingKind::Panic, Some("unwrap"))),
                RISK_LOW,
            ),
            (
                MATCHED,
                MatchStatus::Matched,
                Some(panic),
                None,
                RISK_MEDIUM,
            ),
        ];

        for (kind, status, finding, entry, expected) in cases {
            assert_eq!(
                work_item_risk(kind, status, finding.as_ref(), entry.as_ref()),
                expected
            );
        }
    }

    #[test]
    fn work_item_difficulty_tracks_kind_and_exception_size() {
        let non_rust = finding(FindingKind::NonRustFile, Some("shell_script"));
        let generated = entry(FindingKind::GeneratedCode, Some("generated_code"));
        let panic = finding(FindingKind::Panic, Some("unwrap"));

        let cases = vec![
            (STALE_ALLOW, None, None, DIFFICULTY_SMALL),
            (AMBIGUOUS_SELECTOR, None, None, DIFFICULTY_SMALL),
            (INVALID_SELECTOR, None, None, DIFFICULTY_SMALL),
            (MISSING_REQUIRED_FIELD, None, None, DIFFICULTY_SMALL),
            (MISSING_EVIDENCE, None, None, DIFFICULTY_SMALL),
            (REVIEW_DUE, None, None, DIFFICULTY_MEDIUM),
            (BASELINE_DEBT, None, None, DIFFICULTY_MEDIUM),
            (UNSAFE_MISSING_EVIDENCE, None, None, DIFFICULTY_MEDIUM),
            (
                NEW_UNRECEIPTED_FINDING,
                Some(non_rust),
                None,
                DIFFICULTY_SMALL,
            ),
            (
                NEW_UNRECEIPTED_FINDING,
                None,
                Some(generated),
                DIFFICULTY_SMALL,
            ),
            (
                NEW_UNRECEIPTED_FINDING,
                Some(panic),
                None,
                DIFFICULTY_MEDIUM,
            ),
            (OCCURRENCE_LIMIT_EXCEEDED, None, None, DIFFICULTY_MEDIUM),
        ];

        for (kind, finding, entry, expected) in cases {
            assert_eq!(
                work_item_difficulty(kind, finding.as_ref(), entry.as_ref()),
                expected
            );
        }
    }

    #[test]
    fn work_item_difficulty_always_matches_schema_enum() {
        // Regression for #1968: docs/schemas/worklist.schema.json's
        // work_item.difficulty enum only lists "small"/"medium". This pins
        // that work_item_difficulty can never emit a value outside
        // DIFFICULTY_LEVELS, across every work item kind and exception kind,
        // so a future difficulty tier can't silently fail schema validation.
        use super::super::worklist_item_kind::WORK_ITEM_KINDS;
        use super::super::worklist_priority::DIFFICULTY_LEVELS;

        let assert_valid_difficulty = |label: String, difficulty: &'static str| {
            assert!(
                DIFFICULTY_LEVELS.contains(&difficulty),
                "{label} produced difficulty {difficulty:?} outside {DIFFICULTY_LEVELS:?}"
            );
        };

        for &kind in WORK_ITEM_KINDS {
            assert_valid_difficulty(
                format!("{kind} with no finding/entry"),
                work_item_difficulty(kind, None, None),
            );

            for &exception_kind in FindingKind::ALL {
                let via_finding = finding(exception_kind, Some("family"));
                assert_valid_difficulty(
                    format!("{kind}/{exception_kind:?} finding"),
                    work_item_difficulty(kind, Some(&via_finding), None),
                );

                let via_entry = entry(exception_kind, Some("family"));
                assert_valid_difficulty(
                    format!("{kind}/{exception_kind:?} entry"),
                    work_item_difficulty(kind, None, Some(&via_entry)),
                );
            }
        }
    }

    #[test]
    fn exception_family_prefers_current_finding_and_falls_back_to_entry() {
        let current = finding(FindingKind::Panic, Some("expect"));
        let policy = entry(FindingKind::Panic, Some("unwrap"));
        let missing_family = finding(FindingKind::Panic, None);

        assert_eq!(
            exception_family(Some(&current), Some(&policy)),
            Some("expect")
        );
        assert_eq!(
            exception_family(Some(&missing_family), Some(&policy)),
            Some("unwrap")
        );
        assert_eq!(exception_family(None, Some(&policy)), Some("unwrap"));
        assert_eq!(exception_family(Some(&missing_family), None), None);
    }

    fn outcome(status: MatchStatus, allow_id: Option<&str>) -> MatchOutcome {
        MatchOutcome {
            status,
            allow_id: allow_id.map(str::to_string),
            candidate_ids: Vec::new(),
            finding_index: None,
            message: "test outcome".to_string(),
            score: 100,
        }
    }

    fn finding(kind: FindingKind, family: Option<&str>) -> Finding {
        Finding {
            kind,
            family: family.map(str::to_string),
            path: PathBuf::from("src/lib.rs"),
            span: Some(Span { line: 1, column: 1 }),
            identity: StructuralIdentity::new("rust", "method_call"),
            message: "test finding".to_string(),
            ledger: None,
        }
    }

    fn entry(kind: FindingKind, family: Option<&str>) -> AllowEntry {
        AllowEntry {
            id: "allow-test".to_string(),
            kind,
            family: family.map(str::to_string),
            path: Some(PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "owner".to_string(),
            classification: "reviewed_exception".to_string(),
            reason: "test policy entry".to_string(),
            evidence: Vec::new(),
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle::empty(),
            selector: Selector {
                ast_kind: Some("method_call".to_string()),
                ..Selector::default()
            },
            last_seen: None,
        }
    }
}
