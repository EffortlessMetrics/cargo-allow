use allow_core::{Finding, FindingKind, normalize_snippet, stable_hash_hex};
use std::path::{Path, PathBuf};

use crate::finding_builder::{FindingSite, push_finding};

#[test]
fn push_finding_populates_identity_span_message_and_enrichment() {
    let container = Some("Parser::parse".to_string());
    let modules = vec!["parser".to_string(), "rules".to_string()];
    let line = "    let value = input + 1;";
    let mut findings = Vec::new();

    push_finding(
        FindingSite {
            path: Path::new("src/lib.rs"),
            line,
            line_no: 7,
            column: 9,
            container: &container,
            module_stack: &modules,
        },
        FindingKind::LintException,
        "expect_attribute",
        "attribute",
        |identity| {
            identity.lint = Some("clippy::manual_assert".to_string());
            identity.target_fingerprint = Some("policy:allow-lint".to_string());
        },
        &mut findings,
    );

    match findings.as_slice() {
        [finding] => assert_full_finding(finding, line),
        other => assert_eq!(other.len(), 1),
    }
}

#[test]
fn push_finding_qualifies_unqualified_container_with_module_path() {
    let container = Some("access".to_string());
    let modules = vec!["inner".to_string()];
    let mut findings = Vec::new();

    push_finding(
        FindingSite {
            path: Path::new("src/lib.rs"),
            line: "    unsafe { core::ptr::read(ptr) }",
            line_no: 3,
            column: 5,
            container: &container,
            module_stack: &modules,
        },
        FindingKind::Unsafe,
        "unsafe_block",
        "unsafe_block",
        |_| {},
        &mut findings,
    );

    assert_eq!(
        findings[0].identity.container.as_deref(),
        Some("inner::access")
    );
}

#[test]
fn push_finding_leaves_optional_scope_fields_empty_without_scope_context() {
    let container = None;
    let modules = Vec::new();
    let mut findings = Vec::new();

    push_finding(
        FindingSite {
            path: Path::new("src/main.rs"),
            line: "    value + 1",
            line_no: 3,
            column: 5,
            container: &container,
            module_stack: &modules,
        },
        FindingKind::Panic,
        "indexing",
        "index_expr",
        |_| {},
        &mut findings,
    );

    match findings.as_slice() {
        [finding] => {
            assert_eq!(finding.kind, FindingKind::Panic);
            assert_eq!(finding.family.as_deref(), Some("indexing"));
            assert_eq!(finding.path, PathBuf::from("src/main.rs"));
            assert_eq!(finding.span.as_ref().map(|span| span.line), Some(3));
            assert_eq!(finding.span.as_ref().map(|span| span.column), Some(5));
            assert_eq!(finding.identity.container, None);
            assert_eq!(finding.identity.module, None);
            assert_eq!(finding.identity.language, "rust");
            assert_eq!(finding.identity.ast_kind, "index_expr");
            assert_eq!(finding.identity.line_hint, Some(3));
            assert_eq!(finding.identity.column_hint, Some(5));
            assert_eq!(finding.message, "panic indexing syntax found");
        }
        other => assert_eq!(other.len(), 1),
    }
}

fn assert_full_finding(finding: &Finding, line: &str) {
    assert_eq!(finding.kind, FindingKind::LintException);
    assert_eq!(finding.family.as_deref(), Some("expect_attribute"));
    assert_eq!(finding.path, PathBuf::from("src/lib.rs"));
    assert_eq!(finding.span.as_ref().map(|span| span.line), Some(7));
    assert_eq!(finding.span.as_ref().map(|span| span.column), Some(9));
    assert_eq!(finding.identity.language, "rust");
    assert_eq!(finding.identity.ast_kind, "attribute");
    assert_eq!(finding.identity.container.as_deref(), Some("Parser::parse"));
    assert_eq!(finding.identity.module.as_deref(), Some("parser::rules"));
    assert_eq!(finding.identity.line_hint, Some(7));
    assert_eq!(finding.identity.column_hint, Some(9));
    assert_eq!(
        finding.identity.normalized_snippet_hash.as_deref(),
        Some(stable_hash_hex(&normalize_snippet(line)).as_str())
    );
    assert_eq!(
        finding.identity.target_fingerprint.as_deref(),
        Some("policy:allow-lint")
    );
    assert_eq!(
        finding.identity.lint.as_deref(),
        Some("clippy::manual_assert")
    );
    assert_eq!(
        finding.message,
        "lint_exception expect_attribute syntax found"
    );
}
