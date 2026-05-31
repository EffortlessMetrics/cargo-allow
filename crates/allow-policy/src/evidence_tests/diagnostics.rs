use std::fs;
use std::path::PathBuf;

use allow_core::{AllowConfig, AllowEntry, FindingKind, Lifecycle, Selector};

use super::{remove_test_dir, unique_test_dir};
use crate::{
    EvidenceReferenceStatus, broken_evidence_link_count, evidence_reference_diagnostics,
    validate_local_evidence_references, weak_evidence_reference_count,
};

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
            "legacy-policy:no-panic-baseline".to_string(),
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
fn diagnostics_classify_empty_traceability_evidence_as_weak() {
    let root = unique_test_dir("empty-traceability-evidence");
    let entry = AllowEntry {
        id: "allow-empty-traceability".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "reviewed".to_string(),
        reason: "fixture".to_string(),
        evidence: vec!["test:".to_string(), "issue:   ".to_string()],
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
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry.clone());

    let diagnostics = evidence_reference_diagnostics(&root, &entry);

    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.status)
            .collect::<Vec<_>>(),
        vec![
            EvidenceReferenceStatus::Unstructured,
            EvidenceReferenceStatus::Unstructured
        ]
    );
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic
            .message
            .contains("empty evidence reference target")
    }));
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.target.is_none())
    );
    assert_eq!(weak_evidence_reference_count(&root, &cfg), 2);
    validate_local_evidence_references(&root, &cfg).unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "weak traceability evidence remains advisory: {err}"
        ))
    });
    remove_test_dir(root);
}

#[test]
fn diagnostics_classify_unknown_prefix_evidence_as_weak() {
    let root = unique_test_dir("unknown-prefix-evidence");
    let entry = AllowEntry {
        id: "allow-unknown-prefix".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "reviewed".to_string(),
        reason: "fixture".to_string(),
        evidence: vec!["ticket:evidence/parser-123".to_string()],
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
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry.clone());

    let diagnostics = evidence_reference_diagnostics(&root, &entry);

    assert_eq!(
        diagnostics.first().map(|diagnostic| diagnostic.status),
        Some(EvidenceReferenceStatus::Unstructured)
    );
    assert_eq!(
        diagnostics
            .first()
            .and_then(|diagnostic| diagnostic.prefix.as_deref()),
        Some("ticket")
    );
    assert_eq!(
        diagnostics
            .first()
            .and_then(|diagnostic| diagnostic.target.as_ref()),
        Some(&PathBuf::from("evidence/parser-123"))
    );
    assert!(
        diagnostics.first().is_some_and(|diagnostic| {
            diagnostic.message.contains("unrecognized evidence prefix")
        })
    );
    assert_eq!(weak_evidence_reference_count(&root, &cfg), 1);
    validate_local_evidence_references(&root, &cfg).unwrap_or_else(|err| {
        std::panic::panic_any(format!("unknown-prefix evidence remains advisory: {err}"))
    });
    remove_test_dir(root);
}

#[test]
fn evidence_status_identifies_broken_local_links() {
    assert!(!EvidenceReferenceStatus::LocalFilePresent.is_broken_local_link());
    assert!(EvidenceReferenceStatus::LocalFileMissing.is_broken_local_link());
    assert!(EvidenceReferenceStatus::InvalidLocalPath.is_broken_local_link());
    assert!(!EvidenceReferenceStatus::TraceabilityOnly.is_broken_local_link());
    assert!(!EvidenceReferenceStatus::Unstructured.is_broken_local_link());
}

#[test]
fn evidence_status_identifies_weak_references() {
    assert!(!EvidenceReferenceStatus::LocalFilePresent.is_weak_reference());
    assert!(!EvidenceReferenceStatus::LocalFileMissing.is_weak_reference());
    assert!(!EvidenceReferenceStatus::InvalidLocalPath.is_weak_reference());
    assert!(!EvidenceReferenceStatus::TraceabilityOnly.is_weak_reference());
    assert!(EvidenceReferenceStatus::Unstructured.is_weak_reference());
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

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        symlink(root.join("docs/safety.md"), root.join("docs/link.md"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create symlink: {err}")));
        entry.evidence = vec!["doc:docs/link.md".to_string()];
        let diagnostics = evidence_reference_diagnostics(&root, &entry);
        assert_eq!(
            diagnostics.first().map(|diagnostic| diagnostic.status),
            Some(EvidenceReferenceStatus::InvalidLocalPath)
        );
        assert!(
            diagnostics
                .first()
                .is_some_and(|diagnostic| diagnostic.message.contains("symlink"))
        );

        symlink(root.join("docs"), root.join("docs-link"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create symlinked dir: {err}")));
        entry.evidence = vec!["doc:docs-link/safety.md".to_string()];
        let diagnostics = evidence_reference_diagnostics(&root, &entry);
        assert_eq!(
            diagnostics.first().map(|diagnostic| diagnostic.status),
            Some(EvidenceReferenceStatus::InvalidLocalPath)
        );
        assert!(
            diagnostics
                .first()
                .is_some_and(|diagnostic| diagnostic.message.contains("symlink component"))
        );
    }
    remove_test_dir(root);
}

#[test]
fn counts_missing_and_invalid_local_evidence_links() {
    let root = unique_test_dir("evidence-broken-count");
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
    fs::write(root.join("docs/present.md"), "review notes")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence: {err}")));
    let entry = AllowEntry {
        id: "allow-doc".to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "reviewed".to_string(),
        reason: "fixture".to_string(),
        evidence: vec![
            "doc:docs/present.md".to_string(),
            "spec:docs/missing.md".to_string(),
            "adr:../outside.md".to_string(),
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
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    assert_eq!(broken_evidence_link_count(&root, &cfg), 2);
    assert_eq!(weak_evidence_reference_count(&root, &cfg), 1);
    remove_test_dir(root);
}
