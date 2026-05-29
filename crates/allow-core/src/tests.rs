use super::*;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

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
    for scope in [
        "scripts/*.sh",
        "scripts/?.sh",
        "scripts/[ab].sh",
        "scripts/{a,b}.sh",
    ] {
        assert!(source_tree_scope_has_wildcard(scope));
    }
    assert!(!source_tree_scope_has_wildcard("scripts/release.sh"));
}

#[test]
fn finding_kind_accepts_hyphenated_cli_aliases() {
    assert_eq!(
        FindingKind::from_str("non-rust"),
        Ok(FindingKind::NonRustFile)
    );
    assert_eq!(
        FindingKind::from_str("lint-exception"),
        Ok(FindingKind::LintException)
    );
    assert_eq!(
        FindingKind::from_str("generated-code"),
        Ok(FindingKind::GeneratedCode)
    );
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
fn hash_is_stable() {
    assert_eq!(stable_hash_hex("abc"), stable_hash_hex("abc"));
    assert_ne!(stable_hash_hex("abc"), stable_hash_hex("abd"));
}

#[test]
fn line_distance_score_uses_documented_buckets() {
    assert_eq!(maybe_line_distance_score(Some(10), Some(10)), 15);
    assert_eq!(maybe_line_distance_score(Some(10), Some(13)), 12);
    assert_eq!(maybe_line_distance_score(Some(10), Some(20)), 8);
    assert_eq!(maybe_line_distance_score(Some(10), Some(35)), 3);
    assert_eq!(maybe_line_distance_score(Some(10), Some(36)), 0);
    assert_eq!(maybe_line_distance_score(None, Some(10)), 0);
    assert_eq!(maybe_line_distance_score(Some(10), None), 0);
}

#[test]
fn structural_identity_key_excludes_line_and_column_hints() {
    let mut first = StructuralIdentity::new("rust", "method_call");
    first.module = Some("parser::span".to_string());
    first.container = Some("parse_span".to_string());
    first.callee = Some("unwrap".to_string());
    first.normalized_snippet_hash = Some("fnv1a64:1234".to_string());
    first.line_hint = Some(12);
    first.column_hint = Some(8);

    let mut moved = first.clone();
    moved.line_hint = Some(99);
    moved.column_hint = Some(42);

    assert_eq!(first.stable_key(), moved.stable_key());

    moved.container = Some("parse_other_span".to_string());

    assert_ne!(first.stable_key(), moved.stable_key());
}

#[test]
fn json_escape_covers_quotes_backslashes_whitespace_and_control_chars() {
    assert_eq!(
        json_escape("quote: \" slash: \\ newline:\n tab:\t return:\r bell:\u{0007}"),
        "quote: \\\" slash: \\\\ newline:\\n tab:\\t return:\\r bell:\\u0007"
    );
}

#[test]
fn normalize_snippet_collapses_mixed_whitespace() {
    assert_eq!(
        normalize_snippet("  let\tvalue =\nitems [ index ];\r\n"),
        "let value = items [ index ];"
    );
}

#[test]
fn maybe_line_distance_score_covers_boundary_bands() {
    assert_eq!(maybe_line_distance_score(Some(10), Some(10)), 15);
    assert_eq!(maybe_line_distance_score(Some(10), Some(13)), 12);
    assert_eq!(maybe_line_distance_score(Some(10), Some(20)), 8);
    assert_eq!(maybe_line_distance_score(Some(10), Some(35)), 3);
    assert_eq!(maybe_line_distance_score(Some(10), Some(36)), 0);
    assert_eq!(maybe_line_distance_score(None, Some(10)), 0);
    assert_eq!(maybe_line_distance_score(Some(10), None), 0);
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

    entry.glob = None;
    assert_eq!(entry.path_or_glob(), "src/**/*.rs");

    entry.selector.glob = None;
    assert_eq!(entry.path_or_glob(), "");
}

#[test]
fn allow_config_empty_sets_document_defaults() {
    let config = AllowConfig::empty();

    assert_eq!(config.schema_version, "0.1");
    assert_eq!(config.policy, "cargo-allow");
    assert_eq!(config.owner, None);
    assert_eq!(config.status.as_deref(), Some("active"));
    assert_eq!(config.workspace, WorkspaceConfig::default());
    assert_eq!(config.requirements, Requirements::default());
    assert!(config.allow.is_empty());
}

#[test]
fn match_status_strings_and_failure_modes_cover_all_statuses() {
    let cases = [
        (MatchStatus::Matched, "matched", false, false),
        (MatchStatus::New, "new", true, true),
        (MatchStatus::Stale, "stale", true, false),
        (MatchStatus::Expired, "expired", true, true),
        (MatchStatus::ReviewDue, "review_due", false, false),
        (MatchStatus::Ambiguous, "ambiguous", true, true),
        (MatchStatus::InvalidSelector, "invalid_selector", true, true),
        (
            MatchStatus::MissingRequiredField,
            "missing_required_field",
            true,
            true,
        ),
        (MatchStatus::EvidenceMissing, "evidence_missing", true, true),
        (MatchStatus::BaselineDebt, "baseline_debt", true, false),
    ];

    for (status, expected_name, strict_failure, no_new_failure) in cases {
        assert_eq!(status.as_str(), expected_name);
        assert_eq!(status.is_failure_in_strict(), strict_failure, "{status:?}");
        assert_eq!(status.is_failure_in_no_new(), no_new_failure, "{status:?}");
    }
}

#[test]
fn structural_identity_has_v1_schema_id() {
    assert_eq!(
        StructuralIdentity::schema_id(),
        "cargo-allow.structural-identity.v1"
    );
}

#[test]
fn structural_identity_key_uses_length_prefixed_parts() {
    let mut first = StructuralIdentity::new("rust", "method_call");
    first.container = Some("load".to_string());
    first.callee = Some("unwrap".to_string());
    first.normalized_snippet_hash = Some("fnv1a64:abcd".to_string());

    let key = first.stable_key();

    assert!(key.contains("language:4:rust"));
    assert!(key.contains("container:4:load"));
    assert!(key.contains("callee:6:unwrap"));
    assert!(key.contains("normalized_snippet_hash:12:fnv1a64:abcd"));
}

#[test]
fn structural_identity_v1_fields_affect_stable_key_except_hints() {
    let mut base = StructuralIdentity::new("rust", "method_call");
    base.crate_name = Some("parser".to_string());
    base.module = Some("parser::span".to_string());
    base.container = Some("slice_checked_text_range".to_string());
    base.symbol = Some("source[range]".to_string());
    base.callee = Some("expect".to_string());
    base.macro_name = Some("panic".to_string());
    base.lint = Some("clippy::indexing_slicing".to_string());
    base.receiver_fingerprint = Some("source".to_string());
    base.target_fingerprint = Some("range".to_string());
    base.normalized_snippet_hash = Some("fnv1a64:abcd".to_string());
    base.line_hint = Some(10);
    base.column_hint = Some(4);

    let mut moved = base.clone();
    moved.line_hint = Some(100);
    moved.column_hint = Some(40);
    assert_eq!(base.stable_key(), moved.stable_key());

    let cases: &[fn(&mut StructuralIdentity)] = &[
        |id| id.language = "file".to_string(),
        |id| id.crate_name = Some("runtime".to_string()),
        |id| id.module = Some("runtime::ffi".to_string()),
        |id| id.container = Some("read_buffer".to_string()),
        |id| id.ast_kind = "macro_call".to_string(),
        |id| id.symbol = Some("buffer[index]".to_string()),
        |id| id.callee = Some("unwrap".to_string()),
        |id| id.macro_name = Some("todo".to_string()),
        |id| id.lint = Some("dead_code".to_string()),
        |id| id.receiver_fingerprint = Some("buffer".to_string()),
        |id| id.target_fingerprint = Some("index".to_string()),
        |id| id.normalized_snippet_hash = Some("fnv1a64:dcba".to_string()),
    ];

    for mutate in cases {
        let mut changed = base.clone();
        mutate(&mut changed);
        assert_ne!(base.stable_key(), changed.stable_key());
    }
}

#[test]
fn finding_identity_key_excludes_span_but_includes_structural_scope() {
    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.container = Some("load".to_string());
    identity.callee = Some("unwrap".to_string());
    identity.normalized_snippet_hash = Some("fnv1a64:abcd".to_string());

    let mut first = Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: Some(Span {
            line: 10,
            column: 4,
        }),
        identity,
        message: "test finding".to_string(),
    };
    let mut moved = first.clone();
    moved.span = Some(Span {
        line: 200,
        column: 40,
    });

    assert_eq!(finding_identity_key(&first), finding_identity_key(&moved));

    moved.path = PathBuf::from("src/other.rs");
    assert_ne!(finding_identity_key(&first), finding_identity_key(&moved));

    moved.path = first.path.clone();
    first.family = Some("expect".to_string());
    assert_ne!(finding_identity_key(&first), finding_identity_key(&moved));
}

#[test]
fn finding_source_package_name_trims_source_derived_crate_name() {
    let mut finding = Finding {
        kind: FindingKind::Panic,
        family: None,
        path: PathBuf::from("src/lib.rs"),
        span: None,
        identity: StructuralIdentity::new("rust", "method_call"),
        message: "test finding".to_string(),
    };

    assert_eq!(finding.source_package_name(), None);

    finding.identity.crate_name = Some("  allow-core  ".to_string());
    assert_eq!(finding.source_package_name(), Some("allow-core"));

    finding.identity.crate_name = Some("   ".to_string());
    assert_eq!(finding.source_package_name(), None);
}

#[test]
fn simple_date_rejects_invalid_calendar_dates() {
    assert!(SimpleDate::parse("2026-02-29").is_none());
    assert!(SimpleDate::parse("2024-02-29").is_some());
    assert!(SimpleDate::parse("2026-04-31").is_none());
    assert!(SimpleDate::parse("2026-13-01").is_none());
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
