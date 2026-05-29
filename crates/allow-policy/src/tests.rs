use super::*;

#[test]
fn parses_policy_with_allow() {
    let cfg = parse_policy(
        r#"
                schema_version = "0.1"
                policy = "cargo-allow"

                [requirements]
                expires_or_review_after_required = true
                lint_policy_id_required = true

                [[allow]]
                id = "allow-0001"
                kind = "panic"
                family = "unwrap"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"

                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
                container = "load"
            "#,
    )
    .expect("policy parses");
    assert_eq!(cfg.allow.len(), 1);
    assert!(cfg.requirements.lint_policy_id_required);
    assert_eq!(cfg.allow[0].selector.callee.as_deref(), Some("unwrap"));
}

#[test]
fn parses_unsafe_safety_comment_requirement() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"

                [requirements.unsafe]
                safety_comment_required = true

                [[allow]]
                id = "allow-unsafe"
                kind = "unsafe"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["test:unsafe_boundary"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "unsafe_block"
                container = "load"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy should parse: {err}")));

    assert!(cfg.requirements.unsafe_safety_comment_required);
}

#[test]
fn parses_general_evidence_requirement() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"

                [requirements]
                evidence_required = true
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy should parse: {err}")));

    assert!(cfg.requirements.evidence_required);
}

#[test]
fn parses_legacy_aliases_and_scalar_arrays() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"

                [workspace]
                ignored = ".git/**"

                [requirements]
                owner_required = "true"

                [[allow]]
                id = "allow-legacy"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "legacy"
                explanation = "legacy reason field"
                covered_by = "test:legacy"
                count = 2
                expires = "2026-08-01"

                [allow.selector]
                kind = "macro_call"
                macro = "panic"
                line_hint = "12"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("legacy aliases parse: {err}")));

    assert_eq!(cfg.workspace.ignored, vec![".git/**"]);
    let entry = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one allow entry"));
    assert_eq!(entry.reason, "legacy reason field");
    assert_eq!(entry.evidence, vec!["test:legacy"]);
    assert_eq!(entry.occurrence_limit, Some(2));
    assert_eq!(entry.selector.ast_kind.as_deref(), Some("macro_call"));
    assert_eq!(entry.selector.macro_name.as_deref(), Some("panic"));
    assert_eq!(entry.selector.line_hint, Some(12));
}

#[test]
fn reports_toml_parse_errors() {
    let err = parse_policy("policy = [").unwrap_err();

    assert!(err.to_string().contains("failed to parse policy TOML"));
}

#[test]
fn parses_current_repository_policy() {
    let cfg = parse_policy(include_str!("../../../policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("repo policy parses: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert!(cfg.allow.iter().any(|entry| entry.id == "allow-0076"));
    assert!(cfg.allow.iter().any(|entry| entry.id == "allow-0088"));
    for removed in [
        "allow-0001",
        "allow-0002",
        "allow-0003",
        "allow-0004",
        "allow-0005",
        "allow-0006",
        "allow-0007",
        "allow-0008",
        "allow-0009",
        "allow-0011",
        "allow-0012",
        "allow-0013",
        "allow-0014",
        "allow-0015",
        "allow-0016",
        "allow-0017",
        "allow-0018",
        "allow-0019",
        "allow-0020",
        "allow-0031",
        "allow-0032",
        "allow-0033",
        "allow-0039",
        "allow-0041",
        "allow-0042",
        "allow-0043",
        "allow-0044",
        "allow-0045",
        "allow-0046",
        "allow-0047",
        "allow-0048",
        "allow-0049",
        "allow-0050",
        "allow-0051",
        "allow-0052",
        "allow-0053",
        "allow-0054",
        "allow-0055",
        "allow-0056",
        "allow-0057",
        "allow-0058",
        "allow-0059",
        "allow-0060",
        "allow-0061",
        "allow-0062",
        "allow-0063",
        "allow-0064",
        "allow-0065",
        "allow-0066",
    ] {
        assert!(
            !cfg.allow.iter().any(|entry| entry.id == removed),
            "{removed} should stay pruned from the repository policy"
        );
    }
}
