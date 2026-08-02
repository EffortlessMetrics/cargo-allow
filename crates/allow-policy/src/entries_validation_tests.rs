use super::*;
use allow_core::{FindingKind, Lifecycle, Selector};
use std::path::PathBuf;

fn valid_entry(id: &str) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "repo-infra".to_string(),
        classification: "reviewed".to_string(),
        reason: "fixture reason".to_string(),
        evidence: vec!["test:panic_path_is_covered".to_string()],
        links: vec!["issue:123".to_string()],
        occurrence_limit: Some(1),
        lifecycle: Lifecycle {
            expires: Some("2026-08-01".to_string()),
            ..Lifecycle::empty()
        },
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            callee: Some("unwrap".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn relaxed_requirements() -> Requirements {
    Requirements {
        expires_or_review_after_required: false,
        ..Requirements::default()
    }
}

#[test]
fn validate_allow_entries_returns_ok_for_valid_entry_batch() {
    let entries = vec![valid_entry("allow-1"), valid_entry("allow-2")];
    let requirements = relaxed_requirements();

    let result = validate_allow_entries(&entries, &requirements);

    assert!(result.is_ok());
}

#[test]
fn validate_allow_entries_rejects_duplicate_ids_through_strict_orchestration() {
    let entries = vec![valid_entry("duplicate-id"), valid_entry("duplicate-id")];
    let requirements = relaxed_requirements();

    let err = match validate_allow_entries(&entries, &requirements) {
        Err(err) => err,
        Ok(()) => std::panic::panic_any("duplicate allow ids should fail strict validation"),
    };

    let message = err.to_string();
    assert!(message.contains("duplicate allow id `duplicate-id`"));
}

#[test]
fn validate_allow_entries_strict_rejects_invalid_local_links() {
    let mut entry = valid_entry("invalid-local-link");
    entry.links = vec!["doc:docs/../safety.md".to_string()];
    let requirements = relaxed_requirements();

    let err = match validate_allow_entries(&[entry], &requirements) {
        Err(err) => err,
        Ok(()) => std::panic::panic_any("strict validation should reject parent-segment links"),
    };

    let message = err.to_string();
    assert!(message.contains("invalid-local-link"));
    assert!(message.contains("parent directory segments"));
}

#[test]
fn validate_allow_entries_with_reportable_evidence_accepts_invalid_local_links() {
    let mut entry = valid_entry("invalid-local-link");
    entry.links = vec!["doc:docs/../safety.md".to_string()];
    let requirements = relaxed_requirements();

    let result = validate_allow_entries_with_reportable_evidence(&[entry], &requirements);

    assert!(result.is_ok());
}

#[test]
fn validate_allow_entries_preserves_structured_diagnostics_for_each_failure() {
    let mut invalid = valid_entry("same-id");
    invalid.path = None;
    invalid.reason.clear();
    let requirements = relaxed_requirements();

    let error = validate_allow_entries(&[valid_entry("same-id"), invalid], &requirements)
        .expect_err("duplicate and malformed entry should fail validation");

    let diagnostics = error.diagnostics();
    assert!(diagnostics.len() >= 3);
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.category == "policy_validation"
            && diagnostic.entry_id.as_deref() == Some("same-id")
    }));
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.field.as_deref() == Some("identity"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.field.as_deref() == Some("scope"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.field.as_deref() == Some("requirements"))
    );
}
