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
fn rejects_evidence_entry_with_surrounding_whitespace() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "padded-evidence"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = [" doc:docs/safety.md "]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(
        err.contains(
            "padded-evidence evidence entry 1 must not have leading or trailing whitespace"
        )
    );
}

#[test]
fn rejects_duplicate_evidence_entries() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "duplicate-evidence"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["doc:docs/safety.md", "doc:docs/safety.md"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(err.contains("duplicate-evidence duplicate evidence entry"));
    assert!(err.contains("position 2"));
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
fn rejects_link_entry_with_surrounding_whitespace() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "padded-link"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                links = [" pr:123 "]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(err.contains("padded-link link entry 1 must not have leading or trailing whitespace"));
}

#[test]
fn rejects_duplicate_link_entries() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "duplicate-link"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                links = ["pr:123", "pr:123"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(err.contains("duplicate-link duplicate link entry"));
    assert!(err.contains("position 2"));
}
