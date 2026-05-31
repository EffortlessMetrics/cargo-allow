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
fn rejects_path_scope_with_surrounding_whitespace() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "padded-path"
                kind = "non_rust_file"
                path = " docs/policy.md "
                owner = "core"
                classification = "documentation"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                glob = " docs/policy.md "
            "#,
    );

    assert!(err.contains("padded-path path must not have leading or trailing whitespace"));
}

#[test]
fn rejects_glob_scope_with_surrounding_whitespace() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "padded-glob"
                kind = "non_rust_file"
                glob = " docs/** "
                owner = "core"
                classification = "documentation"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                glob = " docs/** "
            "#,
    );

    assert!(err.contains("padded-glob glob must not have leading or trailing whitespace"));
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
