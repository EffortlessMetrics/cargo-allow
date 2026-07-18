use super::*;

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
fn ignores_zero_selector_line_hint() {
    // line_hint is accepted in TOML for backward compatibility but no longer
    // propagated into the runtime Selector, so line_hint = 0 no longer causes a
    // validation failure. The entry should parse successfully.
    let cfg = match parse_policy(
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
    ) {
        Ok(cfg) => cfg,
        Err(err) => std::panic::panic_any(format!("line_hint = 0 should parse: {err}")),
    };

    assert_eq!(cfg.allow.len(), 1);
    assert_eq!(cfg.allow[0].selector.line_hint, None);
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
