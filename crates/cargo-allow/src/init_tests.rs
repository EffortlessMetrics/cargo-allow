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
fn init_non_force_selects_create_new_only_mode() {
    // #2777: mode selection is the durable contract; CreateNewOnly closes the
    // exists()-then-AtomicReplace TOCTOU window on the non-force path.
    assert_eq!(
        init_policy_apply_mode(false),
        SingleTargetApplyMode::CreateNewOnly
    );
    assert_eq!(
        init_policy_apply_mode(true),
        SingleTargetApplyMode::ReplaceWithBackup
    );
}

#[test]
fn init_non_force_create_new_only_fails_closed_on_late_target() {
    // #2777: a target that appears after the early exists() probe must not be
    // overwritten when applying with the non-force init mode.
    let root = init_fixture_dir();
    let policy_rel = PathBuf::from("policy/allow.toml");
    let policy = root.join(&policy_rel);
    fs::create_dir_all(
        policy
            .parent()
            .unwrap_or_else(|| std::panic::panic_any("fixture policy parent should exist")),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("create policy parent: {err}")));
    assert!(
        !policy.exists(),
        "probe-time absence is required to model the TOCTOU window"
    );

    fs::write(&policy, "policy = \"foreign\"\n").unwrap_or_else(|err| {
        std::panic::panic_any(format!("simulate external target creation: {err}"))
    });

    let contents = starter_policy(false);
    let response = apply_single_target(SingleTargetApplyRequest {
        repository_root: &root,
        target: &policy_rel,
        contents: &contents,
        caller_reference: Some("cargo-allow:init"),
        lock_identity: None,
        mode: init_policy_apply_mode(false),
    });
    assert!(
        !response.receipt.applied(),
        "non-force init apply must fail when the target appears before write: {:?}",
        response.receipt.error_detail
    );
    assert_eq!(
        fs::read_to_string(&policy)
            .unwrap_or_else(|err| std::panic::panic_any(format!("read foreign policy: {err}"))),
        "policy = \"foreign\"\n",
        "foreign bytes must survive failed non-force init apply"
    );

    let atomic_response = apply_single_target(SingleTargetApplyRequest {
        repository_root: &root,
        target: &policy_rel,
        contents: &contents,
        caller_reference: Some("cargo-allow:init"),
        lock_identity: None,
        mode: SingleTargetApplyMode::AtomicReplace,
    });
    assert!(
        atomic_response.receipt.applied(),
        "old AtomicReplace path would have overwritten the late target"
    );
    assert!(
        fs::read_to_string(&policy)
            .unwrap_or_else(|err| std::panic::panic_any(format!("read replaced policy: {err}")))
            .contains("policy = \"cargo-allow\""),
        "AtomicReplace demonstrates the pre-#2777 overwrite behavior"
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

    let message = err.to_string();
    assert!(
        message.contains("failed to create")
            || message.contains("failed to read")
            || message.contains("Not a directory"),
        "expected parent creation or apply read failure, got: {err}"
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
    // The policy path is a directory, so repo-edit apply fails before install.
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

    let message = err.to_string();
    assert!(
        message.contains("failed to install") || message.contains("failed to read"),
        "expected install or read failure for directory target, got: {err}"
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
fn dry_run_announcement_includes_preview_and_next_steps() {
    // #2596: dry-run should preview the starter policy shape and show the
    // next-steps guidance, not just the would-create line.
    let out = super::dry_run_announcement_styled(
        "create",
        "policy/allow.toml",
        false,
        allow_report::Style::PLAIN,
    );

    assert!(
        out.starts_with("would create policy/allow.toml\n"),
        "dry-run should announce intent first: {out}"
    );
    assert!(
        out.contains("starter policy shape:"),
        "dry-run should preview the policy shape: {out}"
    );
    assert!(
        out.contains("default_mode       = no-new"),
        "non-strict preview should show default_mode = no-new: {out}"
    );
    assert!(
        out.contains("stale_entries_fail = false"),
        "non-strict preview should show stale_entries_fail = false: {out}"
    );
    assert!(
        out.contains("next steps:"),
        "dry-run should show next-steps guidance: {out}"
    );
    assert!(
        out.contains("cargo-allow check --mode no-new"),
        "dry-run next steps should mention the CI gate command: {out}"
    );
}

#[test]
fn dry_run_announcement_strict_preview_promotes_mode_and_stale_failure() {
    let out = super::dry_run_announcement_styled(
        "create",
        "policy/allow.toml",
        true,
        allow_report::Style::PLAIN,
    );

    assert!(
        out.contains("default_mode       = strict"),
        "strict preview should show strict mode: {out}"
    );
    assert!(
        out.contains("stale_entries_fail = true"),
        "strict preview should show stale_entries_fail = true: {out}"
    );
}

#[test]
fn dry_run_announcement_keep_and_overwrite_use_action_word() {
    let keep = super::dry_run_announcement_styled(
        "keep",
        "policy/allow.toml",
        false,
        allow_report::Style::PLAIN,
    );
    let overwrite = super::dry_run_announcement_styled(
        "overwrite",
        "policy/allow.toml",
        false,
        allow_report::Style::PLAIN,
    );

    assert!(
        keep.starts_with("would keep policy/allow.toml\n"),
        "keep action should be announced: {keep}"
    );
    assert!(
        overwrite.starts_with("would overwrite policy/allow.toml\n"),
        "overwrite action should be announced: {overwrite}"
    );
}

#[test]
fn post_write_announcement_shares_next_steps_with_dry_run() {
    // The non-dry-run path should emit identical next-steps text so the two
    // paths don't drift.
    let created = super::post_write_announcement_styled(
        "created",
        "policy/allow.toml",
        allow_report::Style::PLAIN,
    );
    let dry = super::dry_run_announcement_styled(
        "create",
        "policy/allow.toml",
        false,
        allow_report::Style::PLAIN,
    );

    assert!(created.starts_with("created policy/allow.toml\n"));
    let created_steps = created.split("next steps:\n").nth(1).unwrap_or("");
    let dry_steps = dry.split("next steps:\n").nth(1).unwrap_or("");
    assert_eq!(
        created_steps, dry_steps,
        "next-steps text must match between dry-run and write paths"
    );
}

#[test]
fn init_human_summary_styles_fixed_action_markers_only() {
    let dry = super::dry_run_announcement_styled(
        "create",
        "policy/allow.toml",
        false,
        allow_report::Style::ANSI,
    );
    assert!(dry.contains("would \u{1b}[33mcreate\u{1b}[0m policy/allow.toml"));
    assert!(!dry.contains("policy/allow.toml\u{1b}"));

    let written = super::post_write_announcement_styled(
        "created",
        "policy/allow.toml",
        allow_report::Style::ANSI,
    );
    assert!(written.starts_with("\u{1b}[32mcreated\u{1b}[0m policy/allow.toml\n"));
    assert!(!written.contains("policy/allow.toml\u{1b}"));
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
        phase: None,
        staged: false,
        staged_identity_only: false,
        expect_staged_identity: None,
        tool_mode: None,
        tool_digest: None,
        preview_authorized: false,
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
        CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::Usage,
            "--strict is not supported with --profile spec-system; remove --strict or drop --profile spec-system"
        )
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
