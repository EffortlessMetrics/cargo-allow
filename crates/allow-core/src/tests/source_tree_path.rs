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
fn glob_match_budget_fails_closed_on_pathological_stars() {
    // Many consecutive `*` tokens against a long segment is the classic
    // exponential-backtracking shape. With the step budget the matcher must
    // return quickly (false) rather than hang.
    let pattern = format!("a{}b", "*".repeat(40));
    let path = format!("a{}c", "x".repeat(40));
    assert!(!glob_matches_str(&pattern, &path));
}

#[test]
fn glob_match_budget_still_accepts_ordinary_patterns() {
    assert!(glob_matches_str(
        "crates/**/src/*.rs",
        "crates/allow-core/src/lib.rs"
    ));
    assert!(glob_matches_str("docs/*.md", "docs/ci.md"));
    assert!(!glob_matches_str("docs/*.md", "docs/nested/ci.md"));
}

#[test]
fn source_tree_path_matches_filter_exact_equality_boundary_discriminator() {
    let item_path = "docs/policy.md";
    let exact_filter = "docs/policy.md";
    let different_filter = "docs/other.md";

    assert!(source_tree_path_matches_filter(item_path, exact_filter));
    assert!(!source_tree_path_matches_filter(
        item_path,
        different_filter
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
        "scripts/release/build.sh",
        "scripts/**/*.sh"
    ));
    assert!(source_tree_path_matches_filter("README.md", "."));
}

#[test]
fn source_tree_path_filter_matches_glob_pattern_as_filter() {
    // #2776: a glob pattern in the filter_path argument (e.g. from
    // `--path 'src/**/*.rs'`) must match against the item_path, not the
    // other way around. Previously the wildcard check was applied to the
    // item_path (which never contains wildcards), making this branch dead.
    assert!(source_tree_path_matches_filter(
        "src/allow-core/lib.rs",
        "src/**/*.rs"
    ));
    assert!(source_tree_path_matches_filter(
        "src/allow-match/scoring.rs",
        "src/**/*.rs"
    ));
    assert!(!source_tree_path_matches_filter(
        "docs/README.md",
        "src/**/*.rs"
    ));
    assert!(source_tree_path_matches_filter(
        "crates/parser/src/lib.rs",
        "crates/*/src/*.rs"
    ));
    assert!(!source_tree_path_matches_filter(
        "crates/parser/tests/main.rs",
        "crates/*/src/*.rs"
    ));
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
fn normalize_path_preserves_windows_drive_letter() {
    // #1821: a Windows drive letter is a meaningful absolute-path identity
    // component. Callers (e.g. migrate evidence diagnostics) legitimately
    // pass absolute roots through normalize_path, so the drive prefix is
    // preserved — not stripped — to avoid corrupting those identities.
    // Backslashes are still normalized to forward slashes.
    assert_eq!(
        normalize_path(r"C:\Users\proj\src\lib.rs"),
        "C:/Users/proj/src/lib.rs"
    );
    assert_eq!(
        normalize_path(r"D:/repo/crates\lib.rs"),
        "D:/repo/crates/lib.rs"
    );
    // Lowercase drive letter is preserved too.
    assert_eq!(normalize_path(r"c:\foo\bar.rs"), "c:/foo/bar.rs");
}

#[test]
fn normalize_path_strips_verbatim_prefix() {
    // #1821: the Windows verbatim prefix `\\?\` (used by
    // std::fs::canonicalize) is stripped so the path degrades to its
    // non-verbatim form. This is the case that silently produced wrong
    // identity keys because the `\\?\` prefix survived as path segments.
    // Verbatim drive-letter path: the drive letter is preserved after stripping.
    assert_eq!(
        normalize_path(r"\\?\C:\proj\src\lib.rs"),
        "C:/proj/src/lib.rs"
    );
    // Verbatim UNC: \\?\UNC\server\share\... — the verbatim prefix is
    // stripped, and the UNC root folds to a Unix-style absolute.
    assert_eq!(
        normalize_path(r"\\?\UNC\server\share\proj\lib.rs"),
        "/server/share/proj/lib.rs"
    );
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
