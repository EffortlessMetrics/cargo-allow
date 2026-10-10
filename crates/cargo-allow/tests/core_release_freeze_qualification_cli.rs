//! Run the actual freeze producer/consumer with intercepted provider I/O.
//! Python-only protocol checks and native Rust acceptance are separate gates;
//! a missing Python interpreter or compiled consumer is a test failure.

use std::path::Path;
use std::process::Command;

#[test]
fn actual_qualifier_prepares_reads_back_composes_and_replays() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(Path::parent)
        .ok_or("repository root is unavailable")?;
    let output = Command::new("python")
        .arg("-I").arg("-B")
        .arg(root.join("scripts/test-qualify-release-freeze.py"))
        .arg("--cargo-allow").arg(env!("CARGO_BIN_EXE_cargo-allow"))
        .env_remove("GH_TOKEN").env_remove("GITHUB_TOKEN").env_remove("CARGO_REGISTRY_TOKEN")
        .output()?;
    if !output.status.success() {
        return Err(format!("actual qualifier consumer failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)).into());
    }
    let stdout = String::from_utf8(output.stdout)?;
    if !stdout.contains("actual prepare/readback/typed-compose/serialized-replay positive; 8 discriminating controls; zero provider mutations") {
        return Err("native qualifier acceptance did not complete its actual consumer controls".into());
    }
    Ok(())
}
