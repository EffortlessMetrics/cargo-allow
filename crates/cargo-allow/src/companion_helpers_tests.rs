use super::*;
use allow_core::{
    AllowConfig, AllowEntry, Finding, FindingKind, Lifecycle, Selector, Span, StructuralIdentity,
};
use std::path::PathBuf;

fn allow_entry(kind: FindingKind, family: Option<&str>) -> AllowEntry {
    AllowEntry {
        id: "fixture".to_string(),
        kind,
        family: family.map(str::to_string),
        path: None,
        glob: None,
        owner: String::new(),
        classification: String::new(),
        reason: String::new(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector::default(),
        last_seen: None,
    }
}

fn policy_family_allow(family: &str) -> AllowConfig {
    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(allow_entry(FindingKind::PolicyException, Some(family)));
    cfg
}

fn generated_code_allow() -> AllowConfig {
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(allow_entry(
        FindingKind::GeneratedCode,
        Some("generated_code"),
    ));
    cfg
}

fn test_finding(kind: FindingKind, family: Option<&str>, path: &str, ast_kind: &str) -> Finding {
    Finding {
        kind,
        family: family.map(str::to_string),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity: StructuralIdentity::new("file", ast_kind),
        message: "test finding".to_string(),
        ledger: None,
    }
}

#[test]
fn has_policy_family_returns_true_when_config_includes_family() {
    let cfg = policy_family_allow("github_workflow");

    assert!(has_policy_family(
        &cfg,
        &["github_workflow", "workflow_external_action"]
    ));
    assert!(!has_policy_family(
        &cfg,
        &["process_spawn", "network_destination"]
    ));
}

#[test]
fn has_allow_family_matches_kind_and_family() {
    let cfg = generated_code_allow();

    assert!(has_allow_family(
        &cfg,
        FindingKind::GeneratedCode,
        "generated_code"
    ));
    assert!(!has_allow_family(
        &cfg,
        FindingKind::PolicyException,
        "generated_code"
    ));
    assert!(!has_allow_family(
        &cfg,
        FindingKind::GeneratedCode,
        "dependency_surface"
    ));
}

#[test]
fn same_finding_identity_compares_finding_identity_keys() {
    let left = test_finding(
        FindingKind::GeneratedCode,
        Some("generated_code"),
        "generated/schema.json",
        "tracked_file",
    );
    let matching = left.clone();
    let distinct = test_finding(
        FindingKind::GeneratedCode,
        Some("generated_code"),
        "generated/other.json",
        "tracked_file",
    );

    assert_eq!(
        allow_core::finding_identity_key(&left),
        allow_core::finding_identity_key(&matching)
    );
    assert_ne!(
        allow_core::finding_identity_key(&left),
        allow_core::finding_identity_key(&distinct)
    );
}
