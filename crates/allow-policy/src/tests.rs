use super::*;
use allow_core::{AllowEntry, FindingKind, Lifecycle, Selector};

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
fn validates_existing_local_evidence_references() {
    let root = unique_test_dir("evidence-existing");
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
    fs::write(root.join("docs/safety.md"), "review notes")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence: {err}")));
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-doc"
                kind = "unsafe"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["doc:docs/safety.md", "test:safety_fixture"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "unsafe_block"
                container = "load"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

    validate_local_evidence_references(&root, &cfg)
        .unwrap_or_else(|err| std::panic::panic_any(format!("evidence validates: {err}")));
    remove_test_dir(root);
}

#[test]
fn validates_existing_unsafe_review_evidence_references() {
    let root = unique_test_dir("unsafe-review-evidence-existing");
    fs::create_dir_all(root.join("docs/evidence/unsafe-review")).unwrap_or_else(|err| {
        std::panic::panic_any(format!("create unsafe-review evidence dir: {err}"))
    });
    fs::write(
        root.join("docs/evidence/unsafe-review/ffi-read-buffer.json"),
        "{}",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write unsafe-review evidence: {err}")));
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-unsafe-review"
                kind = "unsafe"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["unsafe-review:docs/evidence/unsafe-review/ffi-read-buffer.json"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "unsafe_block"
                container = "load"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

    validate_local_evidence_references(&root, &cfg).unwrap_or_else(|err| {
        std::panic::panic_any(format!("unsafe-review evidence validates: {err}"))
    });
    remove_test_dir(root);
}

#[test]
fn rejects_missing_local_evidence_references() {
    let root = unique_test_dir("evidence-missing");
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create root: {err}")));
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-doc"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["doc:docs/missing.md"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

    let err = validate_local_evidence_references(&root, &cfg).unwrap_err();
    assert!(err.to_string().contains("allow-doc evidence"));
    assert!(err.to_string().contains("missing local file"));
    remove_test_dir(root);
}

#[test]
fn rejects_directory_local_evidence_references() {
    let root = unique_test_dir("evidence-directory");
    fs::create_dir_all(root.join("docs/safety"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create evidence dir: {err}")));
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-doc-dir"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["doc:docs/safety"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

    let err = validate_local_evidence_references(&root, &cfg).unwrap_err();
    assert!(err.to_string().contains("allow-doc-dir evidence"));
    assert!(err.to_string().contains("not a directory"));
    remove_test_dir(root);
}

#[test]
fn rejects_missing_unsafe_review_evidence_references() {
    let root = unique_test_dir("unsafe-review-evidence-missing");
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create root: {err}")));
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-unsafe-review"
                kind = "unsafe"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["unsafe-review:docs/evidence/unsafe-review/missing.json"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "unsafe_block"
                container = "load"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

    let err = validate_local_evidence_references(&root, &cfg).unwrap_err();
    assert!(err.to_string().contains("allow-unsafe-review evidence"));
    assert!(err.to_string().contains("missing local file"));
    remove_test_dir(root);
}

#[test]
fn rejects_escaping_local_evidence_references() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-doc"
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
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

    let err = validate_local_evidence_references(".", &cfg).unwrap_err();
    assert!(
        err.to_string()
            .contains("must not contain parent directory segments")
    );
}

#[test]
fn reports_evidence_reference_diagnostics() {
    let root = unique_test_dir("evidence-diagnostics");
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
    fs::write(root.join("docs/safety.md"), "review notes")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence: {err}")));
    let mut entry = AllowEntry {
        id: "allow-doc".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "reviewed".to_string(),
        reason: "fixture".to_string(),
        evidence: vec![
            "doc:docs/safety.md".to_string(),
            "spec:docs/missing.md".to_string(),
            "test:parser_rejects_bad_range".to_string(),
            "TODO: add reviewed evidence".to_string(),
        ],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: None,
            review_after: None,
            expires: Some("2026-08-01".to_string()),
        },
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            callee: Some("unwrap".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    };

    let diagnostics = evidence_reference_diagnostics(&root, &entry);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.status)
            .collect::<Vec<_>>(),
        vec![
            EvidenceReferenceStatus::LocalFilePresent,
            EvidenceReferenceStatus::LocalFileMissing,
            EvidenceReferenceStatus::TraceabilityOnly,
            EvidenceReferenceStatus::Unstructured
        ]
    );

    entry.evidence = vec!["doc:../outside.md".to_string()];
    let diagnostics = evidence_reference_diagnostics(&root, &entry);
    assert_eq!(
        diagnostics.first().map(|diagnostic| diagnostic.status),
        Some(EvidenceReferenceStatus::InvalidLocalPath)
    );

    entry.evidence = vec!["doc:docs".to_string()];
    let diagnostics = evidence_reference_diagnostics(&root, &entry);
    assert_eq!(
        diagnostics.first().map(|diagnostic| diagnostic.status),
        Some(EvidenceReferenceStatus::InvalidLocalPath)
    );
    assert!(
        diagnostics
            .first()
            .is_some_and(|diagnostic| diagnostic.message.contains("not a file"))
    );
    remove_test_dir(root);
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
    assert!(cfg.allow.iter().any(|entry| entry.id == "allow-0001"));
    assert!(cfg.allow.iter().any(|entry| entry.id == "allow-0088"));
    for removed in [
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
        "allow-0054",
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
fn renders_and_parses_occurrence_limit() {
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(AllowEntry {
        id: "allow-counted".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "baseline_debt".to_string(),
        reason: "Generated baseline debt.".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: Some(3),
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: None,
            expires: Some("2026-08-01".to_string()),
        },
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            callee: Some("unwrap".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    });

    let rendered = render_policy(&cfg);
    assert!(rendered.contains("occurrence_limit = 3"));
    let reparsed = parse_policy(&rendered)
        .unwrap_or_else(|err| std::panic::panic_any(format!("rendered policy parses: {err}")));
    assert_eq!(
        reparsed
            .allow
            .first()
            .and_then(|entry| entry.occurrence_limit),
        Some(3)
    );
}

#[test]
fn renders_and_parses_general_evidence_requirement() {
    let mut cfg = AllowConfig::empty();
    cfg.requirements.evidence_required = true;

    let rendered = render_policy(&cfg);
    assert!(rendered.contains("evidence_required = true"));
    let reparsed = parse_policy(&rendered)
        .unwrap_or_else(|err| std::panic::panic_any(format!("rendered policy parses: {err}")));
    assert!(reparsed.requirements.evidence_required);
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

fn unique_test_dir(slug: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("cargo-allow-policy-{slug}-{}", std::process::id()));
    remove_test_dir(path.clone());
    path
}

fn remove_test_dir(path: PathBuf) {
    match fs::remove_dir_all(&path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => std::panic::panic_any(format!(
            "failed to remove test dir {}: {err}",
            path.display()
        )),
    }
}

fn parse_err(input: &str) -> String {
    match parse_policy(input) {
        Ok(_) => std::panic::panic_any("expected policy parse failure"),
        Err(err) => err.to_string(),
    }
}
