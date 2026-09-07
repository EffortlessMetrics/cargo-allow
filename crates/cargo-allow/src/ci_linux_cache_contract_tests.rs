//! Retained-experiment receipt tests for the #3963 Linux cache
//! experiment: the retained receipt is exactly `compile_experiment`
//! over the hosted observation rows, it validates under the
//! inventory contract's own law, and its honest verdict on the
//! current single-run window is NeedsMoreData.

use allow_inventory::{
    CachePostureV1, CacheRunRecordV1, CacheSaveAuthorityV1, CacheTrustClassV1, CiCacheExperimentV1,
    ExperimentVerdictV1, PINNED_RUST_CACHE_ACTION_REF, compile_experiment, validate_experiment,
    validate_run_record,
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
    std::fs::read_to_string(root.join(rel)).expect("the retained surface is present in the tree")
}

/// The hosted observation row: trusted default-branch warm run of the
/// `release-set` lane (run 34044971047, head 1e980deb, conclusion
/// success; restore step 16:28:33Z -> 16:28:38Z restored 187289866
/// bytes with `full match: true`).
fn retained_row() -> CacheRunRecordV1 {
    CacheRunRecordV1 {
        run_id: "run-34044971047-test".to_string(),
        cache_schema_and_generation: "cargo-allow-cache-v1".to_string(),
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        base_commit: "9b62915c".to_string(),
        head_commit: "1e980deb".to_string(),
        workflow_ref: "refs/heads/main (ci.yml push)".to_string(),
        action_ref: PINNED_RUST_CACHE_ACTION_REF.to_string(),
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
        trust_class: CacheTrustClassV1::TrustedDefaultBranch,
        save_authority: CacheSaveAuthorityV1::TrustedSavePermitted,
        posture: CachePostureV1::Warm,
        restore_seconds_ms: 5_000,
        compile_test_seconds_ms: 646_000,
        save_seconds_ms: 0,
        bytes_restored: Some(187_289_866),
        bytes_saved: None,
        selected_commands: vec![
            "cargo test -p allow-report -p allow-diff --locked --all-features".to_string(),
            "cargo test -p cargo-allow --locked".to_string(),
            "cargo run -p cargo-allow -- check --mode no-new".to_string(),
        ],
        semantic_receipt_digest: "test-lane:1e980deb:passed".to_string(),
        envelope_queue_seconds_ms: 0,
        limitations: Vec::new(),
    }
}

fn recompiled() -> CiCacheExperimentV1 {
    compile_experiment(
        &[retained_row()],
        "ci-cache-experiment-3963-retained-window",
        "#3835 retained baseline (docs/ci/receipts/ci-performance-baseline-v1.json)",
    )
    .expect("the retained rows compile under the inventory contract")
}

#[test]
fn ci_linux_cache_contract_retained_receipt_is_the_law_compiled_experiment() {
    let root = workspace_root();
    let text = read_workspace_file(&root, "docs/ci/receipts/ci-cache-experiment-v1.json");
    let retained: CiCacheExperimentV1 =
        serde_json::from_str(&text).expect("the retained receipt parses");
    let expected = recompiled();
    assert_eq!(retained, expected);
    validate_experiment(&retained).expect("the retained receipt validates");
}

#[test]
fn ci_linux_cache_contract_retained_verdict_is_honest_needs_more_data() {
    let experiment = recompiled();
    assert_eq!(
        experiment.verdict,
        ExperimentVerdictV1::NeedsMoreData,
        "a single warm observation is not an acceptance result"
    );
    assert!(
        experiment
            .verdict_reasons
            .iter()
            .any(|reason| reason.contains("missing required postures")),
        "the exact missing postures are named: {:?}",
        experiment.verdict_reasons
    );
    assert!(
        experiment
            .verdict_reasons
            .iter()
            .any(|reason| reason.contains("at least 2 are required")),
        "the thin warm distribution is named"
    );
    // The rollback route and the attribution guard note are carried.
    assert!(experiment.rollback_route.contains("#3835"));
    assert!(
        experiment
            .improvement_attribution_note
            .contains("compile_test_seconds_ms")
    );
}

#[test]
fn ci_linux_cache_contract_warm_carries_hit_evidence_not_action_presence() {
    // Negative control 1: the warm row carries restored bytes and the
    // full-match restore; action presence alone is never the evidence.
    let row = retained_row();
    assert_eq!(row.posture, CachePostureV1::Warm);
    assert_eq!(
        row.bytes_restored,
        Some(187_289_866),
        "the full-match restore size is retained"
    );
    assert!(row.restore_seconds_ms > 0);
    assert!(row.compile_test_seconds_ms > 0, "a real selected run");
    assert!(!row.selected_commands.is_empty());
}

#[test]
fn ci_linux_cache_contract_replayed_run_cannot_double_count() {
    // Negative control: replaying a row must fail compilation.
    let row = retained_row();
    // A true replay keeps the same run identity: only the attempt
    // number would differ, and the contract rejects duplicate run_ids
    // before any count or percentile can double-count the row.
    let duplicate = row.clone();
    let error = compile_experiment(&[row, duplicate], "replay", "#3835")
        .expect_err("a replayed warm row cannot satisfy the two-warm law");
    assert!(error.contains("duplicate run_id"), "{error}");
}

#[test]
fn ci_linux_cache_contract_zero_duration_run_fails_validation() {
    let mut zero = retained_row();
    zero.run_id = "run-zero".to_string();
    zero.compile_test_seconds_ms = 0;
    assert!(
        validate_run_record(&zero)
            .err()
            .is_some_and(|error| error.contains("not a real selected run"))
    );
}

#[test]
fn ci_linux_cache_contract_schema_binds_the_inventory_authority() {
    // One schema ID names one format: the inventory crate owns
    // cargo-allow.ci-cache-experiment.v1; this receipt is its shape.
    let experiment = recompiled();
    assert_eq!(experiment.schema_id, "cargo-allow.ci-cache-experiment.v1");
    assert_eq!(experiment.schema_version, 1);
    assert!(
        experiment
            .claim_boundary
            .contains("never product, package, release, or proof identity")
    );
}

#[test]
fn ci_linux_cache_contract_json_view_parses_back() {
    let experiment = recompiled();
    let json = allow_inventory::render_ci_cache_experiment_v1(&experiment)
        .expect("serialization succeeds");
    let roundtrip: CiCacheExperimentV1 =
        serde_json::from_str(json.as_str()).expect("the JSON view parses back");
    assert_eq!(roundtrip, experiment);
}
