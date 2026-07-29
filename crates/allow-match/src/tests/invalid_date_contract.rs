use crate::lifecycle::{entry_is_expired, entry_review_is_due};
use allow_core::{AllowEntry, FindingKind, Lifecycle, Selector, SimpleDate};
use std::path::PathBuf;

/// Two-layer invalid-date contract (#2680, acceptance criterion #2 from #1777):
///
/// Layer 1 — Loader rejects: `allow_policy::validate_lifecycle` returns an error
/// for malformed dates like "2026-13-40" or "not-a-date".
///
/// Layer 2 — Runtime fail-safe: if an entry with a malformed date somehow
/// reaches the match engine (e.g. constructed programmatically bypassing the
/// loader), `entry_is_expired` and `entry_review_is_due` treat unparseable
/// dates as expired/due — never as "valid and not yet due".
///
/// This test pins BOTH layers so a future refactor that drops either one is
/// caught immediately.
#[test]
fn two_layer_invalid_date_contract() {
    let today = SimpleDate::today_utc_approx();

    // --- Layer 1: Loader rejects malformed dates ---
    // We test this indirectly by verifying the parser function rejects them.
    // The full loader path is tested in allow-policy/src/lifecycle.rs tests.
    assert!(
        SimpleDate::parse("2026-13-40").is_none(),
        "SimpleDate::parse should reject invalid month/day '2026-13-40'"
    );
    assert!(
        SimpleDate::parse("not-a-date").is_none(),
        "SimpleDate::parse should reject garbage 'not-a-date'"
    );

    // --- Layer 2: Runtime fail-safe ---
    // An entry constructed with a malformed expires date (bypassing the loader)
    // must be treated as expired, not immortal.
    let entry_bad_expires = AllowEntry {
        id: "allow-bad-expires".to_string(),
        kind: FindingKind::Panic,
        family: None,
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "test".to_string(),
        classification: "reviewed".to_string(),
        reason: "test".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: None,
            review_after: None,
            expires: Some("2026-13-40".to_string()),
        },
        selector: Selector::default(),
        last_seen: None,
    };
    assert!(
        entry_is_expired(&entry_bad_expires, today),
        "malformed expires date must fail-safe to expired, not be treated as valid"
    );

    // An entry with a malformed review_after must be treated as review-due.
    let entry_bad_review = AllowEntry {
        id: "allow-bad-review".to_string(),
        kind: FindingKind::Panic,
        family: None,
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "test".to_string(),
        classification: "reviewed".to_string(),
        reason: "test".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: None,
            review_after: Some("not-a-date".to_string()),
            expires: None,
        },
        selector: Selector::default(),
        last_seen: None,
    };
    assert!(
        entry_review_is_due(&entry_bad_review, today),
        "malformed review_after date must fail-safe to review-due, not be treated as valid"
    );

    // Sanity: valid future dates should NOT trigger fail-safe.
    let entry_valid = AllowEntry {
        id: "allow-valid".to_string(),
        kind: FindingKind::Panic,
        family: None,
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "test".to_string(),
        classification: "reviewed".to_string(),
        reason: "test".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-01-01".to_string()),
            review_after: Some("2099-12-31".to_string()),
            expires: Some("2099-12-31".to_string()),
        },
        selector: Selector::default(),
        last_seen: None,
    };
    assert!(
        !entry_is_expired(&entry_valid, today),
        "valid future expires date must not be expired"
    );
    assert!(
        !entry_review_is_due(&entry_valid, today),
        "valid future review_after date must not be review-due"
    );
}
