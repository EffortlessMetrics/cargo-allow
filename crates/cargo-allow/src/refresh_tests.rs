use super::{
    RefreshArgs, cmd_refresh, selected_refresh_entry, selected_refresh_entry_mut,
    selected_refresh_finding,
};
use crate::{CargoAllowCli, CargoAllowCommand, HumanJsonFormat};
use allow_core::{AllowConfig, CargoAllowErrorKind, MatchStatus};
use allow_match::{CheckMode, evaluate};
use allow_policy::load_policy;
use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static REFRESH_TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[test]
fn selected_refresh_text_refuses_a_changed_preimage() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_fixture_copy();
    let policy = root.join("policy/allow.toml");
    let original = fs::read(&policy)?;
    let digest = allow_core::sha256_v1_bytes(&original);
    let mut changed = original;
    changed.extend_from_slice(b"\n# external change\n");
    fs::write(&policy, &changed)?;
    let Err(error) = super::refresh_policy::refresh_bound_policy(
        &policy,
        &digest,
        &[(
            "allow-0250",
            &allow_core::LastSeen {
                line: 22,
                column: 5,
            },
        )],
    ) else {
        return Err("stale refresh preimage unexpectedly accepted".into());
    };
    assert_eq!(error.kind(), CargoAllowErrorKind::Usage);
    assert_eq!(fs::read(&policy)?, changed);
    fs::remove_dir_all(root)?;
    Ok(())
}

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/refresh/advisory-drift")
}

fn bulk_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/refresh/bulk-drift")
}

fn unique_fixture_copy() -> PathBuf {
    unique_fixture_copy_from(&fixture_root(), "cargo-allow-refresh")
}

fn unique_bulk_fixture_copy() -> PathBuf {
    unique_fixture_copy_from(&bulk_fixture_root(), "cargo-allow-refresh-bulk")
}

fn unique_fixture_copy_from(source: &Path, prefix: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let id = REFRESH_TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("{prefix}-{stamp}-{id}"));
    fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create refresh fixture copy: {err}")));
    copy_dir(source, &dir);
    dir
}

fn copy_dir(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).unwrap_or_else(|err| {
        std::panic::panic_any(format!("read fixture dir {}: {err}", from.display()))
    }) {
        let entry =
            entry.unwrap_or_else(|err| std::panic::panic_any(format!("read fixture entry: {err}")));
        let target = to.join(entry.file_name());
        let file_type = entry
            .file_type()
            .unwrap_or_else(|err| std::panic::panic_any(format!("read fixture entry type: {err}")));
        if file_type.is_dir() {
            fs::create_dir_all(&target).unwrap_or_else(|err| {
                std::panic::panic_any(format!("create fixture subdir: {err}"))
            });
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target)
                .unwrap_or_else(|err| std::panic::panic_any(format!("copy fixture file: {err}")));
        }
    }
}

#[test]
fn clap_parses_refresh_dry_run_json() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "refresh",
        "--allow-id",
        "allow-0250",
        "--dry-run",
        "--include-untracked",
        "--format",
        "json",
        "--output",
        "target/refresh.json",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Refresh(RefreshArgs {
            allow_id,
            dry_run: true,
            include_untracked: true,
            format: HumanJsonFormat::Json,
            output: Some(path),
            ..
        })) if allow_id.as_deref() == Some("allow-0250") && path == Path::new("target/refresh.json")
    ));
}

