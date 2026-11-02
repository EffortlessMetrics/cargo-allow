//! Unix producer-input controls using real Git objects and a capture-only child.
//! These do not qualify Rust delta semantics, Windows, or a release candidate.
#![cfg(unix)]

use std::path::Path;
use std::process::Command;

#[test]
fn dependency_delta_reads_the_declared_committed_inputs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new("python3")
        .arg(root.join("scripts/test-dependency-delta-inputs.py"))
        .env_remove("DELTA_SCRIPT")
        .current_dir(&root)
        .output()
        .expect("run dependency-delta Git input controls with Python 3.11+");
    assert!(
        output.status.success(),
        "dependency-delta input controls failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
