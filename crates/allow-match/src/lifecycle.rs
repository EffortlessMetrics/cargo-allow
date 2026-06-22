use allow_core::{AllowEntry, MatchStatus, SimpleDate};

pub(crate) fn unused_entry_status(entry: &AllowEntry, today: SimpleDate) -> MatchStatus {
    if entry_is_expired(entry, today) {
        return MatchStatus::Expired;
    }
    if entry_review_is_due(entry, today) {
        return MatchStatus::ReviewDue;
    }
    MatchStatus::Stale
}

/// Returns true if the entry's `expires` date has passed.
///
/// Fail-safe: if `expires` is `Some` but unparseable (e.g. `"2026-13-40"`),
/// the entry is treated as expired. A malformed expiry must never silently
/// make an entry immortal (#1804).
pub(crate) fn entry_is_expired(entry: &AllowEntry, today: SimpleDate) -> bool {
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
/// Fail-safe: if `review_after` is `Some` but unparseable, treat as due
/// (review is required). A malformed date must never silently suppress
/// review (#1804).
pub(crate) fn entry_review_is_due(entry: &AllowEntry, today: SimpleDate) -> bool {
    match entry.lifecycle.review_after.as_deref() {
        Some(review_after) => match SimpleDate::parse(review_after) {
            Some(date) => date <= today,
            None => true,
        },
        None => false,
    }
}