#[test]
fn refresh_index_invariants_are_internal_when_missing() {
    let findings = Vec::new();
    let err = selected_refresh_finding(&findings, 0)
        .expect_err("a missing refresh finding index should fail closed");
    assert_eq!(err.kind(), CargoAllowErrorKind::Internal);
    assert_eq!(err.code(), "E0008_INTERNAL");
    assert_eq!(
        err.to_string(),
        "internal error: selected finding index out of range"
    );

    let config = AllowConfig::empty();
    let err = selected_refresh_entry(&config, 0)
        .expect_err("a missing refresh allow-entry index should fail closed");
    assert_eq!(err.kind(), CargoAllowErrorKind::Internal);
    assert_eq!(err.code(), "E0008_INTERNAL");
    assert_eq!(
        err.to_string(),
        "internal error: selected allow entry index out of range"
    );

    let mut config = AllowConfig::empty();
    let err = selected_refresh_entry_mut(&mut config, 0)
        .expect_err("a missing mutable refresh allow-entry index should fail closed");
    assert_eq!(err.kind(), CargoAllowErrorKind::Internal);
    assert_eq!(err.code(), "E0008_INTERNAL");
    assert_eq!(
        err.to_string(),
        "internal error: selected allow entry index out of range"
    );
}

#[test]
fn cmd_refresh_write_reports_missing_policy_config_with_structured_error() {
    let root = unique_fixture_copy();
    fs::remove_file(root.join("policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove refresh policy: {err}")));

    let err = cmd_refresh(&RefreshArgs {
        root: crate::RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        allow_id: Some("allow-0250".to_string()),
        all: false,
        dry_run: false,
        write: true,
        include_untracked: true,
        format: HumanJsonFormat::Human,
        output: None,
    })
    .expect_err("refresh write without policy config should fail");

    assert_eq!(err.kind(), allow_core::CargoAllowErrorKind::InvalidConfig);
    assert_eq!(err.code(), "E0002_INVALID_CONFIG");
    assert!(err.to_string().contains("cargo-allow init"));

    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove refresh fixture: {err}")));
}

#[test]
fn refresh_fixture_records_drift_receipt_without_extending_lifecycle() {
    let root = unique_fixture_copy();
    let policy_path = root.join("policy/allow.toml");
    let config_arg = PathBuf::from("policy/allow.toml");
    let output_path = root.join("refresh-summary.json");
    let (_loaded_root, cfg, findings, _facts, _federation) =
        crate::load_world(Some(&root), Some(&config_arg), true, None, true).unwrap_or_else(|err| {
            std::panic::panic_any(format!("load refresh fixture world: {err}"))
        });
    let outcomes = evaluate(
        &cfg,
        &findings,
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );
    assert!(
        outcomes.iter().any(|outcome| {
            outcome.allow_id.as_deref() == Some("allow-0250")
                && outcome.status == MatchStatus::LocationDrift
        }),
        "fixture should report advisory location drift before refresh"
    );

    cmd_refresh(&RefreshArgs {
        root: crate::RootArgs {
            root: Some(root.clone()),
        },
        config: Some(config_arg.clone()),
        allow_id: Some("allow-0250".to_string()),
        all: false,
        dry_run: true,
        write: false,
        include_untracked: true,
        format: HumanJsonFormat::Json,
        output: Some(output_path.clone()),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("refresh dry-run: {err}")));

    let summary = fs::read_to_string(&output_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read refresh summary: {err}")));
    assert!(summary.contains("cargo-allow.refresh.v1"));
    assert!(summary.contains("\"lifecycle_preserved\": true"));

    let before = load_policy(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("reload policy before write: {err}")));
    let lifecycle_before = before
        .allow
        .iter()
        .find(|entry| entry.id == "allow-0250")
        .map(|entry| entry.lifecycle.clone())
        .unwrap_or_else(|| std::panic::panic_any("expected refresh fixture entry"));

    cmd_refresh(&RefreshArgs {
        root: crate::RootArgs {
            root: Some(root.clone()),
        },
        config: Some(config_arg.clone()),
        allow_id: Some("allow-0250".to_string()),
        all: false,
        dry_run: false,
        write: true,
        include_untracked: true,
        format: HumanJsonFormat::Human,
        output: None,
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("refresh write: {err}")));

    let after = load_policy(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("reload policy after write: {err}")));
    let entry = after
        .allow
        .iter()
        .find(|entry| entry.id == "allow-0250")
        .unwrap_or_else(|| std::panic::panic_any("expected refreshed entry"));
    assert_eq!(entry.lifecycle, lifecycle_before);
    assert!(
        entry
            .last_seen
            .as_ref()
            .is_some_and(|last_seen| last_seen.line > 1),
        "refresh should move last_seen to the current finding coordinates"
    );

    let post_outcomes = evaluate(
        &after,
        &findings,
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );
    assert!(
        post_outcomes.iter().any(|outcome| {
            outcome.allow_id.as_deref() == Some("allow-0250")
                && outcome.status == MatchStatus::Matched
        }),
        "refresh should clear location drift for the fixture entry"
    );
}

