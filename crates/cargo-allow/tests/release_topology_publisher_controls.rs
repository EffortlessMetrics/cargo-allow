//! Hosted invocation control for the release topology publisher's structural
//! contract suite (#3744): the publisher's acquisition/preflight controls
//! (selected-invocation boundary, env-before-if ordering, pre-token boundary,
//! dispatch contract, checksum contract) must pass on the qualified runner
//! with no credentials and no live writes. Cfg(unix): the suite is a
//! python3/bash characterization script; hosted CI provides the runner.

#![cfg(unix)]

use std::path::Path;
use std::process::Command;

#[test]
fn release_topology_publisher_control_suite_passes() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new("python3")
        .arg(root.join("scripts/test-release-topology-publisher.py"))
        .env_remove("GITHUB_TOKEN")
        .env_remove("GITHUB_BASE_SHA")
        .current_dir(&root)
        .output()?;
    assert!(
        output.status.success(),
        "topology publisher control suite failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("publisher selected-invocation boundary"),
        "control suite must run the selected-invocation boundary; got:\n{stdout}"
    );
    Ok(())
}
