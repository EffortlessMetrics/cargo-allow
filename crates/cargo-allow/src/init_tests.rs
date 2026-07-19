use super::*;
use crate::{CargoAllowCli, CargoAllowCommand, ProfileArg, RootArgs};
use allow_core::CargoAllowError;
use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn clap_parses_init_root_config_and_force() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "init",
        "--root",
        "fixtures/source-tree",
        "--strict",
        "--force",
        "--config",
        "target/allow.toml",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse init: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Init(InitArgs {
            root: RootArgs { root: Some(root) },
            strict: true,
            profile: None,
            dry_run: false,
            force: true,
            config,
        })) if root == Path::new("fixtures/source-tree")
            && config == Path::new("target/allow.toml")
    ));
}

#[test]
fn clap_parses_spec_system_profile_for_init() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "init",
        "--profile",
        "spec-system",
        "--dry-run",
    ]))
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("CLI should parse spec-system init: {err}"))
    });

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Init(InitArgs {
            profile: Some(ProfileArg::SpecSystem),
            dry_run: true,
            ..
        }))
    ));
}

#[test]
fn cmd_init_writes_relative_config_under_explicit_root() {
    let root = init_fixture_dir();
    let policy = root.join("policy/allow.toml");

    let result = cmd_init(&InitArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        strict: false,
        profile: None,
        dry_run: false,
        force: false,
        config: PathBuf::from("policy/allow.toml"),
    });

    assert_eq!(result, Ok(()));
    assert!(
        policy.exists(),
        "init should resolve relative config paths under the source-tree root"
    );

    remove_init_fixture_dir(root);
}

#[test]
fn cmd_init_rejects_existing_policy_without_force() {
    let root = init_fixture_dir();
    let canonical_root = root
        .canonicalize()
        .unwrap_or_else(|err| std::panic::panic_any(format!("canonicalize fixture root: {err}")));
    let policy = canonical_root.join("policy/allow.toml");
    fs::create_dir_all(
        policy
            .parent()
            .unwrap_or_else(|| std::panic::panic_any("fixture policy parent should exist")),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("create policy parent: {err}")));
    fs::write(&policy, "policy = \"cargo-allow\"\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("seed existing policy: {err}")));

    let err = cmd_init(&InitArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        strict: false,
        profile: None,
        dry_run: false,
        force: false,
        config: PathBuf::from("policy/allow.toml"),
    })
    .expect_err("existing policy should fail without --force");

    assert_eq!(
        err,
        CargoAllowError::new(format!(
            "{} already exists; use --force to overwrite",
            policy.display()
        ))
    );

    remove_init_fixture_dir(root);
}

#[test]
fn cmd_init_reports_parent_creation_errors() {
    let root = init_fixture_dir();
    let canonical_root = root
        .canonicalize()
        .unwrap_or_else(|err| std::panic::panic_any(format!("canonicalize fixture root: {err}")));
    let policy_parent = canonical_root.join("policy");
    fs::write(&policy_parent, "not a directory").unwrap_or_else(|err| {
        std::panic::panic_any(format!("create policy parent file blocker: {err}"))
    });
    let source_error = fs::create_dir_all(&policy_parent)
        .expect_err("creating a directory over a file should fail");

    let err = cmd_init(&InitArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        strict: false,
        profile: None,
        dry_run: false,
        force: false,
        config: PathBuf::from("policy/allow.toml"),
    })
    .expect_err("file parent should fail directory creation");

    assert_eq!(
        err,
        CargoAllowError::new(format!(
            "failed to create {}: {source_error}",
            policy_parent.display()
        ))
    );

    remove_init_fixture_dir(root);
}

#[test]
fn cmd_init_reports_policy_write_errors() {
    let root = init_fixture_dir();
    let canonical_root = root
        .canonicalize()
        .unwrap_or_else(|err| std::panic::panic_any(format!("canonicalize fixture root: {err}")));
    let policy = canonical_root.join("policy/allow.toml");
    fs::create_dir_all(&policy).unwrap_or_else(|err| {
        std::panic::panic_any(format!("create policy directory target: {err}"))
    });
    // The policy path is a directory, so write_file's atomic rename step fails
    // with "failed to install" (it cannot rename a temp file over a directory).
    let err = cmd_init(&InitArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        strict: false,
        profile: None,
        dry_run: false,
        force: true,
        config: PathBuf::from("policy/allow.toml"),
    })
    .expect_err("directory policy target should fail policy write");

    assert!(
        err.to_string()
            .contains(&format!("failed to install {}", policy.display())),
        "expected 'failed to install' error for directory target, got: {err}"
    );

    remove_init_fixture_dir(root);
}