#[test]
fn clap_parses_refresh_all_without_allow_id() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "refresh", "--all"]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse --all: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Refresh(RefreshArgs {
            all: true,
            allow_id: None,
            ..
        }))
    ));
}

#[test]
fn clap_rejects_refresh_all_combined_with_allow_id() {
    let Err(error) = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "refresh",
        "--all",
        "--allow-id",
        "allow-0250",
    ])) else {
        std::panic::panic_any("--all must conflict with --allow-id");
    };

    assert!(
        error.to_string().contains("--allow-id"),
        "conflict diagnostic should name the conflicting flag: {error}"
    );
}

#[test]
fn cmd_refresh_without_selector_fails_closed_with_usage() {
    let root = unique_bulk_fixture_copy();

    let err = cmd_refresh(&RefreshArgs {
        root: crate::RootArgs {
            root: Some(root.clone()),
        },
        config: Some(PathBuf::from("policy/allow.toml")),
        allow_id: None,
        all: false,
        dry_run: true,
        write: false,
        include_untracked: true,
        format: HumanJsonFormat::Human,
        output: None,
    })
    .expect_err("refresh without --allow-id or --all should fail closed");

    assert_eq!(err.kind(), CargoAllowErrorKind::Usage);
    assert_eq!(err.code(), "E0001_USAGE");
    assert_eq!(
        err.to_string(),
        "allow entry id is required; pass --allow-id <id> or use --all for bulk refresh"
    );

    fs::remove_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("remove refresh fixture: {err}")));
}

#[test]
fn refresh_all_dry_run_emits_one_parseable_receipt_per_drifted_entry()
-> Result<(), Box<dyn std::error::Error>> {
    let root = unique_bulk_fixture_copy();
    let policy_path = root.join("policy/allow.toml");
    let config_arg = PathBuf::from("policy/allow.toml");
    let output_path = root.join("refresh-bulk-dry-run.json");
    let policy_before = fs::read(&policy_path)?;
    let (_loaded_root, cfg, findings, _facts, _federation) =
        crate::load_world(Some(&root), Some(&config_arg), true, None, true).unwrap_or_else(|err| {
            std::panic::panic_any(format!("load bulk refresh fixture world: {err}"))
        });
    let outcomes = evaluate(
        &cfg,
        &findings,
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );
    for id in ["allow-11366", "allow-11368"] {
        assert!(
            outcomes.iter().any(|outcome| {
                outcome.allow_id.as_deref() == Some(id)
                    && outcome.status == MatchStatus::LocationDrift
            }),
            "bulk fixture should report advisory location drift for {id} before refresh"
        );
    }

    cmd_refresh(&RefreshArgs {
        root: crate::RootArgs {
            root: Some(root.clone()),
        },
        config: Some(config_arg),
        allow_id: None,
        all: true,
        dry_run: true,
        write: false,
        include_untracked: true,
        format: HumanJsonFormat::Json,
        output: Some(output_path.clone()),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("bulk refresh dry-run: {err}")));

    let summary = fs::read_to_string(&output_path)?;
    let parsed: serde_json::Value = serde_json::from_str(&summary)
        .unwrap_or_else(|err| std::panic::panic_any(format!("bulk output should be JSON: {err}")));
    let documents = parsed
        .as_array()
        .unwrap_or_else(|| std::panic::panic_any("bulk JSON output should be an array"));
    assert_eq!(documents.len(), 2);
    fn summary_field<'a>(document: &'a serde_json::Value, field: &str) -> &'a serde_json::Value {
        document
            .get("summary")
            .and_then(|summary| summary.get(field))
            .unwrap_or_else(|| {
                std::panic::panic_any(format!("receipt should carry summary.{field}"))
            })
    }
    let mut entry_ids: Vec<_> = documents
        .iter()
        .map(|document| {
            summary_field(document, "entry_id")
                .as_str()
                .unwrap_or_else(|| std::panic::panic_any("receipt should carry entry_id"))
        })
        .collect();
    entry_ids.sort_unstable();
    assert_eq!(entry_ids, ["allow-11366", "allow-11368"]);
    assert!(documents.iter().all(|document| {
        summary_field(document, "lifecycle_preserved") == &serde_json::Value::Bool(true)
            && document
                .get("mode")
                .and_then(|mode| mode.get("dry_run"))
                .unwrap_or_else(|| std::panic::panic_any("receipt should carry mode.dry_run"))
                == &serde_json::Value::Bool(true)
    }));
    assert_eq!(
        fs::read(&policy_path)?,
        policy_before,
        "dry run must not touch the policy file"
    );

    fs::remove_dir_all(&root)?;
    Ok(())
}

