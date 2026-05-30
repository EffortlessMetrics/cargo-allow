use std::path::PathBuf;

use crate::{
    AllowEntry, FindingKind, Lifecycle, Selector, allow_entry_broad_scope, glob_matches_str,
    normalize_path, source_tree_path_is_ignored, source_tree_path_matches_filter,
    source_tree_scope_has_wildcard,
};

#[test]
fn glob_supports_double_star() {
    assert!(glob_matches_str("crates/**/*.rs", "crates/foo/src/lib.rs"));
    assert!(glob_matches_str(
        ".github/workflows/*.yml",
        ".github/workflows/ci.yml"
    ));
    assert!(!glob_matches_str(
        "scripts/*.sh",
        "scripts/release/build.sh"
    ));
}

#[test]
fn source_tree_path_filter_matches_exact_subtree_and_glob_scope() {
    assert!(source_tree_path_matches_filter(
        "crates/allow-core/src/lib.rs",
        "crates/allow-core"
    ));
    assert!(!source_tree_path_matches_filter(
        "crates/allow-core2/src/lib.rs",
        "crates/allow-core"
    ));
    assert!(source_tree_path_matches_filter(
        "scripts/**/*.sh",
        "scripts/release/build.sh"
    ));
    assert!(source_tree_path_matches_filter("README.md", "."));
}

#[test]
fn source_tree_ignore_matches_git_target_and_custom_subtrees() {
    let patterns = vec![
        ".git/**".to_string(),
        "target/**".to_string(),
        "scripts/**".to_string(),
    ];

    assert!(source_tree_path_is_ignored(".git/config", &patterns));
    assert!(source_tree_path_is_ignored(
        ".git/hooks/pre-commit",
        &patterns
    ));
    assert!(source_tree_path_is_ignored(
        "target/debug/cargo-allow",
        &patterns
    ));
    assert!(source_tree_path_is_ignored(
        "scripts/release/build.sh",
        &patterns
    ));
}

#[test]
fn source_tree_ignore_does_not_swallow_github() {
    let patterns = vec![".git/**".to_string()];

    assert!(!source_tree_path_is_ignored(
        ".github/workflows/ci.yml",
        &patterns
    ));
}

#[test]
fn source_tree_scope_wildcard_detection_covers_supported_glob_tokens() {
    for scope in ["scripts/*.sh", "scripts/?.sh", "scripts/**/*.sh"] {
        assert!(source_tree_scope_has_wildcard(scope));
    }
    assert!(!source_tree_scope_has_wildcard("scripts/[ab].sh"));
    assert!(!source_tree_scope_has_wildcard("scripts/{a,b}.sh"));
    assert!(!source_tree_scope_has_wildcard("scripts/release.sh"));
}

#[test]
fn allow_entry_broad_scope_uses_path_glob_selector_priority() {
    let mut entry = AllowEntry {
        id: "allow-panic".to_string(),
        kind: FindingKind::Panic,
        family: None,
        path: Some(PathBuf::from("src\\*.rs")),
        glob: Some(r"crates\**\*.rs".to_string()),
        owner: "team-runtime".to_string(),
        classification: "accepted-risk".to_string(),
        reason: "test fixture".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector {
            glob: Some("tests/**/*.rs".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    };

    assert_eq!(allow_entry_broad_scope(&entry).as_deref(), Some("src/*.rs"));

    entry.path = Some(PathBuf::from("src/lib.rs"));
    assert_eq!(
        allow_entry_broad_scope(&entry).as_deref(),
        Some("crates/**/*.rs")
    );

    entry.glob = Some("crates/lib.rs".to_string());
    entry.selector.glob = Some(r"tests\**\*.rs".to_string());
    assert_eq!(
        allow_entry_broad_scope(&entry).as_deref(),
        Some("tests/**/*.rs")
    );

    entry.selector.glob = Some("tests/lib.rs".to_string());
    assert_eq!(allow_entry_broad_scope(&entry), None);
}

#[test]
fn normalize_path_preserves_leading_parent_segments() {
    assert_eq!(normalize_path("../src/lib.rs"), "../src/lib.rs");
    assert_eq!(normalize_path("../../src/../README.md"), "../../README.md");
    assert_eq!(normalize_path("src/../README.md"), "README.md");
    assert_eq!(normalize_path(r"..\src\lib.rs"), "../src/lib.rs");
}

#[test]
fn normalize_path_preserves_absolute_unix_root() {
    assert_eq!(normalize_path("/a/../b"), "/b");
    assert_eq!(normalize_path("/../b"), "/b");
    assert_eq!(normalize_path("/"), "/");
    assert_eq!(normalize_path("/a//./b/"), "/a/b");
}

#[test]
fn allow_entry_path_or_glob_prefers_path_then_entry_glob_then_selector_glob() {
    let mut entry = AllowEntry {
        id: "allow-panic".to_string(),
        kind: FindingKind::Panic,
        family: None,
        path: Some(PathBuf::from("src/../src/lib.rs")),
        glob: Some("crates/**/*.rs".to_string()),
        owner: "team-runtime".to_string(),
        classification: "accepted-risk".to_string(),
        reason: "test fixture".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector {
            glob: Some("src/**/*.rs".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    };

    assert_eq!(entry.path_or_glob(), "src/lib.rs");

    entry.path = None;
    assert_eq!(entry.path_or_glob(), "crates/**/*.rs");

    entry.glob = Some(r"crates\**\*.rs".to_string());
    assert_eq!(entry.path_or_glob(), "crates/**/*.rs");

    entry.glob = None;
    assert_eq!(entry.path_or_glob(), "src/**/*.rs");

    entry.selector.glob = Some(r"src\**\*.rs".to_string());
    assert_eq!(entry.path_or_glob(), "src/**/*.rs");

    entry.selector.glob = None;
    assert_eq!(entry.path_or_glob(), "");
}
