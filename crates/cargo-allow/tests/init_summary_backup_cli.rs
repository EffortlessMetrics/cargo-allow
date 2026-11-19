//! Real-process summary, backup, and next-step regressions for `init`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);
const POLICY: &[u8] = b"policy = \"cargo-allow\"\n# original-policy-canary\n";
const BACKUP: &[u8] = b"# original-backup-canary\n";

struct Root(PathBuf);

impl Root {
    fn new() -> Result<Self, String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let serial = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "cargo-allow-init-summary-backup-{}-{now}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&path).map_err(|error| error.to_string())?;
        Ok(Self(path))
    }

    fn seed(&self, config: &str, backup_exists: bool) -> Result<(PathBuf, PathBuf), String> {
        let policy = self.0.join(config);
        let parent = policy.parent().ok_or("policy has no parent")?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        fs::write(&policy, POLICY).map_err(|error| error.to_string())?;
        // Pin the writer's existing replacement-extension contract, including
        // non-.toml configurations; this is not an appended ".bak" suffix.
        let backup = policy.with_extension("toml.bak");
        if backup_exists {
            fs::write(&backup, BACKUP).map_err(|error| error.to_string())?;
        }
        Ok((policy, backup))
    }

    fn run(&self, config: &str, summary: &Path, mode: &str) -> Result<Output, String> {
        let cwd = self.0.join("work");
        fs::create_dir_all(&cwd).map_err(|error| error.to_string())?;
        let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-allow"));
        command
            .current_dir(&cwd)
            .arg("--command-summary-output")
            .arg(summary)
            .args(["init", "--strict", "--root"])
            .arg(&self.0);
        if config != "policy/allow.toml" {
            command.args(["--config", config]);
        }
        match mode {
            "force" => {
                command.arg("--force");
            }
            "dry-run" => {
                command.arg("--dry-run");
            }
            "apply" | "refusal" => {}
            _ => return Err(format!("unknown test mode: {mode}")),
        }
        command.output().map_err(|error| error.to_string())
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn bytes(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("read {}: {error}", path.display())),
    }
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

#[test]
fn init_summary_collision_preserves_policy_and_backup_on_every_route() -> Result<(), String> {
    for config in ["policy/allow.toml", "custom/ledger.data"] {
        for backup_exists in [false, true] {
            for mode in ["force", "refusal", "dry-run"] {
                let root = Root::new()?;
                let (policy, backup) = root.seed(config, backup_exists)?;
                let before = bytes(&backup)?;
                // Summary is cwd-relative while --config is root-relative.
                // The dot component exercises canonical alias comparison.
                let summary = Path::new("../.").join(Path::new(config).with_extension("toml.bak"));
                let output = root.run(config, &summary, mode)?;
                require(
                    !output.status.success(),
                    format!("{config}/{mode}: colliding summary unexpectedly succeeded"),
                )?;
                require(
                    bytes(&policy)?.as_deref() == Some(POLICY) && bytes(&backup)? == before,
                    format!("{config}/{mode}: summary changed policy or backup bytes/absence"),
                )?;
            }
        }
    }
    Ok(())
}

#[test]
fn init_backup_collision_refuses_before_creating_a_new_policy() -> Result<(), String> {
    for mode in ["force", "refusal", "dry-run"] {
        let root = Root::new()?;
        let (policy, backup) = root.seed("policy/allow.toml", true)?;
        fs::remove_file(&policy).map_err(|error| error.to_string())?;
        let output = root.run(
            "policy/allow.toml",
            Path::new("../policy/allow.toml.bak"),
            mode,
        )?;
        require(
            !output.status.success(),
            "new-policy collision must be refused",
        )?;
        require(
            bytes(&policy)?.is_none() && bytes(&backup)?.as_deref() == Some(BACKUP),
            "refused new-policy init changed policy absence or the retained backup",
        )?;
    }
    Ok(())
}

