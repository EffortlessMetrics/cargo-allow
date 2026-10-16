//! Workflow example tests (#3881): verify the committed GitHub Actions
//! examples use single-evaluation artifact-set invocation, not repeated
//! scans per format.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_example(name: &str) -> Result<String, String> {
    let path = repo_root().join("examples/github-actions").join(name);
    std::fs::read_to_string(&path).map_err(|error| format!("read {name}: {error}"))
}

#[test]
fn mainline_example_uses_single_evaluation() -> Result<(), String> {
    let text = read_example("cargo-allow-check.yml")?;
    // Must use --artifact-dir and --emit (single invocation).
    if !text.contains("--artifact-dir") {
        return Err("mainline example missing --artifact-dir".to_string());
    }
    if !text.contains("--emit") {
        return Err("mainline example missing --emit".to_string());
    }
    // Must NOT run the same command once per format (the old repeated-scan pattern).
    let scan_count =
        text.matches("cargo-allow audit").count() + text.matches("cargo-allow check --").count();
    if scan_count > 1 {
        return Err(format!(
            "mainline example runs {scan_count} semantic invocations; expected 1"
        ));
    }
    Ok(())
}

#[test]
fn pr_example_uses_single_evaluation() -> Result<(), String> {
    let text = read_example("cargo-allow-diff.yml")?;
    if !text.contains("--artifact-dir") {
        return Err("PR example missing --artifact-dir".to_string());
    }
    if !text.contains("--emit") {
        return Err("PR example missing --emit".to_string());
    }
    // The old pattern manually arbitrates three exit codes.
    if text.contains("markdown_status")
        || text.contains("json_status")
        || text.contains("sarif_status")
    {
        return Err("PR example still uses manual exit-code arbitration".to_string());
    }
    Ok(())
}

#[test]
fn both_examples_have_exact_install_identity() -> Result<(), String> {
    for name in ["cargo-allow-check.yml", "cargo-allow-diff.yml"] {
        let text = read_example(name)?;
        // #4567: while no published build carries the taught 0.2
        // artifact-emission surface, the examples pin an exact
        // source-candidate revision; once 0.2.0 publishes they return to an
        // exact `--version 0.` pin. Either way the install is exact —
        // never floating.
        let pins_published_version = text.contains("cargo install cargo-allow --version 0.");
        let pins_source_candidate =
            text.contains("--rev 65d76b1494656904c8ce30cf518f2e5bf85a6911");
        if !pins_published_version && !pins_source_candidate {
            return Err(format!("{name} uses a floating install"));
        }
        if text.contains("--version latest") {
            return Err(format!("{name} uses a floating 'latest' version"));
        }
    }
    Ok(())
}

#[test]
fn both_examples_upload_artifacts_on_success_and_failure() -> Result<(), String> {
    for name in ["cargo-allow-check.yml", "cargo-allow-diff.yml"] {
        let text = read_example(name)?;
        if !text.contains("if: always()") {
            return Err(format!("{name} does not upload artifacts on failure"));
        }
        if !text.contains("actions/upload-artifact") {
            return Err(format!("{name} missing artifact upload"));
        }
    }
    Ok(())
}

#[test]
fn both_examples_have_minimal_permissions() -> Result<(), String> {
    for name in ["cargo-allow-check.yml", "cargo-allow-diff.yml"] {
        let text = read_example(name)?;
        if text.contains("contents: write") {
            return Err(format!("{name} requests contents: write"));
        }
        if text.contains("security-events: write") {
            return Err(format!("{name} requests security-events: write"));
        }
        if text.contains("pull-requests: write") {
            return Err(format!("{name} requests pull-requests: write"));
        }
    }
    Ok(())
}