#[test]
fn refresh_all_write_refreshes_every_drifted_entry_then_fails_closed_when_fresh()
-> Result<(), Box<dyn std::error::Error>> {
    let root = unique_bulk_fixture_copy();
    let policy_path = root.join("policy/allow.toml");
    let config_arg = PathBuf::from("policy/allow.toml");
    let (_loaded_root, cfg, findings, _facts, _federation) =
        crate::load_world(Some(&root), Some(&config_arg), true, None, true).unwrap_or_else(|err| {
            std::panic::panic_any(format!("load bulk refresh fixture world: {err}"))
        });
    let lifecycle_before = |config: &allow_core::AllowConfig, id: &str| {
        config
            .allow
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.lifecycle.clone())
    };

    cmd_refresh(&RefreshArgs {
        root: crate::RootArgs {
            root: Some(root.clone()),
        },
        config: Some(config_arg.clone()),
        allow_id: None,
        all: true,
        dry_run: false,
        write: true,
        include_untracked: true,
        format: HumanJsonFormat::Human,
        output: None,
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("bulk refresh write: {err}")));

    let after = load_policy(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("reload policy after write: {err}")));
    for id in ["allow-11366", "allow-11368"] {
        let entry = after
            .allow
            .iter()
            .find(|entry| entry.id == id)
            .unwrap_or_else(|| std::panic::panic_any(format!("expected refreshed entry {id}")));
        assert_eq!(
            Some(&entry.lifecycle),
            lifecycle_before(&cfg, id).as_ref(),
            "bulk refresh must preserve the {id} lifecycle"
        );
        assert!(
            entry
                .last_seen
                .as_ref()
                .is_some_and(|last_seen| last_seen.line > 1),
            "bulk refresh should move the {id} last_seen to current coordinates"
        );
    }

    let post_outcomes = evaluate(
        &after,
        &findings,
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );
    for id in ["allow-11366", "allow-11368"] {
        assert!(
            post_outcomes.iter().any(|outcome| {
                outcome.allow_id.as_deref() == Some(id) && outcome.status == MatchStatus::Matched
            }),
            "bulk refresh should clear location drift for {id}"
        );
    }

    let err = cmd_refresh(&RefreshArgs {
        root: crate::RootArgs {
            root: Some(root.clone()),
        },
        config: Some(config_arg),
        allow_id: None,
        all: true,
        dry_run: true,
        write: false,
        include_untracked: true,
        format: HumanJsonFormat::Human,
        output: None,
    })
    .expect_err("bulk refresh with no drifted entries should fail closed");

    assert_eq!(err.kind(), CargoAllowErrorKind::Usage);
    assert!(
        err.to_string()
            .contains("no allow entries have advisory location drift"),
        "empty bulk run should report the empty drift set: {err}"
    );

    fs::remove_dir_all(&root)?;
    Ok(())
}
