use super::{validate_policy, validate_policy_with_reportable_evidence};
use allow_core::{
    AllowConfig, AllowEntry, CargoAllowErrorKind, FindingKind, Lifecycle, Requirements, Selector,
};
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

fn valid_policy() -> AllowConfig {
    let mut cfg = AllowConfig::empty();
    cfg.requirements = relaxed_requirements();
    cfg.allow.push(valid_entry("allow-1"));
    cfg
}

#[test]
fn validate_policy_returns_ok_for_valid_policy() {
    let cfg = valid_policy();

    assert!(validate_policy(&cfg).is_ok());
}

#[test]
fn validate_policy_rejects_unsupported_schema_version() {
    let mut cfg = valid_policy();
    cfg.schema_version = "9.9".to_string();

    let err = match validate_policy(&cfg) {
        Err(err) => err,
        Ok(()) => std::panic::panic_any("unsupported schema_version should fail policy validation"),
    };

    assert_eq!(err.kind(), CargoAllowErrorKind::InvalidPolicy);
    let message = err.to_string();
    assert!(message.contains("unsupported policy schema_version `9.9`"));
}

#[test]
fn validate_policy_rejects_duplicate_allow_ids() {
    let mut cfg = valid_policy();
    cfg.allow.push(valid_entry("allow-1"));

    let err = match validate_policy(&cfg) {
        Err(err) => err,
        Ok(()) => std::panic::panic_any("duplicate allow ids should fail policy validation"),
    };

    assert_eq!(err.kind(), CargoAllowErrorKind::InvalidPolicy);
    let message = err.to_string();
    assert!(message.contains("duplicate allow id `allow-1`"));
}

#[test]
fn validate_policy_with_reportable_evidence_accepts_invalid_local_links() {
    let mut cfg = valid_policy();
    let mut entry = valid_entry("invalid-local-link");
    entry.links = vec!["doc:docs/../safety.md".to_string()];
    cfg.allow = vec![entry];

    assert!(validate_policy_with_reportable_evidence(&cfg).is_ok());
}

#[test]
fn validate_policy_aggregates_stage_diagnostics() {
    let mut cfg = valid_policy();
    cfg.schema_version = "9.9".to_string();
    cfg.allow.push(valid_entry("allow-1"));

    let error = validate_policy(&cfg).expect_err("invalid header and duplicate id should fail");
    let diagnostics = error.diagnostics();

    assert!(diagnostics.len() >= 2);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.category == "policy_validation")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.field.as_deref() == Some("header"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.entry_id.as_deref() == Some("allow-1"))
    );
}
