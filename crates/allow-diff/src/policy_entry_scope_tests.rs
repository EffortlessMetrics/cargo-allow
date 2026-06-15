use super::{normalize_scope_text, scope_policy_changes};
use crate::{PolicyChangeKind, PolicyChangeSeverity, ScopeChangeField};
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
fn scope_policy_changes_reports_scope_broadening() {
    let base = entry("allow-1");
    let mut head = entry("allow-1");
    head.path = None;
    head.glob = Some("src/**".to_string());

    let changes = scope_policy_changes(&base, &head);

    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::ScopeBroadened)
        .unwrap_or_else(|| std::panic::panic_any("scope broadening should be reported"));
    assert_eq!(change.severity, PolicyChangeSeverity::Fail);
    let scope = change.scope.as_ref().unwrap_or_else(|| {
        std::panic::panic_any("scope broadening should include structured scope delta")
    });
    assert_eq!(scope.field, ScopeChangeField::Effective);
    assert_eq!(scope.before.as_deref(), Some("src/lib.rs"));
    assert_eq!(scope.after.as_deref(), Some("src/**"));
}

#[test]
fn scope_policy_changes_reports_scope_narrowing() {
    let mut base = entry("allow-1");
    base.path = None;
    base.glob = Some("src/**".to_string());
    let head = entry("allow-1");

    let changes = scope_policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ScopeNarrowed
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}

#[test]
fn scope_policy_changes_reports_scope_changed_for_sibling_globs() {
    let mut base = entry("allow-1");
    base.path = None;
    base.glob = Some("src/parser/**".to_string());
    let mut head = entry("allow-1");
    head.path = None;
    head.glob = Some("src/parse/**".to_string());

    let changes = scope_policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ScopeChanged
            && change.severity == PolicyChangeSeverity::Review
    }));
}

#[test]
fn normalize_scope_text_replaces_backslashes() {
    assert_eq!(
        normalize_scope_text(r"src\parser\lib.rs"),
        "src/parser/lib.rs"
    );
}
