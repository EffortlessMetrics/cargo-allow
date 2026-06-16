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

pub(crate) fn entry_is_expired(entry: &AllowEntry, today: SimpleDate) -> bool {
    match entry.lifecycle.expires.as_deref() {
        Some(expires) if expires != "never" => SimpleDate::is_before_date_str(Some(expires), today),
        _ => false,
    }
}

pub(crate) fn entry_review_is_due(entry: &AllowEntry, today: SimpleDate) -> bool {
    SimpleDate::is_due_date_str(entry.lifecycle.review_after.as_deref(), today)
}
