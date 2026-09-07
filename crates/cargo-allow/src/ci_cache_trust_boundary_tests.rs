//! Trust-boundary tests for the #3963 Linux cache experiment: the
//! untrusted restore-only boundary is falsified against the retained
//! action source and the inventory contract's own law, never inferred
//! from YAML wording alone.

use allow_inventory::{
    CachePostureV1, CacheRunRecordV1, CacheSaveAuthorityV1, CacheTrustClassV1,
    untrusted_save_violations, validate_run_record,
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

fn untrusted_run(save: CacheSaveAuthorityV1) -> CacheRunRecordV1 {
    CacheRunRecordV1 {
        run_id: "run-34049432706-test".to_string(),
        cache_schema_and_generation: "cargo-allow-cache-v1".to_string(),
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        base_commit: "9b62915c".to_string(),
        head_commit: "0a7d60c5".to_string(),
        workflow_ref: "pull_request (#4149)".to_string(),
        action_ref: "Swatinem/rust-cache@258712b0b7b1ddf8bddc9fc3b0faca682b2736c3".to_string(),
        runner_provider: "github-hosted".to_string(),
        runner_os: "Linux".to_string(),
        runner_arch: "X64".to_string(),
        runner_image_class: "ubuntu-latest (observed label; image not immutable)".to_string(),
        rust_toolchain: "stable (rust-cache lane default; 1.95 series)".to_string(),
        cargo_version: "not captured verbatim in this window".to_string(),
        cargo_lock_digest: "13ee17aa400eedd416d6e55103d7752d4859347b3cd160cab2ca2c53c4b574c8"
            .to_string(),
        workspace_manifest_digest: "13ee17aa400eedd416d6e55103d7752d4859347b3cd160cab2ca2c53c4b574c8"
            .to_string(),
        build_profile: "release (cargo test --locked release-set proof)".to_string(),
        selected_features: "workspace default".to_string(),
        selected_targets: "x86_64-unknown-linux-gnu".to_string(),
        proof_lane: "test (core compile/test proof)".to_string(),
        cache_lane_namespace: "release-set".to_string(),
        cache_key_identity: "cargo-allow-cache-v1-Linux-X64-stable-13ee17aa400eedd416d6e55103d7752d4859347b3cd160cab2ca2c53c4b574c8-release-set-Linux-x64-6ff13d87-e0f4ea26".to_string(),
        trust_class: CacheTrustClassV1::RepositoryPr,
        save_authority: save,
        posture: CachePostureV1::Warm,
        restore_seconds_ms: 4_000,
        compile_test_seconds_ms: 646_000,
        save_seconds_ms: 0,
        bytes_restored: Some(187_289_866),
        bytes_saved: None,
        selected_commands: vec!["cargo test --locked".to_string()],
        semantic_receipt_digest: "test-lane:0a7d60c5:passed".to_string(),
        envelope_queue_seconds_ms: 0,
        limitations: Vec::new(),
    }
}

#[test]
fn ci_cache_trust_boundary_untrusted_save_fails_run_validation() {
    // Negative control 6: a pull-request run must not publish reusable
    // state into the trusted namespace.
    let violation = untrusted_run(CacheSaveAuthorityV1::TrustedSavePermitted);
    let error = validate_run_record(&violation)
        .expect_err("an untrusted run with save authority must fail");
    assert!(error.contains("save authority") || error.contains("save-restricted"));

    // The honest shape: restore-only.
    let restore_only = untrusted_run(CacheSaveAuthorityV1::SaveRestricted);
    validate_run_record(&restore_only).expect("restore-only is the lawful untrusted posture");
    assert!(untrusted_save_violations(&[restore_only]).is_empty());
}

#[test]
fn ci_cache_trust_boundary_detector_names_the_violations() {
    let rows = vec![
        untrusted_run(CacheSaveAuthorityV1::TrustedSavePermitted),
        untrusted_run(CacheSaveAuthorityV1::SaveRestricted),
    ];
    let violations = untrusted_save_violations(&rows);
    assert_eq!(
        violations.len(),
        1,
        "only the trusted-authority PR row is a violation"
    );
    assert!(violations[0].contains("run-34049432706-test"));
}

#[test]
fn ci_cache_trust_boundary_source_restricts_saves_to_the_default_branch() {
    // The action's save condition must be an explicit expression that
    // names the default branch and only push/dispatch events — not an
    // unconditional or absent save-if. This falsifies the boundary at
    // the source instead of inferring it from intent.
    let root = workspace_root();
    let action = read_workspace_file(&root, ".github/actions/rust-cache/action.yml");
    // Bind the assertion to the ACTIVE save-if field of the
    // Swatinem/rust-cache step: a match inside a comment or an
    // unrelated string must not satisfy it. Exactly one active field
    // is allowed; missing or ambiguous matches fail.
    let active_save_if: Vec<&str> = action
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("save-if:") && !trimmed.starts_with("#")
        })
        .collect();
    assert_eq!(
        active_save_if.len(),
        1,
        "exactly one active save-if field is allowed: {active_save_if:?}"
    );
    assert!(
        active_save_if[0].contains(
            "github.ref == format('refs/heads/{0}', github.event.repository.default_branch)"
        ),
        "the save authority must stay bound to trusted default-branch runs: {active_save_if:?}"
    );
    assert!(
        active_save_if[0].contains("github.event_name == 'push'")
            && active_save_if[0].contains("github.event_name == 'workflow_dispatch'"),
        "saves must stay restricted to push/dispatch events: {active_save_if:?}"
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
fn ci_cache_trust_boundary_action_is_pinned_by_full_sha() {
    let root = workspace_root();
    let action = read_workspace_file(&root, ".github/actions/rust-cache/action.yml");
    let pin_line = action
        .lines()
        .find(|line| line.contains("Swatinem/rust-cache@"))
        .expect("the upstream action is referenced");
    let reference = pin_line
        .split('@')
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
fn ci_cache_trust_boundary_cache_consumes_no_secrets() {
    // Cache bytes must contain no credentials: the composite action
    // declares no secrets inputs and forwards no tokens.
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

#[test]
fn ci_cache_trust_boundary_retained_experiment_is_trusted_only() {
    // The retained single-run window is a trusted default-branch warm
    // run; the untrusted pull-request observation is characterized in
    // these tests (its own head state) rather than pooled across
    // source states into one namespace.
    let root = workspace_root();
    let text = read_workspace_file(&root, "docs/ci/receipts/ci-cache-experiment-v1.json");
    assert!(
        text.contains("\"trust_class\": \"trusted_default_branch\"")
            || text.contains("TrustedDefaultBranch")
    );
    assert!(
        text.contains("ci-cache-experiment-3963-retained-window"),
        "the retained receipt carries the inventory contract's experiment identity"
    );
}
