use super::*;

#[test]
fn rejects_missing_general_evidence_when_required() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"

                [requirements]
                evidence_required = true

                [[allow]]
                id = "allow-panic"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(err.contains("allow-panic missing evidence"));
}

#[test]
fn keeps_unsafe_evidence_requirement_specific() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "allow-unsafe"
                kind = "unsafe"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "unsafe_block"
                container = "load"
            "#,
    );

    assert!(err.contains("allow-unsafe unsafe entry missing evidence"));
}

#[test]
fn rejects_duplicate_ids() {
    let err = parse_policy(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "x"
                kind = "non_rust_file"
                path = "a.py"
                owner = "o"
                classification = "c"
                reason = "r"
                expires = "2026-08-01"
                [allow.selector]
                glob = "a.py"
                [[allow]]
                id = "x"
                kind = "non_rust_file"
                path = "b.py"
                owner = "o"
                classification = "c"
                reason = "r"
                expires = "2026-08-01"
                [allow.selector]
                glob = "b.py"
            "#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("duplicate"));
}

#[test]
fn rejects_invalid_lifecycle_dates() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "bad-date"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-02-31"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(err.contains("invalid expires date"));
}

#[test]
fn rejects_zero_occurrence_limit() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-zero"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "baseline_debt"
                reason = "Generated baseline debt."
                occurrence_limit = 0
                created = "2026-05-26"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(
        err.to_string()
            .contains("occurrence_limit must be greater than zero")
    );
}

#[test]
fn rejects_lifecycle_dates_before_created() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "bad-order"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                created = "2026-08-01"
                review_after = "2026-07-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(err.contains("review_after must not be before created"));
}

#[test]
fn rejects_never_expiry_without_review_after_when_lifecycle_required() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"

                [requirements]
                expires_or_review_after_required = true

                [[allow]]
                id = "never-without-review"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "never"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(err.contains("missing expires or review_after"));
}

#[test]
fn accepts_never_expiry_with_review_after_when_lifecycle_required() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"

                [requirements]
                expires_or_review_after_required = true

                [[allow]]
                id = "never-with-review"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                review_after = "2026-08-01"
                expires = "never"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy should parse: {err}")));

    assert_eq!(
        cfg.allow[0].lifecycle.review_after.as_deref(),
        Some("2026-08-01")
    );
    assert_eq!(cfg.allow[0].lifecycle.expires.as_deref(), Some("never"));
}

#[test]
fn rejects_invalid_glob_scope() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "bad-glob"
                kind = "non_rust_file"
                glob = "../scripts/**"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                glob = "../scripts/**"
            "#,
    );

    assert!(err.contains("parent directory"));
}

#[test]
fn rejects_line_only_selector() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "line-only"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                line_hint = 12
            "#,
    );

    assert!(err.contains("selector must include structural identity"));
}

#[test]
fn rejects_zero_selector_line_hint() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "zero-line-hint"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
                line_hint = 0
            "#,
    );

    assert!(err.contains("line_hint must be greater than zero"));
}

#[test]
fn rejects_zero_last_seen_coordinates() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "zero-last-seen"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
                [allow.last_seen]
                line = 0
                column = 1
            "#,
    );

    assert!(err.contains("last_seen line must be greater than zero"));
}

#[test]
fn rejects_zero_last_seen_column() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "zero-last-seen-column"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
                [allow.last_seen]
                line = 1
                column = 0
            "#,
    );

    assert!(err.contains("last_seen column must be greater than zero"));
}

#[test]
fn rejects_last_seen_with_line_only() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "line-only-last-seen"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
                [allow.last_seen]
                line = 12
            "#,
    );

    assert!(err.contains("last_seen must include both line and column"));
}

#[test]
fn rejects_last_seen_with_column_only() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "column-only-last-seen"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
                [allow.last_seen]
                column = 4
            "#,
    );

    assert!(err.contains("last_seen must include both line and column"));
}

#[test]
fn rejects_baseline_debt_without_short_expiry() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "baseline-too-long"
                kind = "panic"
                path = "src/lib.rs"
                owner = "unowned"
                classification = "baseline_debt"
                reason = "Generated by cargo-allow propose; requires human review."
                created = "2026-05-26"
                expires = "2027-05-26"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(err.contains("baseline_debt expires must be within"));
}

fn parse_err(input: &str) -> String {
    match parse_policy(input) {
        Ok(_) => std::panic::panic_any("expected policy parse failure"),
        Err(err) => err.to_string(),
    }
}
