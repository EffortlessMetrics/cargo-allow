use super::{FileFamilyRuleToml, WorkspaceToml};
use allow_core::WorkspaceConfig;

#[test]
fn into_workspace_config_applies_explicit_root_and_default_mode() {
    let workspace = WorkspaceToml {
        root: Some("fixtures/policy".to_string()),
        inventory: None,
        default_mode: Some("strict".to_string()),
        ignored: Vec::new(),
        generated: Vec::new(),
        file_families: Vec::new(),
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
        file_families: Vec::new(),
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
        file_families: Vec::new(),
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
        file_families: Vec::new(),
    };

    let cfg = workspace.into_workspace_config().unwrap();

    assert_eq!(cfg.ignored, vec!["custom/ignored/**".to_string()]);
    assert_eq!(cfg.generated, vec!["custom/generated/**".to_string()]);
}

#[test]
fn into_workspace_config_preserves_custom_file_family_rules() -> Result<(), String> {
    let workspace = WorkspaceToml {
        root: None,
        inventory: None,
        default_mode: None,
        ignored: Vec::new(),
        generated: Vec::new(),
        file_families: vec![FileFamilyRuleToml {
            id: "model-artifact".to_string(),
            family: "ml_model".to_string(),
            glob: "models/**/*.onnx".to_string(),
            reason: "Govern versioned model artifacts.".to_string(),
        }],
    };

    let cfg = workspace
        .into_workspace_config()
        .map_err(|err| err.to_string())?;

    assert_eq!(cfg.file_families.len(), 1);
    assert_eq!(cfg.file_families[0].id, "model-artifact");
    assert_eq!(cfg.file_families[0].family, "ml_model");
    assert_eq!(cfg.file_families[0].glob, "models/**/*.onnx");
    assert!(cfg.file_families[0].reason == "Govern versioned model artifacts.");
    Ok(())
}
