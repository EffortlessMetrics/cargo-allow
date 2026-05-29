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

use proptest::prelude::*;

fn path_segment_strategy() -> impl Strategy<Value = String> {
    "[A-Za-z0-9_-]{1,12}".prop_map(|segment| segment)
}

fn path_with_dot_segments_strategy() -> impl Strategy<Value = String> {
    (
        proptest::collection::vec(
            prop_oneof![
                Just(".".to_string()),
                Just("..".to_string()),
                path_segment_strategy(),
            ],
            0..12,
        ),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(segments, absolute, backslash)| {
            let separator = if backslash { "\\" } else { "/" };
            let mut path = segments.join(separator);
            if absolute {
                path = format!("/{path}");
            }
            path
        })
}

fn concrete_path_segments_strategy() -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec(path_segment_strategy(), 1..6)
}

proptest! {
    #[test]
    fn normalize_path_is_idempotent(path in path_with_dot_segments_strategy()) {
        let normalized = normalize_path(&path);

        prop_assert_eq!(normalize_path(&normalized), normalized);
    }

    #[test]
    fn normalize_path_removes_reducible_dot_segments(path in path_with_dot_segments_strategy()) {
        let normalized = normalize_path(&path);
        let body = normalized.strip_prefix('/').unwrap_or(&normalized);
        let mut saw_non_parent_segment = false;

        for segment in body.split('/').filter(|segment| !segment.is_empty()) {
            prop_assert_ne!(segment, ".");
            if segment == ".." {
                prop_assert!(!normalized.starts_with('/'));
                prop_assert!(!saw_non_parent_segment);
            } else {
                saw_non_parent_segment = true;
            }
        }
    }

    #[test]
    fn normalize_path_treats_forward_and_backward_slashes_equivalently(
        segments in proptest::collection::vec(
            prop_oneof![
                Just(".".to_string()),
                Just("..".to_string()),
                path_segment_strategy(),
            ],
            0..12,
        ),
        absolute in any::<bool>(),
    ) {
        let slash_path = format!("{}{}", if absolute { "/" } else { "" }, segments.join("/"));
        let backslash_path = format!("{}{}", if absolute { "/" } else { "" }, segments.join("\\"));

        prop_assert_eq!(normalize_path(slash_path), normalize_path(backslash_path));
    }

    #[test]
    fn source_tree_path_filter_matches_generated_subtrees(
        filter_segments in concrete_path_segments_strategy(),
        child_segments in proptest::collection::vec(path_segment_strategy(), 0..5),
    ) {
        let filter = filter_segments.join("/");
        let item = if child_segments.is_empty() {
            filter.clone()
        } else {
            format!("{filter}/{}", child_segments.join("/"))
        };

        let non_matching_filter = format!("{filter}x");

        prop_assert!(source_tree_path_matches_filter(&item, &filter));
        prop_assert!(!source_tree_path_matches_filter(&item, &non_matching_filter));
    }

    #[test]
    fn double_star_globs_match_any_number_of_complete_path_segments(
        prefix_segments in concrete_path_segments_strategy(),
        middle_segments in proptest::collection::vec(path_segment_strategy(), 0..5),
        filename in path_segment_strategy(),
    ) {
        let prefix = prefix_segments.join("/");
        let pattern = format!("{prefix}/**/*.rs");
        let path = if middle_segments.is_empty() {
            format!("{prefix}/{filename}.rs")
        } else {
            format!("{prefix}/{}/{filename}.rs", middle_segments.join("/"))
        };

        prop_assert!(glob_matches_str(&pattern, &path));
        prop_assert!(!glob_matches_str(&pattern, &path.replace(".rs", ".txt")));
    }

    #[test]
    fn simple_date_epoch_day_conversion_round_trips(days in -200_000_i64..200_000_i64) {
        let date = SimpleDate::from_days_since_unix_epoch(days);

        prop_assert_eq!(date.days_since_unix_epoch(), days);
        prop_assert_eq!(SimpleDate::parse(&date.to_string()), Some(date));
    }

    #[test]
    fn simple_date_add_days_is_reversible(
        start_days in -200_000_i64..200_000_i64,
        offset in -10_000_i64..10_000_i64,
    ) {
        let start = SimpleDate::from_days_since_unix_epoch(start_days);
        let shifted = start.add_days(offset);

        prop_assert_eq!(start.days_until(shifted), offset);
        prop_assert_eq!(shifted.add_days(-offset), start);
    }

    #[test]
    fn structural_identity_stable_keys_remain_unambiguous_for_delimiter_text(
        left in ".{0,24}",
        right in ".{0,24}",
    ) {
        prop_assume!(left != right);

        let mut first = StructuralIdentity::new(format!("a|b:{left}"), "method_call");
        first.container = Some(right.clone());

        let mut second = StructuralIdentity::new(format!("a|b:{right}"), "method_call");
        second.container = Some(left.clone());

        prop_assert_ne!(first.stable_key(), second.stable_key());
    }
}
