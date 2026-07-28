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
fn rejects_untyped_general_evidence_when_required() {
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
                evidence = ["manual review note"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(err.contains(
        "allow-panic evidence_required entries require at least one typed evidence reference"
    ));
}

#[test]
fn accepts_typed_general_evidence_when_required() {
    let cfg = parse_policy(
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
                evidence = ["test:panic_path_is_covered"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("typed evidence should parse: {err}")));

    assert_eq!(cfg.allow[0].id, "allow-panic");
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
fn rejects_reviewed_unsafe_entry_with_only_weak_evidence() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "allow-unsafe-weak"
                kind = "unsafe"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["TODO: add unsafe-review evidence"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "unsafe_block"
                container = "load"
            "#,
    );

    assert!(err.contains("allow-unsafe-weak unsafe entry requires at least one typed evidence"));
}

#[test]
fn accepts_reviewed_unsafe_entry_with_typed_evidence() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "allow-unsafe-typed"
                kind = "unsafe"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["test:load_rejects_null"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "unsafe_block"
                container = "load"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy should parse: {err}")));

    assert_eq!(cfg.allow[0].id, "allow-unsafe-typed");
}

#[test]
fn reportable_evidence_mode_keeps_invalid_local_links_for_diagnostics() {
    let input = r#"
                policy = "cargo-allow"

                [[allow]]
                id = "allow-invalid-link"
                kind = "unsafe"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["test:load_rejects_null"]
                links = ["doc:docs/./safety.md"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "unsafe_block"
                container = "load"
            "#;

    let strict_err = parse_err(input);
    assert!(strict_err.contains(
        "allow-invalid-link link entry 1 path must not contain current directory segments"
    ));

    let cfg = parse_policy_with_reportable_evidence(input)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy should parse: {err}")));
    let entry = match cfg.allow.first() {
        Some(entry) => entry,
        None => std::panic::panic_any("policy should contain allow entry"),
    };
    let diagnostics = policy_reference_diagnostics(".", entry);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.source == EvidenceReferenceSource::Link
                && diagnostic.diagnostic.status == EvidenceReferenceStatus::InvalidLocalPath
                && diagnostic
                    .diagnostic
                    .message
                    .contains("current directory segments")
        }),
        "report-only policy loading should keep invalid local links diagnosable: {diagnostics:?}"
    );
}

#[test]
fn accepts_unsafe_baseline_debt_with_weak_evidence_placeholder() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "allow-unsafe-baseline"
                kind = "unsafe"
                path = "src/lib.rs"
                owner = "unowned"
                classification = "baseline_debt"
                reason = "Generated by cargo-allow propose; requires human review."
                evidence = ["TODO: add unsafe-review evidence"]
                created = "2026-05-26"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "unsafe_block"
                container = "load"
            "#,
    )
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "baseline debt may keep uncomfortable placeholder evidence: {err}"
        ))
    });

    assert_eq!(cfg.allow[0].classification, "baseline_debt");
}

#[test]
fn rejects_reviewed_process_policy_entry_with_only_weak_evidence() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "allow-process-weak"
                kind = "policy_exception"
                family = "process_spawn"
                path = ".github/workflows/ci.yml"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["binary:cargo"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "process_spawn"
                symbol = "cargo install cargo-deny --locked"
            "#,
    );

    assert!(err.contains(
        "allow-process-weak policy_exception.process_spawn entry requires at least one typed evidence"
    ));
}

#[test]
fn rejects_reviewed_network_policy_entry_without_typed_evidence() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "allow-network-weak"
                kind = "policy_exception"
                family = "network_destination"
                path = "policy/network-allowlist.toml"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["destination:crates.io"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "network_destination"
                symbol = "crates.io lane build"
            "#,
    );

    assert!(err.contains(
        "allow-network-weak policy_exception.network_destination entry requires at least one typed evidence"
    ));
}

#[test]
fn accepts_reviewed_high_risk_policy_exception_with_typed_evidence() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "allow-process-typed"
                kind = "policy_exception"
                family = "process_spawn"
                path = ".github/workflows/ci.yml"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["legacy-policy:proc-cargo-install-cargo-deny", "binary:cargo"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "process_spawn"
                symbol = "cargo install cargo-deny --locked"
            "#,
    )
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "typed process policy exception should parse: {err}"
        ))
    });

    assert_eq!(cfg.allow[0].id, "allow-process-typed");
}

#[test]
fn accepts_high_risk_policy_baseline_debt_with_weak_evidence_placeholder() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "allow-network-baseline"
                kind = "policy_exception"
                family = "network_destination"
                path = "policy/network-allowlist.toml"
                owner = "unowned"
                classification = "baseline_debt"
                reason = "Generated by cargo-allow propose; requires human review."
                evidence = ["TODO: add network boundary evidence"]
                created = "2026-05-26"
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "network_destination"
                symbol = "crates.io lane build"
            "#,
    )
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "baseline debt may keep uncomfortable process/network placeholder evidence: {err}"
        ))
    });

    assert_eq!(cfg.allow[0].classification, "baseline_debt");
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

#[test]
fn accepts_source_tree_relative_local_link() {
    let policy = parse_policy(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "relative-local-link"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                links = ["doc:docs/safety.md"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(
        policy.is_ok(),
        "source-tree-relative local links should parse"
    );
}

#[test]
fn accepts_source_tree_relative_local_evidence() {
    let policy = parse_policy(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "relative-local-evidence"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["doc:docs/safety.md"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(
        policy.is_ok(),
        "source-tree-relative local evidence should parse"
    );
}

#[test]
fn rejects_local_link_with_parent_directory_segment() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "parent-local-link"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                links = ["doc:../outside.md"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(err.contains(
        "parent-local-link link entry 1 path must not contain parent directory segments"
    ));
}

#[test]
fn rejects_local_evidence_with_parent_directory_segment() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "parent-local-evidence"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["doc:../outside.md"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(err.contains(
        "parent-local-evidence evidence entry 1 path must not contain parent directory segments"
    ));
}

#[test]
fn rejects_local_link_with_absolute_path() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "absolute-local-link"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                links = ["spec:/docs/safety.md"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(err.contains("absolute-local-link link entry 1 path must be source-tree-relative"));
}

#[test]
fn rejects_local_evidence_with_absolute_path() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "absolute-local-evidence"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["spec:/docs/safety.md"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(
        err.contains("absolute-local-evidence evidence entry 1 path must be source-tree-relative")
    );
}

#[test]
fn rejects_local_link_with_wildcard_path() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "wildcard-local-link"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                links = ["adr:docs/*.md"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    );

    assert!(err.contains("wildcard-local-link link entry 1 path uses wildcard token `*`"));
}
