use allow_core::{AllowConfig, AllowEntry, FindingKind, Lifecycle, Selector};
use std::path::PathBuf;

use crate::report_config;

#[test]
fn report_config_filters_allow_entries_by_kind() {
    let mut cfg = AllowConfig::empty();
    cfg.allow
        .push(test_entry("allow-file", FindingKind::NonRustFile));
    cfg.allow
        .push(test_entry("allow-panic", FindingKind::Panic));

    let filtered = report_config(&cfg, Some("non-rust"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("kind filter should parse: {err}")));

    assert_eq!(filtered.allow.len(), 1);
    assert!(
        filtered
            .allow
            .iter()
            .any(|entry| entry.id == "allow-file" && entry.kind == FindingKind::NonRustFile)
    );
}

#[test]
fn report_config_filters_executable_family() {
    let mut cfg = AllowConfig::empty();
    let mut executable = test_entry("allow-exec", FindingKind::PolicyException);
    executable.family = Some("executable_file".to_string());
    let mut other = test_entry("allow-other-policy", FindingKind::PolicyException);
    other.family = Some("workflow_permission".to_string());
    cfg.allow.push(executable);
    cfg.allow.push(other);

    let filtered = report_config(&cfg, Some("executable")).unwrap_or_else(|err| {
        std::panic::panic_any(format!("executable filter should parse: {err}"))
    });

    assert_eq!(filtered.allow.len(), 1);
    let entry = filtered
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected executable entry"));
    assert_eq!(entry.id, "allow-exec");
}

#[test]
fn report_config_filters_workflow_families() {
    let mut cfg = AllowConfig::empty();
    let mut workflow = test_entry("allow-workflow", FindingKind::PolicyException);
    workflow.family = Some("github_workflow".to_string());
    let mut action = test_entry("allow-workflow-action", FindingKind::PolicyException);
    action.family = Some("workflow_external_action".to_string());
    let mut other = test_entry("allow-other-policy", FindingKind::PolicyException);
    other.family = Some("executable_file".to_string());
    cfg.allow.push(workflow);
    cfg.allow.push(action);
    cfg.allow.push(other);

    let filtered = report_config(&cfg, Some("workflow")).unwrap_or_else(|err| {
        std::panic::panic_any(format!("workflow filter should parse: {err}"))
    });

    assert_eq!(filtered.allow.len(), 2);
    assert!(
        filtered
            .allow
            .iter()
            .any(|entry| entry.id == "allow-workflow")
    );
    assert!(
        filtered
            .allow
            .iter()
            .any(|entry| entry.id == "allow-workflow-action")
    );
}

#[test]
fn report_config_filters_dependency_surface_family() {
    let mut cfg = AllowConfig::empty();
    let mut dependency = test_entry("allow-dep", FindingKind::PolicyException);
    dependency.family = Some("dependency_surface".to_string());
    let mut other = test_entry("allow-other-policy", FindingKind::PolicyException);
    other.family = Some("workflow_external_action".to_string());
    cfg.allow.push(dependency);
    cfg.allow.push(other);

    let filtered = report_config(&cfg, Some("dependency-surface")).unwrap_or_else(|err| {
        std::panic::panic_any(format!("dependency-surface filter should parse: {err}"))
    });

    assert_eq!(filtered.allow.len(), 1);
    let entry = filtered
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected dependency entry"));
    assert_eq!(entry.id, "allow-dep");
}

#[test]
fn report_config_filters_process_family() {
    let mut cfg = AllowConfig::empty();
    let mut process = test_entry("allow-process", FindingKind::PolicyException);
    process.family = Some("process_spawn".to_string());
    let mut other = test_entry("allow-other-policy", FindingKind::PolicyException);
    other.family = Some("dependency_surface".to_string());
    cfg.allow.push(process);
    cfg.allow.push(other);

    let filtered = report_config(&cfg, Some("process"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("process filter should parse: {err}")));

    assert_eq!(filtered.allow.len(), 1);
    let entry = filtered
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected process entry"));
    assert_eq!(entry.id, "allow-process");
}

#[test]
fn report_config_filters_network_family() {
    let mut cfg = AllowConfig::empty();
    let mut network = test_entry("allow-network", FindingKind::PolicyException);
    network.family = Some("network_destination".to_string());
    let mut other = test_entry("allow-other-policy", FindingKind::PolicyException);
    other.family = Some("process_spawn".to_string());
    cfg.allow.push(network);
    cfg.allow.push(other);

    let filtered = report_config(&cfg, Some("network"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("network filter should parse: {err}")));

    assert_eq!(filtered.allow.len(), 1);
    let entry = filtered
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected network entry"));
    assert_eq!(entry.id, "allow-network");
}

fn test_entry(id: &str, kind: FindingKind) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind,
        family: None,
        path: Some(PathBuf::from("tracked.file")),
        glob: None,
        owner: "owner".to_string(),
        classification: "classification".to_string(),
        reason: "reason".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector {
            ast_kind: Some("tracked_file".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}
