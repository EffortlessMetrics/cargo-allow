//! Run the actual freeze producer/consumer with intercepted provider I/O.
//! Python-only protocol checks and native Rust acceptance are separate gates;
//! a missing Python interpreter or compiled consumer is a test failure.

use std::path::Path;
use std::process::Command;

#[test]
fn qualifier_protocol_controls_run_in_ci() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("repository root is unavailable")?;
    let output = Command::new("python")
        .args(["-I", "-B", "-W", "error"])
        .arg(root.join("scripts/test-qualify-release-freeze.py"))
        .arg("-v")
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .env_remove("CARGO_REGISTRY_TOKEN")
        .output()?;
    let stderr = String::from_utf8(output.stderr)?;
    if !output.status.success() {
        return Err(format!(
            "qualifier protocol suite failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            stderr
        )
        .into());
    }
    let staging_control_ran = stderr.lines().any(|line| {
        line.starts_with("test_native_bridge_outputs_publish_only_as_a_validated_complete_set")
            && line.ends_with(" ... ok")
    });
    if !staging_control_ran {
        return Err(
            "qualifier protocol suite did not run the child-output staging controls".into(),
        );
    }
    Ok(())
}

#[test]
fn actual_qualifier_prepares_reads_back_composes_and_replays()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("repository root is unavailable")?;
    let output = Command::new("python")
        .arg("-I")
        .arg("-B")
        .arg(root.join("scripts/test-qualify-release-freeze.py"))
        .arg("--cargo-allow")
        .arg(env!("CARGO_BIN_EXE_cargo-allow"))
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .env_remove("CARGO_REGISTRY_TOKEN")
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "actual qualifier consumer failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let stdout = String::from_utf8(output.stdout)?;
    if !stdout.contains("actual prepare/readback/typed-compose/serialized-replay positive; 8 discriminating controls; zero provider mutations") {
        return Err("native qualifier acceptance did not complete its actual consumer controls".into());
    }
    if !stdout.contains("native experience: original receipts and references retained; 20 direct-row/compiled-consumer/replay controls; zero provider mutations") {
        return Err("native experience admission did not run its actual consumer controls".into());
    }
    Ok(())
}
