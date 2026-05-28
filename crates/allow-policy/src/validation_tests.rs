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
