use super::*;
use crate::{CargoAllowCli, CargoAllowCommand, ProfileArg, RootArgs};
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

    cmd_init(&InitArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        strict: false,
        profile: None,
        dry_run: false,
        force: false,
        config: PathBuf::from("policy/allow.toml"),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("init should write policy: {err}")));

    assert!(
        policy.exists(),
        "init should resolve relative config paths under the source-tree root"
    );

    remove_init_fixture_dir(root);
}

#[test]
fn cmd_init_dry_run_does_not_write_default_policy() {
    let root = init_fixture_dir();

    cmd_init(&InitArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        strict: false,
        profile: None,
        dry_run: true,
        force: false,
        config: PathBuf::from("policy/allow.toml"),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("init dry-run should pass: {err}")));

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
        !root.join("policy/spec-system.toml").exists(),
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
        "policy/spec-system.toml",
        "policy/doc-artifacts.toml",
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
        ".codex/goals/README.md",
        ".codex/goals/active.toml",
        ".codex/goals/archive/.gitkeep",
    ] {
        assert!(
            root.join(path).exists(),
            "spec-system init should create {path}"
        );
    }
    let config = fs::read_to_string(root.join("policy/spec-system.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read profile config: {err}")));
    assert!(config.contains("profile = \"spec-system\""));
    assert!(config.contains("mode = \"advisory\""));

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

    assert!(result.is_err());
    let Err(err) = result else {
        return;
    };
    assert!(err.to_string().contains("--strict is not supported"));

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
