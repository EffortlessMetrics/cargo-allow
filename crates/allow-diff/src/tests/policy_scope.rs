use super::*;

#[test]
fn detects_scope_broadening_from_path_to_glob() {
    let base = config_with(entry("allow-1"));
    let mut widened = entry("allow-1");
    widened.path = None;
    widened.glob = Some("src/**".to_string());
    let head = config_with(widened);

    let changes = policy_changes(&base, &head);

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
fn detects_scope_broadening_from_windows_path_to_glob() {
    let mut base_entry = entry("allow-1");
    base_entry.path = Some(PathBuf::from(r"src\lib.rs"));
    let base = config_with(base_entry);
    let mut widened = entry("allow-1");
    widened.path = None;
    widened.glob = Some("src/**".to_string());
    let head = config_with(widened);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ScopeBroadened
            && change.severity == PolicyChangeSeverity::Fail
    }));
}

#[test]
fn detects_scope_broadening_between_entry_globs() {
    let mut base_entry = entry("allow-1");
    base_entry.path = None;
    base_entry.glob = Some("src/parser/**".to_string());
    let base = config_with(base_entry);
    let mut widened = entry("allow-1");
    widened.path = None;
    widened.glob = Some("src/**".to_string());
    let head = config_with(widened);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ScopeBroadened
            && change.severity == PolicyChangeSeverity::Fail
    }));
}

#[test]
fn detects_selector_glob_broadening_even_when_path_remains() {
    let mut base_entry = entry("allow-1");
    base_entry.selector.glob = Some("src/lib.rs".to_string());
    let base = config_with(base_entry);
    let mut widened = entry("allow-1");
    widened.selector.glob = Some("src/**".to_string());
    let head = config_with(widened);

    let changes = policy_changes(&base, &head);

    assert!(
        changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::ScopeBroadened)
    );
    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::ScopeBroadened)
        .unwrap_or_else(|| std::panic::panic_any("selector glob broadening should be reported"));
    let scope = change.scope.as_ref().unwrap_or_else(|| {
        std::panic::panic_any("selector glob broadening should include structured scope delta")
    });
    assert_eq!(scope.field, ScopeChangeField::SelectorGlob);
    assert_eq!(scope.before.as_deref(), Some("src/lib.rs"));
    assert_eq!(scope.after.as_deref(), Some("src/**"));
}

#[test]
fn detects_selector_glob_narrowing_even_when_path_remains() {
    let mut base_entry = entry("allow-1");
    base_entry.selector.glob = Some("src/**".to_string());
    let base = config_with(base_entry);
    let mut narrowed = entry("allow-1");
    narrowed.selector.glob = Some("src/parser/**".to_string());
    let head = config_with(narrowed);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ScopeNarrowed
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}

#[test]
fn detects_scope_narrowing_from_glob_to_path() {
    let mut base_entry = entry("allow-1");
    base_entry.path = None;
    base_entry.glob = Some("src/**".to_string());
    let base = config_with(base_entry);
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ScopeNarrowed
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}

#[test]
fn detects_scope_narrowing_between_globs() {
    let mut base_entry = entry("allow-1");
    base_entry.path = None;
    base_entry.glob = Some("src/**".to_string());
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.path = None;
    head_entry.glob = Some("src/parser/**".to_string());
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ScopeNarrowed
            && change.severity == PolicyChangeSeverity::Improvement
    }));
}

#[test]
fn scope_broadening_respects_directory_segment_boundaries() {
    let mut widened = entry("allow-1");
    widened.path = None;
    widened.glob = Some("src/parse/**".to_string());
    let head = config_with(widened);

    let changes = policy_changes(&config_with(entry("allow-1")), &head);

    assert!(
        !changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::ScopeBroadened),
        "src/parse/** must not be treated as covering src/parser/lib.rs"
    );
}

#[test]
fn glob_scope_changes_respect_directory_segment_boundaries() {
    let mut base_entry = entry("allow-1");
    base_entry.path = None;
    base_entry.glob = Some("src/parser/**".to_string());
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.path = None;
    head_entry.glob = Some("src/parse/**".to_string());
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    assert!(
        !changes.iter().any(|change| {
            matches!(
                change.kind,
                PolicyChangeKind::ScopeBroadened | PolicyChangeKind::ScopeNarrowed
            )
        }),
        "sibling directory globs should not be classified as broadened or narrowed"
    );
    assert!(
        changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::ScopeChanged),
        "sibling directory globs should be surfaced as a review-required retarget"
    );
}

#[test]
fn detects_exact_scope_retarget_as_review_required() {
    let mut base_entry = entry("allow-1");
    base_entry.path = Some(PathBuf::from("src/parser/lib.rs"));
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-1");
    head_entry.path = Some(PathBuf::from(r"src\runtime\lib.rs"));
    let head = config_with(head_entry);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::ScopeChanged
            && change.severity == PolicyChangeSeverity::Review
            && change.message.contains("scope changed")
    }));
}
