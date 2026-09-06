//! Typed contract tests for the #3963 Linux cache experiment: the
//! identity and compatibility law, the hit-vs-presence law, the
//! verdict derivation, and the retained experiment receipt.

use allow_report::{
    CI_CACHE_EXPERIMENT_SCHEMA_ID, CiCacheExperimentV1, CiCacheLaneObservationV1, CiCachePostureV1,
    CiCacheTrustClassV1, CiCacheVerdictV1, evaluate_ci_cache_experiment,
    render_ci_cache_verdict_human, render_ci_cache_verdict_json,
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

const PIN: &str = "258712b0b7b1ddf8bddc9fc3b0faca682b2736c3";

fn lane(
    name: &str,
    trust: CiCacheTrustClassV1,
    save: bool,
    posture: CiCachePostureV1,
    restored_bytes: Option<u64>,
) -> CiCacheLaneObservationV1 {
    CiCacheLaneObservationV1 {
        lane: name.to_string(),
        workflow: "ci.yml".to_string(),
        run_id: 1,
        attempt: 1,
        head_sha: "head".to_string(),
        base_sha: "base".to_string(),
        runner_label: "ubuntu-latest".to_string(),
        runner_os: "Linux".to_string(),
        runner_arch: "X64".to_string(),
        toolchain: "stable".to_string(),
        lock_digest: "13ee17aa".to_string(),
        action_ref: PIN.to_string(),
        key_generation: "v1".to_string(),
        trust_class: trust,
        save_authority: save,
        posture,
        lookup_seconds: None,
        restore_seconds: Some(4),
        compile_seconds: None,
        test_seconds: None,
        save_seconds: None,
        restored_bytes,
        saved_bytes: None,
        commands: vec!["cargo test --locked".to_string()],
        semantic_result: "passed".to_string(),
        incident: None,
    }
}

fn experiment(lanes: Vec<CiCacheLaneObservationV1>) -> CiCacheExperimentV1 {
    CiCacheExperimentV1 {
        schema_id: CI_CACHE_EXPERIMENT_SCHEMA_ID.to_string(),
        schema_version: 1,
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        generation: "v1".to_string(),
        baseline_ref: "#3835".to_string(),
        action_ref: PIN.to_string(),
        key_generation: "v1".to_string(),
        lanes,
        parity: Vec::new(),
        limits: Vec::new(),
        claim_boundary: "bounded".to_string(),
    }
}

#[test]
fn ci_linux_cache_contract_loads_the_retained_experiment() {
    let root = workspace_root();
    let text = read_workspace_file(&root, "docs/ci/receipts/ci-cache-experiment-v1.json");
    let experiment: CiCacheExperimentV1 =
        serde_json::from_str(&text).expect("the retained experiment parses");
    assert_eq!(experiment.generation, "v1");
    assert_eq!(experiment.action_ref, PIN);
    assert_eq!(experiment.lanes.len(), 4);
    // Three trusted main-run warms plus one untrusted restore-only PR
    // warm: the retained window.
    let warm_trusted = experiment
        .lanes
        .iter()
        .filter(|lane| {
            lane.posture == CiCachePostureV1::Warm
                && lane.trust_class == CiCacheTrustClassV1::Trusted
        })
        .count();
    assert_eq!(warm_trusted, 3);
    let untrusted = experiment
        .lanes
        .iter()
        .find(|lane| lane.trust_class == CiCacheTrustClassV1::Untrusted)
        .expect("the retained experiment carries an untrusted row");
    assert!(
        !untrusted.save_authority,
        "the pull-request run restored without save authority"
    );
    for lane in &experiment.lanes {
        // Warm postures carry restored-byte evidence, never action
        // presence alone.
        assert!(lane.restored_bytes.is_some_and(|bytes| bytes > 0));
        // The proof commands ran under the cache; a hit never skips.
        assert!(!lane.commands.is_empty());
    }
}

#[test]
fn ci_linux_cache_contract_rejects_action_presence_as_hit() {
    // Negative control 1: a reuse claim without restored bytes fails.
    let receipt = experiment(vec![lane(
        "release-set",
        CiCacheTrustClassV1::Trusted,
        true,
        CiCachePostureV1::Warm,
        None,
    )]);
    let evaluation = evaluate_ci_cache_experiment(&receipt);
    assert_eq!(evaluation.verdict, CiCacheVerdictV1::InstrumentFailure);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("hit_without_restored_bytes"))
    );
}

#[test]
fn ci_linux_cache_contract_rejects_unpinned_actions_and_generation_drift() {
    let mut unpinned = experiment(vec![lane(
        "release-set",
        CiCacheTrustClassV1::Trusted,
        true,
        CiCachePostureV1::Warm,
        Some(1024),
    )]);
    unpinned.action_ref = "Swatinem/rust-cache@v2".to_string();
    let evaluation = evaluate_ci_cache_experiment(&unpinned);
    assert_eq!(evaluation.verdict, CiCacheVerdictV1::InstrumentFailure);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("cache_action_unpinned"))
    );

    let mut drifted = experiment(vec![lane(
        "release-set",
        CiCacheTrustClassV1::Trusted,
        true,
        CiCachePostureV1::Warm,
        Some(1024),
    )]);
    drifted.key_generation = "v2".to_string();
    let evaluation = evaluate_ci_cache_experiment(&drifted);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("key_generation_mismatch"))
    );
}

