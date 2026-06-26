use std::time::{SystemTime, UNIX_EPOCH};

use crate::SimpleDate;

#[test]
fn simple_date_rejects_invalid_calendar_dates() {
    assert!(SimpleDate::parse("2026-02-29").is_none());
    assert!(SimpleDate::parse("2024-02-29").is_some());
    assert!(SimpleDate::parse("2026-04-31").is_none());
    assert!(SimpleDate::parse("2026-13-01").is_none());
}

#[test]
fn simple_date_rejects_out_of_range_years() {
    // Regression for #1827: unbounded years allowed typos like
    // "99999-01-01" to silently validate, making entries immortal.
    assert!(SimpleDate::parse("99999-01-01").is_none());
    assert!(SimpleDate::parse("10000-01-01").is_none());
    assert!(SimpleDate::parse("1899-12-31").is_none());
    // Valid boundaries
    assert!(SimpleDate::parse("1900-01-01").is_some());
    assert!(SimpleDate::parse("9999-12-31").is_some());
    assert!(SimpleDate::parse("3026-01-01").is_some()); // in range
    assert!(SimpleDate::parse("2026-06-21").is_some());
}

#[test]
fn simple_date_counts_days_between_dates() {
    let start = SimpleDate::parse("2026-05-26")
        .unwrap_or_else(|| std::panic::panic_any("valid start date"));
    let end =
        SimpleDate::parse("2026-08-01").unwrap_or_else(|| std::panic::panic_any("valid end date"));

    assert_eq!(start.days_until(end), 67);
}

#[test]
fn simple_date_adds_days_across_months() {
    let start = SimpleDate::parse("2026-05-26")
        .unwrap_or_else(|| std::panic::panic_any("valid start date"));

    assert_eq!(start.add_days(67).to_string(), "2026-08-01");
}

#[test]
fn simple_date_converts_unix_epoch_days() {
    assert_eq!(
        SimpleDate::from_days_since_unix_epoch(0).to_string(),
        "1970-01-01"
    );
    assert_eq!(
        SimpleDate::from_days_since_unix_epoch(
            SimpleDate::parse("2026-05-27")
                .unwrap_or_else(|| std::panic::panic_any("valid date"))
                .days_since_unix_epoch()
        )
        .to_string(),
        "2026-05-27"
    );
}

#[test]
fn today_utc_approx_uses_system_clock_day() {
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() / 86_400)
        .unwrap_or(0);
    let today = SimpleDate::today_utc_approx();
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() / 86_400)
        .unwrap_or(0);

    let today_days = today.days_since_unix_epoch() as u64;
    assert!(
        (before..=after).contains(&today_days),
        "today_utc_approx should use the current UTC day"
    );
}

#[test]
fn has_passed_date_str_is_inclusive_of_today() {
    // #2008: an `expires`/deadline date equal to today must count as passed.
    // The deadline has arrived; enforcing it a day late (strict `<`) let drain
    // windows and entries live one extra day past their stated expiry.
    let today = SimpleDate::parse("2026-06-26")
        .unwrap_or_else(|| std::panic::panic_any("fixture today should parse"));

    // The deadline day itself is passed (inclusive).
    assert!(SimpleDate::has_passed_date_str(Some("2026-06-26"), today));
    // A deadline later than today is not yet passed.
    assert!(!SimpleDate::has_passed_date_str(Some("2026-06-27"), today));
    // A deadline earlier than today is passed.
    assert!(SimpleDate::has_passed_date_str(Some("2026-06-25"), today));
}

#[test]
fn has_passed_date_str_handles_missing_and_unparseable() {
    let today = SimpleDate::parse("2026-06-26")
        .unwrap_or_else(|| std::panic::panic_any("fixture today should parse"));

    // No deadline -> not passed (nothing to enforce).
    assert!(!SimpleDate::has_passed_date_str(None, today));
    // Unparseable deadline -> not passed (surfaced as a separate validation
    // error upstream, not silently treated as expired).
    assert!(!SimpleDate::has_passed_date_str(Some("not-a-date"), today));
}

#[test]
fn has_passed_and_is_due_share_the_inclusive_boundary() {
    // #2008: expiry (has_passed) and review_after (is_due) must agree on the
    // boundary. A deadline equal to today counts for both.
    let today = SimpleDate::parse("2026-06-26")
        .unwrap_or_else(|| std::panic::panic_any("fixture today should parse"));

    assert!(SimpleDate::has_passed_date_str(Some("2026-06-26"), today));
    assert!(SimpleDate::is_due_date_str(Some("2026-06-26"), today));
}
