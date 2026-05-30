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
fn rejects_blank_policy_schema_version() {
    let err = parse_err(
        r#"
                schema_version = "   "
                policy = "cargo-allow"
            "#,
    );

    assert!(err.contains("policy schema_version must not be empty"));
}

#[test]
fn rejects_unsupported_policy_schema_version() {
    let err = parse_err(
        r#"
                schema_version = "99.0"
                policy = "cargo-allow"
            "#,
    );

    assert!(err.contains("unsupported policy schema_version `99.0`"));
}

#[test]
fn rejects_blank_policy_owner() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                owner = ""
            "#,
    );

    assert!(err.contains("policy owner must not be empty"));
}

#[test]
fn rejects_blank_policy_status() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                status = "   "
            "#,
    );

    assert!(err.contains("policy status must not be empty"));
}

#[test]
fn accepts_advisory_policy_status() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                status = "advisory"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy should parse: {err}")));

    assert_eq!(cfg.status.as_deref(), Some("advisory"));
}

#[test]
fn rejects_unsupported_policy_status() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                status = "paused"
            "#,
    );

    assert!(err.contains("unsupported policy status `paused`"));
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
fn rejects_blank_evidence_entry() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "blank-evidence"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["   "]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(err.contains("blank-evidence evidence entry 1 must not be empty"));
}

#[test]
fn rejects_blank_link_entry() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "blank-link"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                links = [""]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(err.contains("blank-link link entry 1 must not be empty"));
}

#[test]
fn rejects_empty_workspace_root() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [workspace]
                root = "   "
            "#,
    );

    assert!(err.contains("workspace root has empty path"));
}

#[test]
fn rejects_parent_workspace_root() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [workspace]
                root = "../outside"
            "#,
    );

    assert!(err.contains("workspace root path must not contain parent directory segments"));
}

#[test]
fn rejects_unsupported_workspace_inventory() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [workspace]
                inventory = "filesystem"
            "#,
    );

    assert!(err.contains("unsupported workspace inventory `filesystem`"));
}

#[test]
fn accepts_artifact_style_git_tracked_workspace_inventory_alias() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [workspace]
                inventory = "git_tracked"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy should parse: {err}")));

    assert_eq!(cfg.workspace.inventory, "git-tracked");
}

#[test]
fn rejects_unsupported_workspace_default_mode() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [workspace]
                default_mode = "permissive"
            "#,
    );

    assert!(err.contains("unsupported workspace default_mode `permissive`"));
}

#[test]
fn rejects_invalid_workspace_ignored_glob() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [workspace]
                ignored = ["../target/**"]
            "#,
    );

    assert!(err.contains("source-tree ignored glob must not contain parent directory segments"));
}

#[test]
fn rejects_invalid_workspace_generated_glob() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [workspace]
                generated = ["/generated/**"]
            "#,
    );

    assert!(err.contains("source-tree generated glob must be source-tree-relative"));
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
fn accepts_allow_id_token_characters() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow_test-1"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy should parse: {err}")));

    assert_eq!(cfg.allow[0].id, "allow_test-1");
}

#[test]
fn rejects_allow_id_with_surrounding_whitespace() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = " allow-1 "
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(err.contains("allow id ` allow-1 ` must not have leading or trailing whitespace"));
}

#[test]
fn rejects_allow_id_with_unsupported_characters() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow:1"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(err.contains(
        "allow id `allow:1` may contain only ASCII letters, digits, hyphen, or underscore"
    ));
}

#[test]
fn rejects_blank_allow_family() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "blank-family"
                kind = "panic"
                family = "   "
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(err.contains("blank-family family must not be empty"));
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
fn rejects_expiry_date_before_created() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "bad-expiry-order"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                created = "2026-08-01"
                expires = "2026-07-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(err.contains("expires must not be before created"));
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
fn rejects_unsupported_brace_glob_syntax() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [workspace]
                ignored = ["scripts/{a,b}.sh"]
            "#,
    );

    assert!(err.contains("unsupported glob token `{`"));
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