#[test]
fn ci_linux_cache_contract_allows_repeated_observations_of_one_lane() {
    // Repeated runs of one lane legitimately share its namespace:
    // that is how the warm distribution accumulates.
    let receipt = experiment(vec![
        lane(
            "release-set",
            CiCacheTrustClassV1::Trusted,
            true,
            CiCachePostureV1::Warm,
            Some(1024),
        ),
        lane(
            "release-set",
            CiCacheTrustClassV1::Trusted,
            true,
            CiCachePostureV1::Warm,
            Some(1024),
        ),
    ]);
    let evaluation = evaluate_ci_cache_experiment(&receipt);
    assert!(
        !evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("shared_namespace"))
    );
}

#[test]
fn ci_linux_cache_contract_demands_warm_lock_identity() {
    // Negative control 4: warm state without the lock identity that
    // made it compatible cannot be restored as current.
    let mut warm = lane(
        "release-set",
        CiCacheTrustClassV1::Trusted,
        true,
        CiCachePostureV1::Warm,
        Some(1024),
    );
    warm.lock_digest = String::new();
    let evaluation = evaluate_ci_cache_experiment(&experiment(vec![warm]));
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("warm_without_lock_identity"))
    );
}

#[test]
fn ci_linux_cache_contract_needs_more_data_without_coverage() {
    // The retained window: warm-only. Cold, disabled, and fallback
    // coverage is missing, so the honest verdict is NeedsMoreData with
    // the exact missing rows.
    let receipt = experiment(vec![
        lane(
            "release-set",
            CiCacheTrustClassV1::Trusted,
            true,
            CiCachePostureV1::Warm,
            Some(1024),
        ),
        lane(
            "release-set",
            CiCacheTrustClassV1::Trusted,
            true,
            CiCachePostureV1::Warm,
            Some(1024),
        ),
        lane(
            "release-set",
            CiCacheTrustClassV1::Untrusted,
            false,
            CiCachePostureV1::Warm,
            Some(1024),
        ),
    ]);
    let evaluation = evaluate_ci_cache_experiment(&receipt);
    assert_eq!(evaluation.verdict, CiCacheVerdictV1::NeedsMoreData);
    for missing in ["cold", "disabled", "fallback"] {
        assert!(
            evaluation
                .reasons
                .iter()
                .any(|reason| reason.contains(&format!("no {missing} posture"))),
            "missing-coverage reason for {missing} must be named"
        );
    }
    // The rollback route is always stated for a non-accepted verdict.
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("rollback route"))
    );
}

#[test]
fn ci_linux_cache_contract_accepts_only_a_full_evidence_set() {
    let mut receipt = experiment(Vec::new());
    let mut lanes = Vec::new();
    // Two warms of one lane satisfy the distribution law; the
    // remaining postures are distinct coverage rows.
    let postures = [
        (
            "release-set",
            CiCachePostureV1::Cold,
            CiCacheTrustClassV1::Trusted,
            true,
        ),
        (
            "release-set",
            CiCachePostureV1::Warm,
            CiCacheTrustClassV1::Trusted,
            true,
        ),
        (
            "release-set",
            CiCachePostureV1::Warm,
            CiCacheTrustClassV1::Trusted,
            true,
        ),
        (
            "dogfood",
            CiCachePostureV1::Disabled,
            CiCacheTrustClassV1::Trusted,
            true,
        ),
        (
            "dogfood",
            CiCachePostureV1::Fallback,
            CiCacheTrustClassV1::Trusted,
            true,
        ),
        (
            "release-set-pr",
            CiCachePostureV1::Warm,
            CiCacheTrustClassV1::Untrusted,
            false,
        ),
        (
            "release-set-pr",
            CiCachePostureV1::Warm,
            CiCacheTrustClassV1::Untrusted,
            false,
        ),
    ];
    for (name, posture, trust, save) in postures.iter() {
        lanes.push(lane(name, *trust, *save, *posture, Some(1024)));
    }
    receipt.lanes = lanes;
    receipt.parity = vec![allow_report::CiCacheParityRowV1 {
        lane: "release-set".to_string(),
        compared_postures: vec![
            "cold".to_string(),
            "warm".to_string(),
            "disabled".to_string(),
            "fallback".to_string(),
        ],
        semantic_results_equal: true,
    }];
    let evaluation = evaluate_ci_cache_experiment(&receipt);
    assert_eq!(evaluation.verdict, CiCacheVerdictV1::Accepted);
    assert!(evaluation.reasons.is_empty());
}

#[test]
fn ci_linux_cache_contract_views_derive_from_one_result() {
    let evaluation = evaluate_ci_cache_experiment(&experiment(vec![lane(
        "release-set",
        CiCacheTrustClassV1::Trusted,
        true,
        CiCachePostureV1::Warm,
        Some(1024),
    )]));
    let json = render_ci_cache_verdict_json(&evaluation).expect("serialization succeeds");
    let roundtrip: allow_report::CiCacheVerdictEvaluationV1 =
        serde_json::from_str(json.as_str()).expect("the JSON view parses back");
    assert_eq!(roundtrip, evaluation);
    let human = render_ci_cache_verdict_human(&evaluation);
    assert!(human.contains("verdict="));
    assert!(human.contains("claim boundary:"));
}
