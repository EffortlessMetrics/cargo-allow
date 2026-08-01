use super::*;
use allow_core::{AllowConfig, AllowEntry, FindingKind, Lifecycle, Selector};
use std::path::Path;

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

fn generated_code_allow() -> AllowConfig {
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(allow_entry(
        FindingKind::GeneratedCode,
        Some("generated_code"),
    ));
    cfg
}

fn policy_family_allow(family: &str) -> AllowConfig {
    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(allow_entry(FindingKind::PolicyException, Some(family)));
    cfg
}

#[test]
fn has_generated_code_receipt_returns_true_for_generated_code_allow() {
    let cfg = generated_code_allow();
    let expected = true;

    assert_eq!(has_generated_code_receipt(&cfg), expected);
}

#[test]
fn has_generated_code_receipt_returns_false_without_generated_code_allow() {
    let cfg = AllowConfig::empty();
    let expected = false;

    assert_eq!(has_generated_code_receipt(&cfg), expected);
}

#[test]
fn has_policy_family_returns_true_when_config_includes_family() {
    let cfg = policy_family_allow("process_spawn");

    assert!(has_policy_family(&cfg, &["process_spawn"]));
    assert!(!has_policy_family(
        &cfg,
        &["github_workflow", "workflow_external_action"]
    ));
}

#[test]
fn is_workflow_path_accepts_github_workflow_yaml_paths() {
    assert!(is_workflow_path(Path::new(".github/workflows/ci.yml")));
    assert!(is_workflow_path(Path::new(".github/workflows/ci.yaml")));
    assert!(!is_workflow_path(Path::new(".github/workflows/ci.txt")));
    assert!(!is_workflow_path(Path::new("docs/workflows/ci.yml")));
}

#[test]
fn missing_revision_source_reports_inventory_error() {
    let err = missing_revision_source(Path::new("src/lib.rs"));

    assert_eq!(err.kind(), allow_core::CargoAllowErrorKind::Inventory);
    assert!(err.to_string().contains("src/lib.rs"));
}