#[test]
fn cmd_init_dry_run_does_not_write_default_policy() {
    let root = init_fixture_dir();

    let result = cmd_init(&InitArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        strict: false,
        profile: None,
        dry_run: true,
        force: false,
        config: PathBuf::from("policy/allow.toml"),
    });

    assert_eq!(result, Ok(()));
    assert!(
        !root.join("policy/allow.toml").exists(),
        "default init dry-run should not write policy/allow.toml"
    );

    remove_init_fixture_dir(root);
}

#[test]
fn spec_system_init_dry_run_does_not_write_bootstrap_files() {
    let root = init_fixture_dir();

    cmd_init(&InitArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        strict: false,
        profile: Some(ProfileArg::SpecSystem),
        dry_run: true,
        force: false,
        config: PathBuf::from("policy/allow.toml"),
    })
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("spec-system init dry-run should pass: {err}"))
    });

    assert!(
        !root.join(".allow/profiles/spec-system.toml").exists(),
        "spec-system init dry-run should not write profile config"
    );
    assert!(
        !root.join("docs/templates/spec.md").exists(),
        "spec-system init dry-run should not write templates"
    );

    remove_init_fixture_dir(root);
}

#[test]
fn spec_system_init_bootstraps_profile_files() {
    let root = init_fixture_dir();

    cmd_init(&InitArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        strict: false,
        profile: Some(ProfileArg::SpecSystem),
        dry_run: false,
        force: false,
        config: PathBuf::from("policy/allow.toml"),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("spec-system init should pass: {err}")));

    for path in [
        ".allow/profiles/spec-system.toml",
        ".allow/artifacts/doc-artifacts.toml",
        ".allow/imports/README.md",
        "docs/proposals/README.md",
        "docs/specs/README.md",
        "docs/adr/README.md",
        "docs/templates/proposal.md",
        "docs/templates/spec.md",
        "docs/templates/adr.md",
        "docs/templates/implementation-plan.md",
        "docs/templates/plan-item.md",
        "docs/templates/closeout.md",
        "docs/templates/pr-body.md",
        "docs/status/SUPPORT_TIERS.md",
        "plans/README.md",
    ] {
        assert!(
            root.join(path).exists(),
            "spec-system init should create {path}"
        );
    }
    let config = fs::read_to_string(root.join(".allow/profiles/spec-system.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read profile config: {err}")));
    assert!(config.contains("profile = \"spec-system\""));
    assert!(config.contains("mode = \"advisory\""));
    assert!(config.contains("generation = \"current-v2\""));
    assert!(!config.contains("goals = \".allow/goals\""));
    assert!(config.contains("artifact_ledger = \".allow/artifacts/doc-artifacts.toml\""));
    assert!(!config.contains("active_goal_required"));
    assert!(!root.join(".allow/goals").exists());

    remove_init_fixture_dir(root);
}

#[test]
fn spec_system_init_explicit_legacy_profile_bootstraps_legacy_goal_files() {
    let root = init_fixture_dir();
    let config_path = root.join("legacy-profile.toml");
    fs::write(
        &config_path,
        r#"
schema_version = "1.0"
profile = "spec-system"
mode = "advisory"
generation = "legacy-v1"

[roots]
proposals = "docs/proposals"
specs = "docs/specs"
adrs = "docs/adr"
plans = "plans"
goals = ".allow/goals"
support_tiers = "docs/status/SUPPORT_TIERS.md"
artifact_ledger = ".allow/artifacts/doc-artifacts.toml"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write legacy profile: {err}")));

    cmd_init(&InitArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        strict: false,
        profile: Some(ProfileArg::SpecSystem),
        dry_run: false,
        force: false,
        config: PathBuf::from("legacy-profile.toml"),
    })
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("legacy spec-system init should pass: {err}"))
    });

    for path in [
        ".allow/goals/README.md",
        ".allow/goals/active.toml",
        ".allow/goals/archive/.gitkeep",
    ] {
        assert!(root.join(path).exists(), "legacy init should create {path}");
    }
    let config = fs::read_to_string(root.join("legacy-profile.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read legacy profile: {err}")));
    assert!(config.contains("generation = \"legacy-v1\""));
    assert!(config.contains("goals = \".allow/goals\""));

    remove_init_fixture_dir(root);
}

