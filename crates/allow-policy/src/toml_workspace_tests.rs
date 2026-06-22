use super::WorkspaceToml;
use allow_core::WorkspaceConfig;

#[test]
fn into_workspace_config_applies_explicit_root_and_default_mode() {
    let workspace = WorkspaceToml {
        root: Some("fixtures/policy".to_string()),
        inventory: None,
        default_mode: Some("strict".to_string()),
        ignored: Vec::new(),
        generated: Vec::new(),
    };
    let default = WorkspaceConfig::default();

    let cfg = workspace.into_workspace_config().unwrap();

    assert_eq!(cfg.root, "fixtures/policy");
    assert_eq!(cfg.default_mode, "strict");
    assert_eq!(cfg.inventory, default.inventory);
}

#[test]
fn into_workspace_config_normalizes_git_tracked_inventory_alias() {
    let workspace = WorkspaceToml {
        root: None,
        inventory: Some("git_tracked".to_string()),
        default_mode: None,
        ignored: Vec::new(),
        generated: Vec::new(),
    };

    let cfg = workspace.into_workspace_config().unwrap();

    assert_eq!(cfg.inventory, "git-tracked");
}

#[test]
fn into_workspace_config_preserves_default_ignored_and_generated_globs() {
    let workspace = WorkspaceToml {
        root: None,
        inventory: None,
        default_mode: None,
        ignored: Vec::new(),
        generated: Vec::new(),
    };
    let default = WorkspaceConfig::default();

    let cfg = workspace.into_workspace_config().unwrap();

    assert_eq!(cfg.ignored, default.ignored);
    assert_eq!(cfg.generated, default.generated);
}

#[test]
fn into_workspace_config_preserves_custom_ignored_and_generated_globs() {
    let workspace = WorkspaceToml {
        root: None,
        inventory: None,
        default_mode: None,
        ignored: vec!["custom/ignored/**".to_string()],
        generated: vec!["custom/generated/**".to_string()],
    };

    let cfg = workspace.into_workspace_config().unwrap();

    assert_eq!(cfg.ignored, vec!["custom/ignored/**".to_string()]);
    assert_eq!(cfg.generated, vec!["custom/generated/**".to_string()]);
}
