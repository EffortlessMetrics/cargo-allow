use super::*;
use allow_core::{AllowEntry, FindingKind, Lifecycle, Selector};

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
fn validates_all_local_evidence_reference_prefixes() {
    let root = unique_test_dir("evidence-local-prefixes");
    for path in [
        "docs/rationale.md",
        "docs/specs/parser.md",
        "docs/adr/0001.md",
        "target/ripr/parser.json",
        "target/coverage/parser.info",
        "docs/evidence/unsafe-review/ffi.json",
        "docs/evidence/unsafe_review/ffi.json",
    ] {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap_or_else(|| {
            std::panic::panic_any(format!("evidence path has no parent: {}", path.display()))
        }))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("create evidence parent {}: {err}", path.display()))
        });
        fs::write(&path, "{}").unwrap_or_else(|err| {
            std::panic::panic_any(format!("write evidence {}: {err}", path.display()))
        });
    }
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-local-evidence"
                kind = "unsafe"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = [
                  "doc:docs/rationale.md",
                  "spec:docs/specs/parser.md",
                  "adr:docs/adr/0001.md",
                  "ripr:target/ripr/parser.json",
                  "coverage:target/coverage/parser.info",
                  "unsafe-review:docs/evidence/unsafe-review/ffi.json",
                  "unsafe_review:docs/evidence/unsafe_review/ffi.json",
                ]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "unsafe_block"
                container = "load"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

    validate_local_evidence_references(&root, &cfg).unwrap_or_else(|err| {
        std::panic::panic_any(format!("local evidence prefixes validate: {err}"))
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
fn diagnostics_classify_traceability_evidence_without_local_validation() {
    let root = unique_test_dir("evidence-traceability-prefixes");
    let entry = AllowEntry {
        id: "allow-traceability".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "reviewed".to_string(),
        reason: "fixture".to_string(),
        evidence: vec![
            "test:parser_rejects_bad_range".to_string(),
            "cargo:cargo test -p parser".to_string(),
            "issue:123".to_string(),
            "pr:456".to_string(),
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
            EvidenceReferenceStatus::TraceabilityOnly,
            EvidenceReferenceStatus::TraceabilityOnly,
            EvidenceReferenceStatus::TraceabilityOnly,
            EvidenceReferenceStatus::TraceabilityOnly
        ]
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.message.contains("not executed"))
    );
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