#[test]
fn init_separate_summary_keeps_success_preview_and_error_outputs() -> Result<(), String> {
    for config in ["policy/allow.toml", "custom/ledger.data"] {
        for mode in ["force", "refusal", "dry-run"] {
            let root = Root::new()?;
            let (policy, backup) = root.seed(config, true)?;
            let summary = root.0.join("summary.json");
            let output = root.run(config, Path::new("../summary.json"), mode)?;
            require(
                output.status.success() == (mode != "refusal"),
                format!("{config}/{mode}: wrong exit: {:?}", output.status),
            )?;
            let value: serde_json::Value =
                serde_json::from_slice(&fs::read(&summary).map_err(|error| error.to_string())?)
                    .map_err(|error| error.to_string())?;
            require(
                value.get("operation").and_then(serde_json::Value::as_str) == Some("init"),
                "noncolliding summary must still be emitted as an init summary",
            )?;
            let expected_backup = if mode == "force" { POLICY } else { BACKUP };
            require(
                bytes(&backup)?.as_deref() == Some(expected_backup),
                "valid summary corrupted the expected recovery copy",
            )?;
            if mode != "force" {
                require(
                    bytes(&policy)?.as_deref() == Some(POLICY),
                    "read-only route changed policy",
                )?;
            }
        }
    }
    Ok(())
}

#[test]
fn init_git_write_reports_tracking_before_check_without_staging() -> Result<(), String> {
    let root = Root::new()?;
    let git = |args: &[&str]| -> Result<Output, String> {
        let output = Command::new("git")
            .current_dir(&root.0)
            .args(args)
            .output()
            .map_err(|error| error.to_string())?;
        require(
            output.status.success(),
            format!("git {args:?}: {}", String::from_utf8_lossy(&output.stderr)),
        )?;
        Ok(output)
    };
    git(&["init", "--quiet"])?;
    let config = "policy/allow.toml";
    let summary_path = root.0.join("summary.json");
    let read_summary = || -> Result<serde_json::Value, String> {
        let data = fs::read(&summary_path).map_err(|error| error.to_string())?;
        serde_json::from_slice(&data).map_err(|error| error.to_string())
    };

    let preview = root.run(config, &summary_path, "dry-run")?;
    let preview_summary = read_summary()?;
    let preview_action = preview_summary
        .get("primary_action")
        .ok_or("missing init preview action")?;
    require(
        preview.status.success()
            && bytes(&root.0.join(config))?.is_none()
            && preview_action.get("id").and_then(serde_json::Value::as_str) == Some("init.apply")
            && preview_action
                .get("program")
                .and_then(serde_json::Value::as_str)
                == Some("cargo-allow"),
        "dry run must leave the policy absent and still offer init apply",
    )?;

    let applied = root.run(config, &summary_path, "apply")?;
    let applied_summary = read_summary()?;
    let action = applied_summary
        .get("primary_action")
        .ok_or("missing init tracking action")?;
    require(
        applied.status.success()
            && bytes(&root.0.join(config))?.is_some()
            && git(&["ls-files", "--", config])?.stdout.is_empty()
            && action.get("id").and_then(serde_json::Value::as_str)
                == Some("init.ensure_policy_tracked")
            && action.get("kind").and_then(serde_json::Value::as_str) == Some("decision")
            && action.get("program").is_none()
            && action.get("args").is_none()
            && applied_summary
                .get("next_proof")
                .is_none_or(serde_json::Value::is_null)
            && String::from_utf8_lossy(&applied.stdout)
                .lines()
                .any(|line| {
                    line == "Next: Make sure the starter policy is tracked by Git before checking"
                }),
        "live Git init must write the untracked policy and guide the operator without emitting executable check argv",
    )
}

#[cfg(unix)]
#[test]
fn init_summary_through_parent_symlink_cannot_alias_backup() -> Result<(), String> {
    let root = Root::new()?;
    let (policy, backup) = root.seed("policy/allow.toml", true)?;
    std::os::unix::fs::symlink(root.0.join("policy"), root.0.join("alias"))
        .map_err(|error| error.to_string())?;
    let output = root.run(
        "policy/allow.toml",
        Path::new("../alias/allow.toml.bak"),
        "force",
    )?;
    require(
        !output.status.success(),
        "symlink backup collision must be refused",
    )?;
    require(
        bytes(&policy)?.as_deref() == Some(POLICY) && bytes(&backup)?.as_deref() == Some(BACKUP),
        "symlink collision changed the policy or its backup",
    )
}

