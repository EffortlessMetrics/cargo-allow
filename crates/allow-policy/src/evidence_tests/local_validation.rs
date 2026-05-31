use std::{fs, path::PathBuf};

use super::{remove_test_dir, unique_test_dir};
use crate::{
    EvidenceReferenceStatus, evidence_reference_diagnostics, parse_policy,
    validate_local_evidence_references,
};

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
fn validates_backslash_local_evidence_references_portably() {
    let root = unique_test_dir("evidence-backslash");
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
    fs::write(root.join("docs/safety.md"), "review notes")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence: {err}")));
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-doc-backslash"
                kind = "unsafe"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["doc:docs\\safety.md"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "unsafe_block"
                container = "load"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

    validate_local_evidence_references(&root, &cfg).unwrap_or_else(|err| {
        std::panic::panic_any(format!("backslash evidence validates portably: {err}"))
    });
    let Some(entry) = cfg.allow.first() else {
        std::panic::panic_any("policy should contain fixture allow entry");
    };
    let diagnostics = evidence_reference_diagnostics(&root, entry);
    assert_eq!(
        diagnostics.first().map(|diagnostic| diagnostic.status),
        Some(EvidenceReferenceStatus::LocalFilePresent)
    );
    assert_eq!(
        diagnostics
            .first()
            .and_then(|diagnostic| diagnostic.target.as_ref()),
        Some(&PathBuf::from("docs/safety.md"))
    );
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
