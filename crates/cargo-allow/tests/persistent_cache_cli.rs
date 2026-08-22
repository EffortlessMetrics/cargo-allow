use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) -> Result<(), String> {
    let status = Command::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("git {:?} failed with {status}", args))
    }
}

fn semantic_receipt(mut receipt: Value) -> Value {
    if let Value::Object(fields) = &mut receipt {
        for key in ["run_id", "started_at"] {
            fields.remove(key);
        }
    }
    receipt
}

fn has_inventory_row(value: &Value, section: &str, field: &str, expected: &str) -> bool {
    value
        .get("source_inventory")
        .and_then(|inventory| inventory.get(section))
        .and_then(Value::as_array)
        .is_some_and(|rows| {
            rows.iter()
                .any(|row| row.get(field).and_then(Value::as_str) == Some(expected))
        })
}

fn has_inventory_count(value: &Value, field: &str, expected: u64) -> bool {
    value
        .get("source_inventory")
        .and_then(|inventory| inventory.get(field))
        .and_then(Value::as_u64)
        == Some(expected)
}

fn has_scanned_source_file(value: &Value, expected: u64) -> bool {
    value
        .get("inventory")
        .and_then(|inventory| inventory.get("files_scanned"))
        .and_then(Value::as_u64)
        == Some(expected)
}

fn has_expected_finding(value: &Value) -> bool {
    value
        .get("findings")
        .and_then(Value::as_array)
        .is_some_and(|findings| {
            findings.iter().any(|finding| {
                finding.get("kind").and_then(Value::as_str) == Some("panic")
                    && finding.get("family").and_then(Value::as_str) == Some("unwrap")
                    && finding.get("path").and_then(Value::as_str) == Some("src/lib.rs")
            })
        })
}

struct TempGuard(std::path::PathBuf);
impl Drop for TempGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn check_persistent_cache_off_reports_finding_without_cache_io() -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-persistent-cache-off-cli-{}-{stamp}",
        std::process::id()
    ));
    let cleanup = TempGuard(root);
    fs::create_dir_all(cleanup.0.join("src")).map_err(|error| error.to_string())?;
    fs::create_dir_all(cleanup.0.join("policy")).map_err(|error| error.to_string())?;
    fs::write(
        cleanup.0.join("src/lib.rs"),
        "fn known_finding(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .map_err(|error| error.to_string())?;
    fs::write(
        cleanup.0.join("policy/allow.toml"),
        "schema_version = 1\n\n[workspace]\nignored = []\ngenerated = []\n",
    )
    .map_err(|error| error.to_string())?;
    git(&cleanup.0, &["init"])?;
    git(
        &cleanup.0,
        &["config", "user.email", "cargo-allow@example.invalid"],
    )?;
    git(&cleanup.0, &["config", "user.name", "cargo-allow test"])?;
    git(&cleanup.0, &["add", "--all"])?;
    git(&cleanup.0, &["commit", "-m", "cache off fixture"])?;

    let run = |mode: &str, receipt: &Path| -> Result<(Value, Value), String> {
        let output = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
            .args([
                "check",
                "--root",
                cleanup
                    .0
                    .to_str()
                    .ok_or_else(|| "root is not UTF-8".to_string())?,
                "--config",
                "policy/allow.toml",
                "--persistent-cache",
                mode,
                "--format",
                "json",
                "--receipt",
                receipt
                    .to_str()
                    .ok_or_else(|| "receipt is not UTF-8".to_string())?,
            ])
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() {
            return Err("known unwrap finding unexpectedly passed".to_string());
        }
        let stdout = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
        let report = serde_json::from_str(&stdout)
            .map_err(|error| format!("report JSON: {error}: {stdout}"))?;
        let receipt_bytes = fs::read(receipt).map_err(|error| format!("receipt: {error}"))?;
        let receipt = serde_json::from_slice(&receipt_bytes)
            .map_err(|error| format!("receipt JSON: {error}"))?;
        Ok((report, receipt))
    };
    let on_receipt = cleanup.0.join("receipt-on.json");
    let off_receipt = cleanup.0.join("receipt-off.json");
    let (on_report, on_receipt_json) = run("on", &on_receipt)?;
    let on_identity = has_expected_finding(&on_report);
    let on_kind = has_inventory_row(&on_report, "by_kind", "kind", "panic");
    let on_family = has_inventory_row(&on_receipt_json, "by_family", "family", "unwrap");
    let on_count = has_inventory_count(&on_report, "findings", 2);
    let on_files = has_scanned_source_file(&on_report, 2);
    if !(on_identity && on_kind && on_family && on_count && on_files) {
        return Err(format!(
            "persistent-cache on identity mismatch: identity={on_identity} kind={on_kind} family={on_family} count={on_count} files={on_files}"
        ));
    }
    if !allow_rust::ScanCacheStore::default_dir(&cleanup.0).exists() {
        return Err("persistent-cache on did not create a cache root".to_string());
    }
    fs::remove_dir_all(allow_rust::ScanCacheStore::default_dir(&cleanup.0))
        .map_err(|error| format!("remove enabled cache: {error}"))?;
    let (off_report, off_receipt_json) = run("off", &off_receipt)?;
    if !has_expected_finding(&off_report)
        || !has_inventory_row(&off_report, "by_kind", "kind", "panic")
        || !has_inventory_row(&off_receipt_json, "by_family", "family", "unwrap")
        || !has_inventory_count(&off_report, "findings", 2)
        || !has_scanned_source_file(&off_report, 2)
    {
        return Err(
            "persistent-cache off omitted the expected panic inventory identity".to_string(),
        );
    }
    if on_report != off_report {
        return Err("persistent-cache on/off reports differ".to_string());
    }
    if semantic_receipt(on_receipt_json) != semantic_receipt(off_receipt_json) {
        return Err("persistent-cache on/off semantic receipts differ".to_string());
    }
    if allow_rust::ScanCacheStore::default_dir(&cleanup.0).exists() {
        return Err("persistent-cache off created a cache root".to_string());
    }
    for args in [
        vec!["check", "--staged", "--persistent-cache", "off"],
        vec![
            "check",
            "--profile",
            "spec-system",
            "--persistent-cache",
            "off",
        ],
    ] {
        let unsupported = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
            .args(args)
            .output()
            .map_err(|error| error.to_string())?;
        if unsupported.status.success() {
            return Err(
                "persistent-cache off unexpectedly parsed on an unsupported check path".to_string(),
            );
        }
        let diagnostic = format!(
            "{}{}",
            String::from_utf8_lossy(&unsupported.stdout),
            String::from_utf8_lossy(&unsupported.stderr)
        );
        if !diagnostic.contains("supported only for source-tree checks") {
            return Err(format!(
                "unsupported cache path had unexpected diagnostic: {diagnostic}"
            ));
        }
    }
    Ok(())
}
