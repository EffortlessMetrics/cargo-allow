//! Hosted invocation control for the agent-context structural contract suite
//! (#3731): the Gemini CLI context and agent operating surface contract
//! (settings keys, negative controls for forbidden keys, ignore rules,
//! campaign view rows) must pass on the qualified runner. Cfg(unix): the
//! suite is a python3 characterization script; hosted CI provides the runner.

#![cfg(unix)]

use std::path::Path;
use std::process::Command;

#[test]
fn campaign_agent_context_contract_suite_passes() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new("python3")
        .arg(root.join("scripts/test-agent-context-contract.py"))
        .env_remove("GEMINI_API_KEY")
        .env_remove("GITHUB_TOKEN")
        .current_dir(&root)
        .output()?;
    assert!(
        output.status.success(),
        "agent context contract suite failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("agent context contract tests: passed"),
        "contract suite must report its pass marker; got:\n{stdout}"
    );
    Ok(())
}
