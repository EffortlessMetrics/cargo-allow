//! End-to-end controls for the retained campaign closeout verifier (#4717).
//!
//! Python's stdlib loopback server records complete request bodies before
//! answering the actual verifier process, runs positive and negative fixture
//! events, and proves the oracle rejects three known incorrect variants.
//! No live GitHub calls, real issue mutations, or selected release token.
#![cfg(unix)]

use std::error::Error;
use std::path::Path;
use std::process::Command;

#[test]
fn campaign_closeout_verifier_reopens_invalid_and_accepts_complete() -> Result<(), Box<dyn Error>> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let script = repo_root.join("scripts/test-campaign-closeout-request-law.py");
    let output = Command::new("python3")
        .arg(&script)
        .current_dir(&repo_root)
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .env_remove("CARGO_REGISTRY_TOKEN")
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success()
            && stderr.contains("Ran 6 tests in")
            && stderr.contains("\nOK\n"),
        "campaign closeout request-law suite failed, or collected no cases.\nstdout:\n{}\nstderr:\n{stderr}",
        String::from_utf8_lossy(&output.stdout),
    );
    Ok(())
}
