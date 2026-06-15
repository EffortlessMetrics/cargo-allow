use super::identity_policy_changes;
use crate::{ExceptionIdentityChangeField, PolicyChangeKind, PolicyChangeSeverity};
use allow_core::{AllowEntry, FindingKind, Lifecycle, Selector};
use std::path::PathBuf;

fn entry(id: &str) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Range is validated before use.".to_string(),
        evidence: vec!["test:range_is_validated".to_string()],
        links: Vec::new(),
        occurrence_limit: Some(1),
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: Some("2026-08-01".to_string()),
            expires: Some("2026-09-01".to_string()),
        },
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            container: Some("load".to_string()),
            callee: Some("unwrap".to_string()),
            normalized_snippet_hash: Some("fnv1a64:1234".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

#[test]
fn identity_policy_changes_reports_kind_changed() {
    let base = entry("allow-1");
    let mut head = entry("allow-1");
    head.kind = FindingKind::Unsafe;

    let changes = identity_policy_changes(&base, &head);

    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::KindChanged)
        .unwrap_or_else(|| std::panic::panic_any("kind change should be reported"));
    assert_eq!(change.severity, PolicyChangeSeverity::Fail);
    assert_eq!(
        change.exception_identity.as_ref().map(|identity| {
            (
                identity.field,
                identity.before.as_deref(),
                identity.after.as_deref(),
            )
        }),
        Some((
            ExceptionIdentityChangeField::Kind,
            Some("panic"),
            Some("unsafe")
        ))
    );
}

#[test]
fn identity_policy_changes_reports_family_changed() {
    let base = entry("allow-1");
    let mut head = entry("allow-1");
    head.family = Some("expect".to_string());

    let changes = identity_policy_changes(&base, &head);

    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::FamilyChanged)
        .unwrap_or_else(|| std::panic::panic_any("family change should be reported"));
    assert_eq!(change.severity, PolicyChangeSeverity::Fail);
    assert_eq!(
        change.exception_identity.as_ref().map(|identity| {
            (
                identity.field,
                identity.before.as_deref(),
                identity.after.as_deref(),
            )
        }),
        Some((
            ExceptionIdentityChangeField::Family,
            Some("unwrap"),
            Some("expect")
        ))
    );
}

#[test]
fn identity_policy_changes_reports_removed_family() {
    let base = entry("allow-1");
    let mut head = entry("allow-1");
    head.family = None;

    let changes = identity_policy_changes(&base, &head);

    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::FamilyChanged)
        .unwrap_or_else(|| std::panic::panic_any("removed family should be reported"));
    assert_eq!(
        change.exception_identity.as_ref().map(|identity| {
            (
                identity.field,
                identity.before.as_deref(),
                identity.after.as_deref(),
            )
        }),
        Some((ExceptionIdentityChangeField::Family, Some("unwrap"), None))
    );
}

#[test]
fn identity_policy_changes_returns_empty_when_identity_unchanged() {
    let base = entry("allow-1");
    let head = entry("allow-1");

    let changes = identity_policy_changes(&base, &head);

    assert_eq!(changes.len(), 0);
}
