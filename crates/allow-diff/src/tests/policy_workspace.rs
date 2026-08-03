use super::*;
use allow_core::FileFamilyRule;

#[test]
fn detects_added_workspace_ignored_scope_as_failure() {
    let base = config_with(entry("allow-1"));
    let mut head = base.clone();
    head.workspace.ignored.push("src/**".to_string());

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::WorkspaceIgnoredAdded
            && change.severity == PolicyChangeSeverity::Fail
            && change.allow_id == "workspace.ignored"
            && change.message.contains("src/**")
    }));
    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::WorkspaceIgnoredAdded)
        .unwrap_or_else(|| std::panic::panic_any("ignored scope addition should be reported"));
    assert_eq!(
        change.scope.as_ref().map(|scope| (
            scope.field,
            scope.before.as_deref(),
            scope.after.as_deref()
        )),
        Some((ScopeChangeField::Effective, None, Some("src/**")))
    );
}

#[test]
fn detects_removed_workspace_ignored_scope_as_improvement() {
    let mut base = config_with(entry("allow-1"));
    base.workspace.ignored.push("ignored/**".to_string());
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::WorkspaceIgnoredRemoved
            && change.severity == PolicyChangeSeverity::Improvement
            && change.allow_id == "workspace.ignored"
            && change.message.contains("ignored/**")
    }));
    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::WorkspaceIgnoredRemoved)
        .unwrap_or_else(|| std::panic::panic_any("ignored scope removal should be reported"));
    assert_eq!(
        change.scope.as_ref().map(|scope| (
            scope.field,
            scope.before.as_deref(),
            scope.after.as_deref()
        )),
        Some((ScopeChangeField::Effective, Some("ignored/**"), None))
    );
}

#[test]
fn detects_workspace_generated_scope_changes() {
    let base = config_with(entry("allow-1"));
    let mut head = base.clone();
    head.workspace.generated.push("schemas/**".to_string());

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::WorkspaceGeneratedAdded
            && change.severity == PolicyChangeSeverity::Review
            && change.allow_id == "workspace.generated"
            && change.message.contains("schemas/**")
    }));
    let generated_added = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::WorkspaceGeneratedAdded)
        .unwrap_or_else(|| std::panic::panic_any("generated scope addition should be reported"));
    assert_eq!(
        generated_added.scope.as_ref().map(|scope| (
            scope.field,
            scope.before.as_deref(),
            scope.after.as_deref()
        )),
        Some((ScopeChangeField::Effective, None, Some("schemas/**")))
    );

    let changes = policy_changes(&head, &base);
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::WorkspaceGeneratedRemoved
            && change.severity == PolicyChangeSeverity::Improvement
            && change.allow_id == "workspace.generated"
            && change.message.contains("schemas/**")
    }));
    let generated_removed = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::WorkspaceGeneratedRemoved)
        .unwrap_or_else(|| std::panic::panic_any("generated scope removal should be reported"));
    assert_eq!(
        generated_removed.scope.as_ref().map(|scope| (
            scope.field,
            scope.before.as_deref(),
            scope.after.as_deref()
        )),
        Some((ScopeChangeField::Effective, Some("schemas/**"), None))
    );
}

#[test]
fn workspace_scope_changes_normalize_windows_separators() {
    let base = config_with(entry("allow-1"));
    let mut head = base.clone();
    head.workspace.ignored.push(r"src\generated\**".to_string());

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::WorkspaceIgnoredAdded
            && change.message.contains("src/generated/**")
    }));
    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::WorkspaceIgnoredAdded)
        .unwrap_or_else(|| std::panic::panic_any("ignored scope addition should be reported"));
    assert_eq!(
        change
            .scope
            .as_ref()
            .and_then(|scope| scope.after.as_deref()),
        Some("src/generated/**")
    );
}

#[test]
fn detects_file_family_rule_addition_and_removal_with_stable_signatures() {
    let base = config_with(entry("allow-1"));
    let mut head = base.clone();
    head.workspace.file_families.push(FileFamilyRule {
        id: "model-artifact".to_string(),
        family: "ml_model".to_string(),
        glob: r"models\**\*.onnx".to_string(),
        reason: "Govern tracked model artifacts.".to_string(),
    });

    let changes = policy_changes(&base, &head);
    let added = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::FamilyRuleAdded)
        .unwrap_or_else(|| std::panic::panic_any("family rule addition should be reported"));
    assert_eq!(added.allow_id, "workspace.file_family.model-artifact");
    assert_eq!(added.severity, PolicyChangeSeverity::Review);
    assert_eq!(
        added
            .scope
            .as_ref()
            .and_then(|scope| scope.after.as_deref()),
        Some("family=ml_model; glob=models/**/*.onnx; reason=Govern tracked model artifacts.")
    );

    let removed_changes = policy_changes(&head, &base);
    let removed = removed_changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::FamilyRuleRemoved)
        .unwrap_or_else(|| std::panic::panic_any("family rule removal should be reported"));
    assert_eq!(removed.allow_id, "workspace.file_family.model-artifact");
    assert_eq!(removed.severity, PolicyChangeSeverity::Review);
    assert_eq!(
        removed
            .scope
            .as_ref()
            .and_then(|scope| scope.before.as_deref()),
        Some("family=ml_model; glob=models/**/*.onnx; reason=Govern tracked model artifacts.")
    );
}
