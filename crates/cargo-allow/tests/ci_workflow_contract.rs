//! Offline CI workflow contract (#2355).
//!
//! Proves committed GitHub Actions examples and the Run-in-CI how-to agree on
//! install pins, PR base-history strategy, always-on artifact upload, and a
//! blocking gate (no continue-on-error). Does not run hosted workflows.

use std::path::{Path, PathBuf};

const RUN_IN_CI: &str = include_str!("../../../docs/how-to/run-in-ci.md");
const TROUBLESHOOT: &str = include_str!("../../../docs/how-to/troubleshoot-cargo-allow.md");
const ROLLBACK: &str = include_str!("../../../docs/how-to/rollback-cargo-allow-adoption.md");
const CI_DOC: &str = include_str!("../../../docs/ci.md");
const DIFF_WORKFLOW: &str = include_str!("../../../examples/github-actions/cargo-allow-diff.yml");
const CHECK_WORKFLOW: &str = include_str!("../../../examples/github-actions/cargo-allow-check.yml");
const SHALLOW_NEGATIVE: &str =
    include_str!("../../../docs/dogfood/fixtures/ci/shallow-checkout-missing-base.yml");
const PARTIAL_DIFF_ARTIFACTS: &str =
    include_str!("../../../docs/dogfood/fixtures/ci/partial-diff-artifacts.yml");
const PUBLISHED_REGISTRY: &str =
    include_str!("../../../docs/dogfood/fixtures/getting-started/published-command-registry.toml");

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| std::panic::panic_any("cargo-allow manifest should be under crates/"))
        .to_path_buf()
}

fn normalize_lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn assert_yaml_workflow_shape(label: &str, body: &str) {
    let text = normalize_lf(body);
    assert!(
        text.lines()
            .any(|line| line.trim_start().starts_with("name:")),
        "{label} must declare name:"
    );
    assert!(
        text.lines()
            .any(|line| line.trim_start() == "on:" || line.starts_with("on:")),
        "{label} must declare on:"
    );
    assert!(
        text.lines()
            .any(|line| line.trim_start() == "jobs:" || line.starts_with("jobs:")),
        "{label} must declare jobs:"
    );
    assert!(
        text.lines().any(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("runs-on:") || trimmed.starts_with("- uses:")
        }),
        "{label} must declare a job runner or uses step"
    );
    for (idx, line) in text.lines().enumerate() {
        assert!(
            !line.contains('\t'),
            "{label}:{} must not use TAB indentation",
            idx.saturating_add(1)
        );
    }
}

fn published_subcommands() -> Vec<&'static str> {
    let marker = "subcommands = [";
    let start = PUBLISHED_REGISTRY
        .find(marker)
        .unwrap_or_else(|| std::panic::panic_any("registry missing subcommands"));
    let after = PUBLISHED_REGISTRY
        .get(start.saturating_add(marker.len())..)
        .unwrap_or_else(|| std::panic::panic_any("registry slice"));
    let end = after
        .find(']')
        .unwrap_or_else(|| std::panic::panic_any("registry list end"));
    let list = after
        .get(..end)
        .unwrap_or_else(|| std::panic::panic_any("registry list body"));
    let mut cmds = Vec::new();
    for line in list.lines() {
        let trimmed = line.trim().trim_end_matches(',');
        if let Some(value) = trimmed
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            && !value.is_empty()
        {
            cmds.push(value);
        }
    }
    cmds
}

#[test]
fn run_in_ci_links_complete_workflow_examples() {
    for required in [
        "examples/github-actions/cargo-allow-diff.yml",
        "examples/github-actions/cargo-allow-check.yml",
        "fetch-depth: 0",
        "if: always()",
        "continue-on-error",
        "cargo install cargo-allow --version 0.1.11 --locked",
        "troubleshoot-cargo-allow.md",
        "rollback-cargo-allow-adoption.md",
        "error-codes.md",
        "human",
        "markdown",
        "json",
        "html",
        "sarif",
        "Exit `1`",
        "Exit `2`",
    ] {
        assert!(
            RUN_IN_CI.contains(required),
            "run-in-ci.md must document `{required}`"
        );
    }
    assert!(
        RUN_IN_CI.contains("Do **not** set `continue-on-error`")
            || RUN_IN_CI.contains("no `continue-on-error`"),
        "run-in-ci.md must forbid continue-on-error on the gate"
    );
}

