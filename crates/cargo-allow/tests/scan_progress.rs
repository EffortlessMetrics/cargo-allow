use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[test]
fn audit_emits_scan_status_for_human_terminal_output() -> Result<(), String> {
    let root = TempRoot::new("audit-scan-progress")?;
    fs::write(root.path().join("tracked.txt"), "tracked\n").map_err(|error| error.to_string())?;

    let result = run(&[
        "audit",
        "--root",
        root.path().to_str().ok_or("non-UTF-8 root")?,
    ])?;
    if !result.status.success() {
        return Err(format!(
            "audit failed: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    if !String::from_utf8_lossy(&result.stderr).contains("cargo-allow audit: scanning...") {
        return Err(format!(
            "audit did not emit scan status: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }

    Ok(())
}

#[test]
fn scan_status_stays_off_machine_and_quiet_surfaces() -> Result<(), String> {
    let root = TempRoot::new("scan-progress-suppressed")?;
    fs::write(root.path().join("tracked.txt"), "tracked\n").map_err(|error| error.to_string())?;
    let root_text = root.path().to_str().ok_or("non-UTF-8 root")?;

    let json = run(&["audit", "--root", root_text, "--format", "json"])?;
    let quiet = run(&["audit", "--root", root_text, "--quiet"])?;
    if !json.status.success() || !quiet.status.success() {
        return Err(format!(
            "suppressed-surface audit failed: json={} quiet={}",
            json.status, quiet.status
        ));
    }
    let json_stderr = String::from_utf8_lossy(&json.stderr);
    let quiet_stderr = String::from_utf8_lossy(&quiet.stderr);
    if json_stderr.contains("scanning...") || quiet_stderr.contains("scanning...") {
        return Err(format!(
            "suppressed surfaces emitted scan status: json={json_stderr} quiet={quiet_stderr}"
        ));
    }

    Ok(())
}

fn run(args: &[&str]) -> Result<Output, String> {
    cargo_allow_command()
        .args(args)
        .output()
        .map_err(|error| error.to_string())
}

fn cargo_allow_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Result<Self, String> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cargo-allow-{label}-{}-{timestamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).map_err(|error| error.to_string())?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
