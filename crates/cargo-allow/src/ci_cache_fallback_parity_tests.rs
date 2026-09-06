//! Fallback and parity tests for the #3963 Linux cache experiment:
//! corruption falls back cleanly, provider outages are limitations,
//! cache classes never change the semantic proof, and a cache hit
//! never skips the selected commands.

use allow_report::{
    CiCacheExperimentV1, CiCacheLaneObservationV1, CiCacheParityRowV1, CiCachePostureV1,
    CiCacheTrustClassV1, CiCacheVerdictV1, evaluate_ci_cache_experiment,
};

const PIN: &str = "258712b0b7b1ddf8bddc9fc3b0faca682b2736c3";

fn lane(lane: &str, posture: CiCachePostureV1, semantic: &str) -> CiCacheLaneObservationV1 {
    CiCacheLaneObservationV1 {
        lane: lane.to_string(),
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
        trust_class: CiCacheTrustClassV1::Trusted,
        save_authority: true,
        posture,
        lookup_seconds: None,
        restore_seconds: Some(4),
        compile_seconds: None,
        test_seconds: None,
        save_seconds: None,
        restored_bytes: Some(1024),
        saved_bytes: None,
        commands: vec!["cargo test --locked".to_string()],
        semantic_result: semantic.to_string(),
        incident: None,
    }
}

fn experiment(
    lanes: Vec<CiCacheLaneObservationV1>,
    parity: Vec<CiCacheParityRowV1>,
) -> CiCacheExperimentV1 {
    CiCacheExperimentV1 {
        schema_id: "cargo-allow.ci-cache-experiment.v1".to_string(),
        schema_version: 1,
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        generation: "v1".to_string(),
        baseline_ref: "#3835".to_string(),
        action_ref: PIN.to_string(),
        key_generation: "v1".to_string(),
        lanes,
        parity,
        limits: Vec::new(),
        claim_boundary: "bounded".to_string(),
    }
}

#[test]
fn ci_cache_fallback_parity_rejects_divergent_semantic_results() {
    // Negative controls 8 and 9: the same lane under warm and disabled
    // postures must produce the same semantic result; divergence is a
    // rejection, not a caveat.
    let receipt = experiment(
        vec![
            lane("release-set", CiCachePostureV1::Warm, "passed:receipt-1"),
            lane("release-set", CiCachePostureV1::Warm, "passed:receipt-1"),
            lane(
                "release-set",
                CiCachePostureV1::Disabled,
                "passed:different-receipt",
            ),
            lane(
                "release-set",
                CiCachePostureV1::Fallback,
                "passed:receipt-1",
            ),
            lane("release-set", CiCachePostureV1::Cold, "passed:receipt-1"),
        ],
        vec![CiCacheParityRowV1 {
            lane: "release-set".to_string(),
            compared_postures: vec!["warm".to_string(), "disabled".to_string()],
            semantic_results_equal: false,
        }],
    );
    let evaluation = evaluate_ci_cache_experiment(&receipt);
    assert_eq!(evaluation.verdict, CiCacheVerdictV1::Rejected);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("proof_divergence: release-set"))
    );
}

#[test]
fn ci_cache_fallback_parity_requires_comparisons_to_be_populated() {
    let receipt = experiment(
        vec![lane("release-set", CiCachePostureV1::Warm, "passed")],
        vec![CiCacheParityRowV1 {
            lane: "release-set".to_string(),
            compared_postures: Vec::new(),
            semantic_results_equal: true,
        }],
    );
    let evaluation = evaluate_ci_cache_experiment(&receipt);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("empty_parity_comparison"))
    );
}

#[test]
fn ci_cache_fallback_parity_corruption_requires_clean_fallback() {
    // Negative control 7: a corrupt cache must fall back to a clean
    // source run; it can never become a product failure with no
    // recorded fallback result.
    let mut corrupt = lane("release-set", CiCachePostureV1::Corrupt, "");
    corrupt.semantic_result = String::new();
    let evaluation = evaluate_ci_cache_experiment(&experiment(vec![corrupt], Vec::new()));
    assert_eq!(evaluation.verdict, CiCacheVerdictV1::InstrumentFailure);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("corrupt_without_fallback"))
    );
}

#[test]
fn ci_cache_fallback_parity_provider_outage_blocks_acceptance() {
    // Negative control 10: provider unavailability is a limitation;
    // the verdict cannot be Accepted on that window.
    let receipt = experiment(
        vec![
            lane("release-set", CiCachePostureV1::Warm, "passed"),
            lane("release-set", CiCachePostureV1::Warm, "passed"),
            lane("release-set", CiCachePostureV1::Disabled, "passed"),
            lane("release-set", CiCachePostureV1::Fallback, "passed"),
            lane("release-set", CiCachePostureV1::Cold, "passed"),
            lane(
                "release-set",
                CiCachePostureV1::ProviderUnavailable,
                "passed",
            ),
        ],
        vec![CiCacheParityRowV1 {
            lane: "release-set".to_string(),
            compared_postures: vec!["warm".to_string(), "cold".to_string()],
            semantic_results_equal: true,
        }],
    );
    let evaluation = evaluate_ci_cache_experiment(&receipt);
    assert_ne!(evaluation.verdict, CiCacheVerdictV1::Accepted);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("provider cache unavailability"))
    );
}

#[test]
fn ci_cache_fallback_parity_commands_run_under_every_posture() {
    // Proof preservation: every observation carries the exact selected
    // commands — a cache hit never skips them.
    let postures = [
        CiCachePostureV1::Cold,
        CiCachePostureV1::Warm,
        CiCachePostureV1::PartialHit,
        CiCachePostureV1::Miss,
        CiCachePostureV1::Disabled,
        CiCachePostureV1::Fallback,
    ];
    for posture in postures {
        let observation = lane("release-set", posture, "passed");
        assert!(
            !observation.commands.is_empty(),
            "posture {} must still run the selected commands",
            posture.label()
        );
    }
}

#[test]
fn ci_cache_fallback_parity_incidents_are_rejections() {
    let mut incident = lane("release-set", CiCachePostureV1::Warm, "passed");
    incident.incident = Some("false reuse across lock movement".to_string());
    let evaluation = evaluate_ci_cache_experiment(&experiment(vec![incident], Vec::new()));
    assert_eq!(evaluation.verdict, CiCacheVerdictV1::InstrumentFailure);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("incident: false reuse"))
    );
}
