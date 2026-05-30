use super::*;

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
fn rejects_unsupported_bracket_glob_syntax() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "bracket-glob"
                kind = "non_rust_file"
                glob = "scripts/[ab].sh"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                glob = "scripts/[ab].sh"
            "#,
    );

    assert!(err.contains("unsupported glob token `[`"));
}

#[test]
fn rejects_non_whole_segment_double_star_glob_syntax() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "embedded-globstar"
                kind = "non_rust_file"
                glob = "scripts/**.sh"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                glob = "scripts/**.sh"
            "#,
    );

    assert!(err.contains("unsupported glob token `**`"));
    assert!(err.contains("whole source-tree path segment"));
}

#[test]
fn rejects_wildcard_tokens_in_exact_path_scope() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "wildcard-path"
                kind = "non_rust_file"
                path = "scripts/*.sh"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                glob = "scripts/*.sh"
            "#,
    );

    assert!(err.contains("wildcard-path path uses wildcard token `*`"));
    assert!(err.contains("use `glob`"));
}

#[test]
fn accepts_path_with_matching_selector_glob() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "matching-selector-glob"
                kind = "non_rust_file"
                path = "docs/policy.md"
                owner = "core"
                classification = "documentation"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                glob = "docs/policy.md"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy should parse: {err}")));

    assert_eq!(cfg.allow[0].path_or_glob(), "docs/policy.md");
}

#[test]
fn accepts_path_with_slash_equivalent_selector_glob() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "matching-selector-glob"
                kind = "non_rust_file"
                path = "docs/policy.md"
                owner = "core"
                classification = "documentation"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                glob = "docs\\policy.md"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy should parse: {err}")));

    assert_eq!(cfg.allow[0].path_or_glob(), "docs/policy.md");
}

#[test]
fn accepts_glob_with_slash_equivalent_selector_glob() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "matching-selector-glob"
                kind = "non_rust_file"
                glob = "docs\\**"
                owner = "core"
                classification = "documentation"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                glob = "docs/**"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy should parse: {err}")));

    assert_eq!(cfg.allow[0].path_or_glob(), "docs/**");
}

#[test]
fn rejects_entry_with_path_and_top_level_glob() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "path-and-glob"
                kind = "non_rust_file"
                path = "scripts/release.sh"
                glob = "scripts/**"
                owner = "core"
                classification = "release_script"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "tracked_file"
            "#,
    );

    assert!(err.contains("path-and-glob must not define both path and glob"));
}

#[test]
fn rejects_path_with_mismatched_selector_glob() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "path-selector-mismatch"
                kind = "non_rust_file"
                path = "docs/policy.md"
                owner = "core"
                classification = "documentation"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                glob = "docs/**"
            "#,
    );

    assert!(err.contains(
        "path-selector-mismatch selector glob `docs/**` must match path `docs/policy.md`"
    ));
}

#[test]
fn rejects_glob_with_mismatched_selector_glob() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "glob-selector-mismatch"
                kind = "non_rust_file"
                glob = "docs/**"
                owner = "core"
                classification = "documentation"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                glob = "scripts/**"
            "#,
    );

    assert!(
        err.contains("glob-selector-mismatch selector glob `scripts/**` must match glob `docs/**`")
    );
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
fn rejects_source_code_scope_only_selector() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "scope-only-source"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                glob = "src/lib.rs"
            "#,
    );

    assert!(
        err.contains("scope-only-source source-code selector must include structural identity")
    );
}

#[test]
fn rejects_empty_selector_identity_field() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "empty-selector-field"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = ""
            "#,
    );

    assert!(err.contains("selector ast_kind must not be empty"));
}

#[test]
fn rejects_blank_selector_identity_field_even_with_other_identity() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "blank-selector-field"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                normalized_snippet_hash = "   "
            "#,
    );

    assert!(err.contains("selector normalized_snippet_hash must not be empty"));
}

#[test]
fn rejects_selector_identity_field_with_surrounding_whitespace() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "padded-selector"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = " unwrap "
            "#,
    );

    assert!(
        err.contains(
            "padded-selector selector callee must not have leading or trailing whitespace"
        )
    );
}

#[test]
fn rejects_padded_snippet_hash_selector() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "padded-snippet-hash"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                normalized_snippet_hash = " fnv1a64:abc "
            "#,
    );

    assert!(err.contains(
        "padded-snippet-hash selector normalized_snippet_hash must not have leading or trailing whitespace"
    ));
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