#[test]
fn spec_system_init_current_profile_reports_legacy_conflict_without_writing() {
    let root = init_fixture_dir();
    fs::create_dir_all(root.join(".allow/goals"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create legacy root: {err}")));
    let result = cmd_init(&InitArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        strict: false,
        profile: Some(ProfileArg::SpecSystem),
        dry_run: false,
        force: false,
        config: PathBuf::from("policy/allow.toml"),
    });

    assert!(result.is_err());
    assert!(!root.join(".allow/profiles/spec-system.toml").exists());
    remove_init_fixture_dir(root);
}

#[test]
fn spec_system_init_bootstrapped_profile_loads_via_check() {
    let root = init_fixture_dir();

    cmd_init(&InitArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        strict: false,
        profile: Some(ProfileArg::SpecSystem),
        dry_run: false,
        force: false,
        config: PathBuf::from("policy/allow.toml"),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("spec-system init should pass: {err}")));

    let output = root.join("check.json");
    let result = crate::check::cmd_check(&crate::check::CheckArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: Some(ProfileArg::SpecSystem),
        compat: false,
        kind: None,
        include_untracked: false,
        format: crate::OutputFormat::Json,
        output: Some(output.clone()),
        receipt: None,
        mode: Some("audit".to_string()),
        deny: Vec::new(),
    });
    assert!(
        result.is_ok(),
        "spec-system check should load bootstrapped .allow profile: {:?}",
        result.err()
    );
    let json = fs::read_to_string(&output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read check output: {err}")));
    assert!(
        json.contains(".allow/profiles/spec-system.toml"),
        "check should resolve owned profile config: {json}"
    );
    assert!(
        json.contains("\"allow_profiles\""),
        "check should report allow_profiles provenance: {json}"
    );

    remove_init_fixture_dir(root);
}

#[test]
fn spec_system_init_does_not_write_legacy_policy_profile_paths() {
    let root = init_fixture_dir();

    cmd_init(&InitArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        strict: false,
        profile: Some(ProfileArg::SpecSystem),
        dry_run: false,
        force: false,
        config: PathBuf::from("policy/allow.toml"),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("spec-system init should pass: {err}")));

    assert!(
        !root.join("policy/spec-system.toml").exists(),
        "init should not create legacy policy/spec-system.toml"
    );
    assert!(
        !root.join("policy/doc-artifacts.toml").exists(),
        "init should not create legacy policy/doc-artifacts.toml"
    );
    assert!(
        !root.join(".codex/goals/active.toml").exists(),
        "init should not create legacy .codex/goals layout"
    );

    remove_init_fixture_dir(root);
}

#[test]
fn spec_system_init_rejects_strict_source_policy_option() {
    let root = init_fixture_dir();

    let result = cmd_init(&InitArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        strict: true,
        profile: Some(ProfileArg::SpecSystem),
        dry_run: false,
        force: false,
        config: PathBuf::from("policy/allow.toml"),
    });

    let err = result.expect_err("strict spec-system init should fail");
    assert_eq!(
        err,
        CargoAllowError::new("--strict is not supported with --profile spec-system")
    );

    remove_init_fixture_dir(root);
}

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}

fn init_fixture_dir() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("cargo-allow-init-{}-{stamp}", std::process::id()));
    remove_init_fixture_dir(dir.clone());
    fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create init fixture: {err}")));
    dir
}

fn remove_init_fixture_dir(path: PathBuf) {
    match fs::remove_dir_all(&path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => std::panic::panic_any(format!("remove init fixture {}: {err}", path.display())),
    }
}