#[test]
fn troubleshooting_and_rollback_guides_cover_ops_contract() {
    for required in [
        "no policy config found",
        "fetch-depth: 0",
        "Exit `2`",
        "Exit `1`",
        "location-drift",
        "baseline_debt",
        "cargo-allow doctor",
        "shipped in Published",
    ] {
        assert!(
            TROUBLESHOOT.contains(required),
            "troubleshoot guide missing `{required}`"
        );
    }
    for required in [
        "cargo uninstall cargo-allow",
        "policy/allow.toml",
        "target/cargo-allow/",
        ".allow/",
        "Tool uninstall",
        "Policy rollback",
        "unrelated",
    ] {
        assert!(
            ROLLBACK.contains(required),
            "rollback guide missing `{required}`"
        );
    }
}

#[test]
fn committed_workflow_examples_parse_and_meet_semantic_contract() {
    assert_yaml_workflow_shape("cargo-allow-diff.yml", DIFF_WORKFLOW);
    assert_yaml_workflow_shape("cargo-allow-check.yml", CHECK_WORKFLOW);
    assert_yaml_workflow_shape("shallow-checkout-missing-base.yml", SHALLOW_NEGATIVE);
    assert_yaml_workflow_shape("partial-diff-artifacts.yml", PARTIAL_DIFF_ARTIFACTS);

    assert!(
        DIFF_WORKFLOW.contains("fetch-depth: 0"),
        "PR diff example must set fetch-depth: 0"
    );
    assert!(
        !SHALLOW_NEGATIVE.contains("fetch-depth: 0"),
        "shallow negative fixture must omit fetch-depth: 0"
    );
    assert!(
        DIFF_WORKFLOW.contains("cargo install cargo-allow --version 0.1.11 --locked"),
        "PR example must pin Published 0.1.11"
    );
    assert!(
        CHECK_WORKFLOW.contains("cargo install cargo-allow --version 0.1.11 --locked"),
        "mainline example must pin Published 0.1.11"
    );
    assert!(
        DIFF_WORKFLOW.contains("upload-artifact") && DIFF_WORKFLOW.contains("if: always()"),
        "PR example must upload artifacts under if: always()"
    );
    assert!(
        CHECK_WORKFLOW.contains("upload-artifact") && CHECK_WORKFLOW.contains("if: always()"),
        "mainline example must upload artifacts under if: always()"
    );
    assert!(
        !DIFF_WORKFLOW.contains("continue-on-error"),
        "PR example must not set continue-on-error"
    );
    assert!(
        !CHECK_WORKFLOW.contains("continue-on-error"),
        "mainline example must not set continue-on-error"
    );
    assert!(
        DIFF_WORKFLOW.contains("cargo-allow diff") && DIFF_WORKFLOW.contains("--base"),
        "PR example must run cargo-allow diff --base"
    );
    assert!(
        CHECK_WORKFLOW.contains("cargo-allow check") && CHECK_WORKFLOW.contains("--mode no-new"),
        "mainline example must run check --mode no-new"
    );
    assert!(
        CHECK_WORKFLOW.contains("target/cargo-allow/"),
        "mainline example must write under target/cargo-allow/"
    );
    assert!(
        DIFF_WORKFLOW.contains("target/cargo-allow/"),
        "PR example must write under target/cargo-allow/"
    );
    for artifact in [
        "pr-summary.md",
        "diff.json",
        "diff.receipt.json",
        "diff.sarif",
    ] {
        assert!(
            DIFF_WORKFLOW.contains(artifact),
            "PR example must retain `{artifact}` when diff is non-clean"
        );
        assert!(
            PARTIAL_DIFF_ARTIFACTS.contains(artifact),
            "partial workflow fixture must declare `{artifact}`"
        );
    }
    for required in [
        "set +e",
        "markdown_status=$?",
        "json_status=$?",
        "sarif_status=$?",
        "set -e",
        "exit \"$final_status\"",
    ] {
        assert!(
            DIFF_WORKFLOW.contains(required),
            "PR example must preserve the blocking artifact sequence `{required}`"
        );
    }
    for required in [
        "expected_diff_exit: 1",
        "expected_job_posture: failure",
        "expected_upload_condition: always",
        "improvements: 0",
        "removed: 0",
    ] {
        assert!(
            PARTIAL_DIFF_ARTIFACTS.contains(required),
            "partial workflow fixture must preserve `{required}`"
        );
    }
}