#[test]
fn init_summary_file_ancestry_preserves_bytes_and_absence() -> Result<(), String> {
    for config in ["policy/allow.toml", "custom/ledger.data"] {
        for policy_exists in [false, true] {
            for mode in ["force", "refusal", "dry-run"] {
                for relationship in ["policy-child", "backup-child", "policy-parent"] {
                    let root = Root::new()?;
                    let policy = root.0.join(config);
                    let backup = policy.with_extension("toml.bak");
                    if policy_exists {
                        root.seed(config, false)?;
                    }
                    let before = bytes(&policy)?;
                    let parent = policy.parent().ok_or("policy has no parent")?;
                    let parent_existed = parent.exists();
                    let summary = match relationship {
                        "policy-child" => policy.join("nested/summary.json"),
                        "backup-child" => backup.join("nested/summary.json"),
                        "policy-parent" => parent.to_path_buf(),
                        _ => return Err("unexpected relationship".to_string()),
                    };
                    let output = root.run(config, &summary, mode)?;
                    require(
                        !output.status.success(),
                        format!("{config}/{mode}/{relationship}: file ancestry accepted"),
                    )?;
                    require(
                        bytes(&policy)? == before
                            && bytes(&backup)?.is_none()
                            && parent.exists() == parent_existed,
                        format!(
                            "{config}/{mode}/{relationship}: refusal changed policy, backup or parent"
                        ),
                    )?;
                }
            }
        }
    }
    Ok(())
}

#[test]
fn init_summary_prefix_lookalike_directory_remains_supported() -> Result<(), String> {
    for mode in ["force", "refusal", "dry-run"] {
        let root = Root::new()?;
        let (policy, backup) = root.seed("policy/allow.toml", true)?;
        let summary = root.0.join("policy/allow.toml.bak-extra/summary.json");
        let output = root.run("policy/allow.toml", &summary, mode)?;
        require(
            output.status.success() == (mode != "refusal") && summary.is_file(),
            "component-distinct prefix sibling must still emit its summary",
        )?;
        let expected = if mode == "force" { POLICY } else { BACKUP };
        require(
            bytes(&backup)?.as_deref() == Some(expected),
            "sibling summary changed backup",
        )?;
        if mode != "force" {
            require(
                bytes(&policy)?.as_deref() == Some(POLICY),
                "read-only route changed policy",
            )?;
        }
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn init_config_symlink_reserves_canonical_backup_on_every_route() -> Result<(), String> {
    for backup_exists in [false, true] {
        for mode in ["force", "refusal", "dry-run"] {
            for child in [false, true] {
                let root = Root::new()?;
                let (policy, backup) = root.seed("policy/allow.toml", backup_exists)?;
                let before = bytes(&backup)?;
                std::os::unix::fs::symlink(&policy, root.0.join("alias.policy"))
                    .map_err(|error| error.to_string())?;
                let summary = if child {
                    backup.join("summary.json")
                } else {
                    backup.clone()
                };
                let output = root.run("alias.policy", &summary, mode)?;
                require(
                    !output.status.success(),
                    "canonical backup collision must be refused",
                )?;
                require(
                    bytes(&policy)?.as_deref() == Some(POLICY) && bytes(&backup)? == before,
                    "config symlink route changed canonical policy or backup",
                )?;
                require(
                    fs::symlink_metadata(root.0.join("alias.policy"))
                        .map_err(|error| error.to_string())?
                        .file_type()
                        .is_symlink(),
                    "config alias must remain unchanged",
                )?;
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn init_config_symlink_noncolliding_summaries_preserve_writer_refusal() -> Result<(), String> {
    for mode in ["force", "refusal", "dry-run"] {
        let root = Root::new()?;
        let (policy, backup) = root.seed("policy/allow.toml", true)?;
        std::os::unix::fs::symlink(&policy, root.0.join("alias.policy"))
            .map_err(|error| error.to_string())?;
        let summary = root.0.join("summary.json");
        let output = root.run("alias.policy", &summary, mode)?;
        require(
            output.status.success() == (mode == "dry-run") && summary.is_file(),
            "noncolliding summary must preserve preview and writer/error behavior",
        )?;
        require(
            bytes(&policy)?.as_deref() == Some(POLICY)
                && bytes(&backup)?.as_deref() == Some(BACKUP),
            "noncolliding config-link route changed protected bytes",
        )?;
    }
    Ok(())
}
