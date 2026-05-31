use std::fs;
use std::path::PathBuf;

use allow_core::{AllowConfig, AllowEntry, FindingKind, Lifecycle, Selector};

use super::{remove_test_dir, unique_test_dir};
use crate::{
    EvidenceReferenceStatus, broken_evidence_link_count, evidence_reference_diagnostics,
    weak_evidence_reference_count,
};

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
