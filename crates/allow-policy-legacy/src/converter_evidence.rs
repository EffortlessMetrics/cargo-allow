use allow_core::normalize_path;
use std::path::Path;

use crate::types::{
    LegacyDependencySurfaceRule, LegacyExecutableRule, LegacyGeneratedRule, LegacyNetworkRule,
    LegacyProcessRule, LegacyUnsafeRule, LegacyWorkflowRule,
};

pub(crate) fn generated_evidence(rule: &LegacyGeneratedRule) -> Vec<String> {
    let mut evidence = Vec::new();
    if let Some(generator) = &rule.generator {
        evidence.push(format!("generator:{generator}"));
    }
    if let Some(command) = &rule.regenerate_command {
        evidence.push(format!("cargo:{command}"));
    }
    evidence
}

pub(crate) fn unsafe_evidence(rule: &LegacyUnsafeRule) -> Vec<String> {
    if rule.evidence.is_empty() {
        vec!["TODO: add unsafe-review or boundary-test evidence".to_string()]
    } else {
        rule.evidence.clone()
    }
}

pub(crate) fn executable_evidence(rule: &LegacyExecutableRule) -> Vec<String> {
    rule.interpreter
        .as_ref()
        .map(|interpreter| vec![format!("interpreter:{interpreter}")])
        .unwrap_or_default()
}

pub(crate) fn workflow_evidence(rule: &LegacyWorkflowRule) -> Vec<String> {
    let mut evidence = Vec::new();
    evidence.extend(
        rule.permissions
            .iter()
            .map(|permission| format!("permission:{permission}")),
    );
    evidence.extend(
        rule.secrets_used
            .iter()
            .map(|secret| format!("secret:{secret}")),
    );
    if let Some(lane) = &rule.duplicate_of_lane {
        evidence.push(format!("duplicate_of_lane:{lane}"));
    }
    evidence
}

pub(crate) fn dependency_surface_evidence(rule: &LegacyDependencySurfaceRule) -> Vec<String> {
    let mut evidence = vec![format!("surface:{}", rule.surface)];
    if let Some(count) = rule.dep_count_at_baseline {
        evidence.push(format!("dep_count_at_baseline:{count}"));
    }
    evidence
}

pub(crate) fn process_evidence(rule: &LegacyProcessRule) -> Vec<String> {
    let mut evidence = vec![
        format!("binary:{}", rule.binary),
        format!("argv_shape:{}", rule.argv_shape.join(" ")),
        format!("network_reach:{}", rule.network_reach),
    ];
    evidence.extend(
        rule.called_by
            .iter()
            .map(|path| format!("called_by:{}", normalize_path(Path::new(path)))),
    );
    evidence
}

pub(crate) fn network_evidence(rule: &LegacyNetworkRule) -> Vec<String> {
    let mut evidence = vec![
        format!("destination:{}", rule.destination),
        format!("lane:{}", rule.lane),
        format!("auth_required:{}", rule.auth_required),
    ];
    if let Some(secret) = &rule.auth_secret {
        evidence.push(format!("auth_secret:{secret}"));
    }
    evidence
}
