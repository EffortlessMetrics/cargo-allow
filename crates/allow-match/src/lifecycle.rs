use allow_core::{AllowEntry, MatchStatus, SimpleDate};

pub(crate) fn unused_entry_status(entry: &AllowEntry, today: SimpleDate) -> MatchStatus {
    // A malformed lifecycle date is a policy defect, not a maintenance
    // posture: fail closed even when no finding currently matches (#1804,
    // #4238). Collapsing into `Expired` would silently become advisory once
    // expiry lost candidate authority.
    if malformed_lifecycle_reason(entry).is_some() {
        return MatchStatus::MissingRequiredField;
    }
    if entry_is_expired(entry, today) {
        return MatchStatus::Expired;
    }
    if entry_review_is_due(entry, today) {
        return MatchStatus::ReviewDue;
    }
    MatchStatus::Stale
}

/// Human-readable reason an entry's lifecycle dates are malformed, when they
/// are. `expires = "never"` is the documented immortal form and never
/// malformed.
///
/// The caller must treat `Some` as fail-closed: an unparseable date must never
/// silently become advisory or silently authorize (#1804).
///
/// Public for the #4239 cadence classifier, which reuses the match-engine
/// lifecycle predicates so the read-only cadence surface can never disagree
/// with match statuses on malformed or boundary dates.
pub fn malformed_lifecycle_reason(entry: &AllowEntry) -> Option<String> {
    if let Some(expires) = entry.lifecycle.expires.as_deref()
        && expires != "never"
        && SimpleDate::parse(expires).is_none()
    {
        return Some(format!(
            "{} has malformed lifecycle date: expires `{expires}` is not a parseable YYYY-MM-DD date; failing closed (#1804)",
            entry.id
        ));
    }
    if let Some(review_after) = entry.lifecycle.review_after.as_deref()
        && SimpleDate::parse(review_after).is_none()
    {
        return Some(format!(
            "{} has malformed lifecycle date: review_after `{review_after}` is not a parseable YYYY-MM-DD date; failing closed (#1804)",
            entry.id
        ));
    }
    None
}

/// Returns true if the entry's `expires` date has passed.
///
/// Boundary law: STRICT `date < today`. The expires day itself is not
/// expired; that deliberate asymmetry against [`entry_review_is_due`]'s
/// inclusive form is recorded in #2008 and reused verbatim by the #4239
/// cadence classifier.
///
/// Fail-safe: if `expires` is `Some` but unparseable (e.g. `"2026-13-40"`),
/// the entry is treated as expired. A malformed expiry must never silently
/// make an entry immortal (#1804). Classification funnels malformed dates
/// through [`malformed_lifecycle_reason`] to a blocking status before this
/// predicate is consulted, so the fail-safe here only guards direct callers.
pub fn entry_is_expired(entry: &AllowEntry, today: SimpleDate) -> bool {
    match entry.lifecycle.expires.as_deref() {
        Some("never") => false,
        Some(expires) => {
            // If the date parses, compare normally. If it does NOT parse,
            // fail-safe: treat as expired (the entry's lifecycle is broken).
            match SimpleDate::parse(expires) {
                Some(date) => date < today,
                None => true,
            }
        }
        None => false,
    }
}

/// Returns true if the entry's `review_after` date has been reached.
///
/// Boundary law: INCLUSIVE `date <= today`; the deadline day itself is due.
/// That deliberate asymmetry against [`entry_is_expired`]'s strict form is
/// recorded in #2008 and reused verbatim by the #4239 cadence classifier.
///
/// Fail-safe: if `review_after` is `Some` but unparseable, treat as due
/// (review is required). A malformed date must never silently suppress
/// review (#1804).
pub fn entry_review_is_due(entry: &AllowEntry, today: SimpleDate) -> bool {
    match entry.lifecycle.review_after.as_deref() {
        Some(review_after) => match SimpleDate::parse(review_after) {
            Some(date) => date <= today,
            None => true,
        },
        None => false,
    }
}
