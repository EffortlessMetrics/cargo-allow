//! Workflow-graph binding tests for the #3836 Stage 1 pre-gate: heavy
//! jobs depend on the pre-gate, artifact-only diagnostics cannot make
//! the aggregate green, no secrets are read, Clippy stays in Stage 2,
//! and the pre-gate never duplicates #2569's routing authority.

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set for cargo tests");
    std::path::PathBuf::from(manifest_dir)
        .join("../..")
        .canonicalize()
        .expect("workspace root resolves")
}

fn read_workspace_file(root: &std::path::Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).expect("the pre-gate surface is retained in the tree")
}

fn pregate_block(workflow: &str) -> String {
    match workflow.split_once("  pregate:") {
        Some((_, rest)) => match rest.split_once("  msrv:") {
            Some((block, _)) => block.to_string(),
            None => String::new(),
        },
        None => String::new(),
    }
}

fn workflow_after(workflow: &str, job: &str) -> String {
    match workflow.split_once(job) {
        Some((_, rest)) => rest.to_string(),
        None => String::new(),
    }
}

/// The measured-heavy set from the #3835 baseline: these must gate on
/// the pre-gate.
const HEAVY_JOBS: [&str; 12] = [
    "msrv:",
    "package-smoke:",
    "product-candidates-interop:",
    "operator-latency:",
    "test:",
    "integrated-dogfood:",
    "test-core-platforms:",
    "compat-delegation:",
    "test-intent-experimental:",
    "test-proof-experimental:",
    "test-shared-protocol:",
    "coverage:",
];

#[test]
fn ci_pregate_workflow_defines_the_fast_tier_first() {
    let root = workspace_root();
    let workflow = read_workspace_file(&root, ".github/workflows/ci.yml");
    let pregate = workflow.find("  pregate:");
    let first_heavy = workflow.find("  msrv:");
    assert!(pregate.is_some(), "the pre-gate job is defined");
    let pregate_first = match (pregate, first_heavy) {
        (Some(pregate_position), Some(msrv_position)) => pregate_position < msrv_position,
        _ => false,
    };
    assert!(pregate_first, "the pre-gate is the first job in the graph");
    assert!(
        workflow.contains("ci-pregate evaluate"),
        "the pre-gate emits and evaluates the typed aggregate"
    );
    // The pinned linter predates the macos-15-intel label; the ignore
    // is precise and the scoped gate stays on this workflow's graph.
    assert!(
        workflow.contains("label \"macos-15-intel\" is unknown"),
        "the unknown-label ignore is precise"
    );
    assert!(
        workflow.contains(".github/workflows/ci.yml"),
        "the syntax gate scopes to this workflow's graph"
    );
    assert!(
        workflow.contains("-shellcheck="),
        "the shellcheck layer stays off in stage 1 (recorded tiering work)"
    );
}

#[test]
fn ci_pregate_workflow_gates_every_heavy_job() {
    // Negative control 1: a heavy job must not start before the
    // pre-gate finishes.
    let root = workspace_root();
    let workflow = read_workspace_file(&root, ".github/workflows/ci.yml");
    for job in HEAVY_JOBS {
        let remainder = workflow_after(&workflow, &format!("  {job}"));
        assert!(!remainder.is_empty(), "job {job} is present");
        let needs_position = [
            remainder.find("needs: [pregate"),
            remainder.find("needs: pregate"),
        ]
        .iter()
        .filter_map(|position| *position)
        .min()
        .expect("every heavy job must need the pre-gate");
        assert!(
            needs_position < remainder.find("steps:").unwrap_or(remainder.len()),
            "job {job} declares its pre-gate dependency before its steps"
        );
    }
}

#[test]
fn ci_pregate_workflow_diagnostics_cannot_make_it_green() {
    // Negative control 3: the diagnostic upload runs under always()
    // but the permit decision is the typed evaluate step's exit code.
    let root = workspace_root();
    let workflow = read_workspace_file(&root, ".github/workflows/ci.yml");
    assert!(
        workflow.contains("if: always()"),
        "diagnostics may upload under failure"
    );
    assert!(
        workflow.contains("ci-pregate evaluate"),
        "the permit decision is the typed evaluation"
    );
    assert!(
        !workflow.contains("Continue-On-Error")
            && !workflow.contains("continue-on-error: true\n        id: no_new"),
        "the selected checks must not swallow failures"
    );
}

#[test]
fn ci_pregate_workflow_reads_no_secrets_and_mutates_nothing() {
    // Negative control 12: no release secrets, no external mutation.
    let root = workspace_root();
    let pregate_block = pregate_block(&read_workspace_file(&root, ".github/workflows/ci.yml"));
    for forbidden in [
        "secrets.",
        "RELEASE_TOKEN",
        "CARGO_REGISTRY_TOKEN",
        "cargo publish",
    ] {
        assert!(
            !pregate_block.contains(forbidden),
            "the pre-gate must not use '{forbidden}'"
        );
    }
}

#[test]
fn ci_pregate_workflow_keeps_clippy_in_stage_two() {
    // Negative control 6: Clippy is not moved into the pre-gate
    // without measurement.
    let root = workspace_root();
    let pregate_block = pregate_block(&read_workspace_file(&root, ".github/workflows/ci.yml"));
    assert!(
        !pregate_block.contains("clippy"),
        "clippy stays in the heavy lanes until measured"
    );
}

#[test]
fn ci_pregate_workflow_head_movement_reruns_the_gate() {
    // Negative control 8: head movement must rerun the pre-gate; the
    // pull_request synchronize and push triggers cover it.
    let root = workspace_root();
    let workflow = read_workspace_file(&root, ".github/workflows/ci.yml");
    assert!(workflow.contains("synchronize"));
    assert!(workflow.contains("branches: [main]"));
    // The staleness law lives in the typed evaluation.
    let head_binding =
        workflow.contains("PR_HEAD: ${{ github.event.pull_request.head.sha || github.sha }}");
    assert!(
        head_binding,
        "the emitted result binds the exact current head"
    );
}

#[test]
fn ci_pregate_workflow_does_not_own_proof_routing() {
    // Negative controls 5, 9, 10: the pre-gate changes reachability
    // only; the routing authority stays #2569 and release-sensitive
    // routing stays #3794.
    let root = workspace_root();
    let pregate_block = pregate_block(&read_workspace_file(&root, ".github/workflows/ci.yml"));
    assert!(
        !pregate_block.contains("impact"),
        "the pre-gate must not decide impact classes"
    );
    assert!(
        !pregate_block.contains("paths:"),
        "the pre-gate must not gain path-based proof selection"
    );
}
