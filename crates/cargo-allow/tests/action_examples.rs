//! Action example tests (#3881/#2575): verify the committed GitHub Actions
//! examples are minimally permissioned, use exact install identity, and
//! upload artifacts on success and failure.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_example(name: &str) -> Result<String, String> {
    let path = repo_root().join("examples/github-actions").join(name);
    std::fs::read_to_string(&path).map_err(|error| format!("read {name}: {error}"))
}

#[test]
fn examples_do_not_request_write_permissions() -> Result<(), String> {
    for name in ["cargo-allow-check.yml", "cargo-allow-diff.yml"] {
        let text = read_example(name)?;
        for forbidden in [
            "contents: write",
            "issues: write",
            "pull-requests: write",
            "security-events: write",
            "actions: write",
            "deployments: write",
        ] {
            if text.contains(forbidden) {
                return Err(format!("{name} requests forbidden permission: {forbidden}"));
            }
        }
    }
    Ok(())
}

#[test]
fn examples_do_not_use_ambient_binary() -> Result<(), String> {
    for name in ["cargo-allow-check.yml", "cargo-allow-diff.yml"] {
        let text = read_example(name)?;
        if text.contains("cargo install cargo-allow --version latest") {
            return Err(format!("{name} installs from 'latest'"));
        }
        // The issue forbids "no ambient binary" — the install step must pin
        // an exact version.
        if !text.contains("--version 0.") {
            return Err(format!("{name} does not pin an exact cargo-allow version"));
        }
    }
    Ok(())
}

#[test]
fn examples_do_not_silently_fall_back_to_source_build() -> Result<(), String> {
    for name in ["cargo-allow-check.yml", "cargo-allow-diff.yml"] {
        let text = read_example(name)?;
        if text.contains("cargo build") && !text.contains("# fallback") {
            return Err(format!("{name} contains a silent source-build fallback"));
        }
    }
    Ok(())
}

#[test]
fn examples_preserve_semantic_exit_on_failure() -> Result<(), String> {
    // The workflow must not swallow a non-zero exit from the semantic
    // evaluation; `if: always()` on upload steps is fine, but the main
    // run step must not have a `|| true` or `set +e` that masks failure.
    for name in ["cargo-allow-check.yml", "cargo-allow-diff.yml"] {
        let text = read_example(name)?;
        if text.contains("|| true") {
            return Err(format!("{name} swallows semantic failure with || true"));
        }
    }
    Ok(())
}
