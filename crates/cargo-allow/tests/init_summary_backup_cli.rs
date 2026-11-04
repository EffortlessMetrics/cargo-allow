//! Real-process backup collision regressions for #4714 / PR #4716.

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
            "refusal" => {}
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
