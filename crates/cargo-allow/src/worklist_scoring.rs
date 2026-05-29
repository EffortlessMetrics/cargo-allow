use allow_core::{AllowEntry, Finding, FindingKind, MatchOutcome, MatchStatus};

use super::worklist_item_kind::{
    AMBIGUOUS_SELECTOR, BASELINE_DEBT, EXPIRED_ALLOW, INVALID_SELECTOR, MATCHED, MISSING_EVIDENCE,
    MISSING_REQUIRED_FIELD, NEW_UNRECEIPTED_FINDING, OCCURRENCE_LIMIT_EXCEEDED, REVIEW_DUE,
    STALE_ALLOW, UNSAFE_MISSING_EVIDENCE,
};

pub(crate) fn work_item_kind(
    outcome: &MatchOutcome,
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
) -> String {
    match outcome.status {
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
        MatchStatus::ReviewDue => REVIEW_DUE.to_string(),
        MatchStatus::Matched => MATCHED.to_string(),
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
        return "low";
    }
    if matches!(
        (exception_kind, family),
        (
            Some(FindingKind::PolicyException),
            Some("process_spawn" | "network_destination")
        )
    ) {
        return "high";
    }
    if matches!(exception_kind, Some(FindingKind::Unsafe)) {
        return "high";
    }
    match (kind, status) {
        (AMBIGUOUS_SELECTOR, _) | (_, MatchStatus::Expired) => "high",
        (NEW_UNRECEIPTED_FINDING, _) | (OCCURRENCE_LIMIT_EXCEEDED, _) => "medium",
        (MISSING_EVIDENCE, _) | (MISSING_REQUIRED_FIELD, _) | (INVALID_SELECTOR, _) => "medium",
        (BASELINE_DEBT, _) | (REVIEW_DUE, _) => "medium",
        (STALE_ALLOW, _) => "low",
        _ => "medium",
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
        STALE_ALLOW => "small",
        AMBIGUOUS_SELECTOR | INVALID_SELECTOR => "small",
        MISSING_REQUIRED_FIELD | MISSING_EVIDENCE => "small",
        REVIEW_DUE | BASELINE_DEBT => "medium",
        UNSAFE_MISSING_EVIDENCE => "medium",
        NEW_UNRECEIPTED_FINDING
            if matches!(
                exception_kind,
                Some(FindingKind::NonRustFile | FindingKind::GeneratedCode)
            ) =>
        {
            "small"
        }
        NEW_UNRECEIPTED_FINDING | OCCURRENCE_LIMIT_EXCEEDED => "medium",
        _ => "medium",
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
