use super::{RefreshArgs, cmd_refresh};
use crate::{CargoAllowCli, CargoAllowCommand, HumanJsonFormat};
use allow_core::MatchStatus;
use allow_match::{CheckMode, evaluate};
use allow_policy::load_policy;
use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static REFRESH_TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/refresh/advisory-drift")
}

fn unique_fixture_copy() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let id = REFRESH_TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("cargo-allow-refresh-{stamp}-{id}"));
    fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create refresh fixture copy: {err}")));
    copy_dir(&fixture_root(), &dir);
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
        })) if allow_id == "allow-0250" && path == Path::new("target/refresh.json")
    ));
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
        allow_id: "allow-0250".to_string(),
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
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);
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
        allow_id: "allow-0250".to_string(),
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
        allow_id: "allow-0250".to_string(),
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

    let post_outcomes = evaluate(&after, &findings, CheckMode::NoNew);
    assert!(
        post_outcomes.iter().any(|outcome| {
            outcome.allow_id.as_deref() == Some("allow-0250")
                && outcome.status == MatchStatus::Matched
        }),
        "refresh should clear location drift for the fixture entry"
    );
}