#[test]
fn ci_surfaces_exist_on_disk_and_stay_in_published_command_set() {
    let root = workspace_root();
    for relative in [
        "examples/github-actions/cargo-allow-diff.yml",
        "examples/github-actions/cargo-allow-check.yml",
        "docs/how-to/run-in-ci.md",
        "docs/how-to/troubleshoot-cargo-allow.md",
        "docs/how-to/rollback-cargo-allow-adoption.md",
        "docs/dogfood/fixtures/ci/shallow-checkout-missing-base.yml",
        "docs/dogfood/fixtures/ci/partial-diff-artifacts.yml",
    ] {
        assert!(
            root.join(relative).is_file(),
            "expected committed path {relative}"
        );
    }

    let published: std::collections::BTreeSet<&str> = published_subcommands().into_iter().collect();
    let ci_bodies = [DIFF_WORKFLOW, CHECK_WORKFLOW, RUN_IN_CI, CI_DOC];
    for body in ci_bodies {
        for line in body.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#')
                || trimmed.starts_with("- name:")
                || trimmed.starts_with("name:")
            {
                continue;
            }
            let flat = line.replace('`', " ");
            if let Some(idx) = flat.find("cargo-allow ") {
                let after = flat
                    .get(idx.saturating_add("cargo-allow ".len())..)
                    .unwrap_or("");
                let token = after.split_whitespace().next().unwrap_or("");
                if token.is_empty() || token.starts_with('-') || token == "…" || token == "..." {
                    continue;
                }
                if !token.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
                    continue;
                }
                if token == "why" {
                    std::panic::panic_any(format!(
                        "Published CI surfaces must not teach `why` without a separate candidate guide: {line}"
                    ));
                }
                if !published.contains(token) {
                    // Prose such as "Use cargo-allow in CI" is not a subcommand teaching.
                    continue;
                }
            }
        }
    }
}

#[test]
fn ci_doc_points_at_ops_how_tos_and_base_history() {
    for required in [
        "how-to/run-in-ci.md",
        "how-to/troubleshoot-cargo-allow.md",
        "how-to/rollback-cargo-allow-adoption.md",
        "fetch-depth: 0",
    ] {
        assert!(
            CI_DOC.contains(required),
            "docs/ci.md must reference `{required}`"
        );
    }
}

#[test]
fn run_in_ci_references_hosted_shallow_diff_smoke() {
    assert!(
        RUN_IN_CI.contains("shallow-diff-base-smoke.sh"),
        "run-in-ci.md should point at the hosted shallow-diff characterization script"
    );
    let root = workspace_root();
    assert!(
        root.join("scripts/shallow-diff-base-smoke.sh").is_file(),
        "shallow-diff-base-smoke.sh must exist"
    );
    assert!(
        root.join("scripts/test-shallow-diff-base-smoke.sh")
            .is_file(),
        "test-shallow-diff-base-smoke.sh must exist"
    );
    let workflow = normalize_lf(include_str!("../../../.github/workflows/ci.yml"));
    assert!(
        workflow.contains("shallow-diff-smoke:"),
        "ci.yml must define shallow-diff-smoke job"
    );
    assert!(
        workflow.contains("scripts/shallow-diff-base-smoke.sh"),
        "ci.yml must run the shallow-diff script"
    );
}
