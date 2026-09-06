//! Trust-boundary tests for the #3963 Linux cache experiment: the
//! untrusted restore-only boundary is falsified against the retained
//! action source and the hosted observation, never inferred from YAML
//! wording alone.

use allow_report::{
    CiCacheExperimentV1, CiCacheLaneObservationV1, CiCachePostureV1, CiCacheTrustClassV1,
    CiCacheVerdictV1, evaluate_ci_cache_experiment,
};

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set for cargo tests");
    std::path::PathBuf::from(manifest_dir)
        .join("../..")
        .canonicalize()
        .expect("workspace root resolves")
}

fn read_workspace_file(root: &std::path::Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).expect("the cache trust surface is present in the tree")
}

fn untrusted_lane(save_authority: bool) -> CiCacheLaneObservationV1 {
    CiCacheLaneObservationV1 {
        lane: "release-set".to_string(),
        workflow: "ci.yml".to_string(),
        run_id: 34049432706,
        attempt: 1,
        head_sha: "0a7d60c5".to_string(),
        base_sha: "9b62915c".to_string(),
        runner_label: "ubuntu-latest".to_string(),
        runner_os: "Linux".to_string(),
        runner_arch: "X64".to_string(),
        toolchain: "stable".to_string(),
        lock_digest: "13ee17aa".to_string(),
        action_ref: "258712b0b7b1ddf8bddc9fc3b0faca682b2736c3".to_string(),
        key_generation: "v1".to_string(),
        trust_class: CiCacheTrustClassV1::Untrusted,
        save_authority,
        posture: CiCachePostureV1::Warm,
        lookup_seconds: None,
        restore_seconds: Some(4),
        compile_seconds: None,
        test_seconds: None,
        save_seconds: None,
        restored_bytes: Some(1024),
        saved_bytes: None,
        commands: vec!["cargo test --locked".to_string()],
        semantic_result: "passed".to_string(),
        incident: None,
    }
}

fn experiment_with(lane: CiCacheLaneObservationV1) -> CiCacheExperimentV1 {
    CiCacheExperimentV1 {
        schema_id: "cargo-allow.ci-cache-experiment.v1".to_string(),
        schema_version: 1,
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        generation: "v1".to_string(),
        baseline_ref: "#3835".to_string(),
        action_ref: "258712b0b7b1ddf8bddc9fc3b0faca682b2736c3".to_string(),
        key_generation: "v1".to_string(),
        lanes: vec![lane],
        parity: Vec::new(),
        limits: Vec::new(),
        claim_boundary: "bounded".to_string(),
    }
}

#[test]
fn ci_cache_trust_boundary_rejects_untrusted_save_authority() {
    // Negative control 6: a pull-request run must not publish reusable
    // state into the trusted namespace.
    let evaluation = evaluate_ci_cache_experiment(&experiment_with(untrusted_lane(true)));
    assert_eq!(evaluation.verdict, CiCacheVerdictV1::InstrumentFailure);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("untrusted_save: release-set"))
    );
}

#[test]
fn ci_cache_trust_boundary_is_falsified_by_the_retained_row() {
    // The retained experiment carries a hosted untrusted observation
    // with save authority absent: the boundary was exercised, not just
    // written down.
    let root = workspace_root();
    let text = read_workspace_file(&root, "docs/ci/receipts/ci-cache-experiment-v1.json");
    let experiment: CiCacheExperimentV1 =
        serde_json::from_str(&text).expect("the retained experiment parses");
    let untrusted = experiment
        .lanes
        .iter()
        .find(|lane| lane.trust_class == CiCacheTrustClassV1::Untrusted)
        .expect("the retained window exercises the untrusted boundary");
    assert!(!untrusted.save_authority);
    assert_eq!(untrusted.posture, CiCachePostureV1::Warm);
    assert!(untrusted.saved_bytes.is_none());
}

#[test]
fn ci_cache_trust_boundary_source_restricts_saves_to_the_default_branch() {
    // The action's save condition must be an explicit expression that
    // names the default branch and only push/dispatch events — not an
    // unconditional or absent save-if.
    let root = workspace_root();
    let action = read_workspace_file(&root, ".github/actions/rust-cache/action.yml");
    assert!(
        action.contains("save-if: ${{ github.ref == format('refs/heads/{0}', github.event.repository.default_branch) && (github.event_name == 'push' || github.event_name == 'workflow_dispatch') }}"),
        "the save authority must stay bound to trusted default-branch runs"
    );
    assert!(
        action.contains("prefix-key: cargo-allow-cache-v1-${{ runner.os }}-${{ runner.arch }}-${{ inputs.toolchain }}-${{ hashFiles('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml') }}"),
        "the key must bind platform, architecture, toolchain, and manifest identity"
    );
    assert!(
        action.contains("shared-key: ${{ inputs.lane }}"),
        "the namespace must be the stable per-lane input"
    );
}

#[test]
fn ci_cache_trust_boundary_lanes_are_declared_in_the_routing_source() {
    // Every rust-cache usage declares a non-empty lane namespace. A
    // namespace shared between jobs of the same proof purpose is the
    // deliberate object-sharing boundary (they build the same lock);
    // cross-toolchain or cross-manifest sharing stays impossible by
    // the prefix binding in the action itself.
    let root = workspace_root();
    let workflow = read_workspace_file(&root, ".github/workflows/ci.yml");
    let mut lanes: Vec<String> = Vec::new();
    for line in workflow.lines() {
        let trimmed = line.trim();
        if let Some(lane) = trimmed.strip_prefix("lane: ") {
            assert!(
                !lane.trim().is_empty(),
                "every cache usage declares its lane"
            );
            lanes.push(lane.trim().to_string());
        }
    }
    assert!(
        lanes.len() >= 10,
        "the cache policy covers the measured lanes: {lanes:?}"
    );
}

#[test]
fn ci_cache_trust_boundary_action_is_pinned_by_full_sha() {
    let root = workspace_root();
    let action = read_workspace_file(&root, ".github/actions/rust-cache/action.yml");
    let pin_line = action
        .lines()
        .find(|line| line.contains("Swatinem/rust-cache@"))
        .expect("the upstream action is referenced");
    let reference = pin_line
        .split("@")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .unwrap_or_default();
    assert_eq!(
        reference.len(),
        40,
        "the upstream action is pinned by full commit SHA, not a tag: {reference}"
    );
    assert!(
        reference.chars().all(|c| c.is_ascii_hexdigit()),
        "the pinned reference is a commit SHA: {reference}"
    );
}

#[test]
fn ci_cache_trust_boundary_caches_carries_no_credentials() {
    // Cache bytes must contain no credentials: the composite action
    // declares no secrets inputs and passes no secrets to the upstream
    // step.
    let root = workspace_root();
    let action = read_workspace_file(&root, ".github/actions/rust-cache/action.yml");
    assert!(
        !action.contains("secrets."),
        "the cache action must not consume repository secrets"
    );
    assert!(
        !action.contains("token:"),
        "the cache action must not mint or forward tokens"
    );
}
