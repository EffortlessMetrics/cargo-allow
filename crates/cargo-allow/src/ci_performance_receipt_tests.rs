//! Typed proof tests for the #3835 CI performance receipt: the
//! measurement law (separated timing, honest missingness, retained
//! failures, attempt identity, generation binding, inventory-bound
//! purposes), the twelve negative controls, and the retained baseline
//! receipt.

use allow_report::{
    CI_PERFORMANCE_CLAIM_BOUNDARY, CiEnvironmentV1, CiJobConclusionV1, CiJobPurposeV1,
    CiPerformanceReceiptV1, validate_ci_performance_receipt,
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

fn run(conclusion: &str, jobs: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({
        "workflow": "ci.yml",
        "run_id": 1,
        "attempt": 1,
        "event": "push",
        "conclusion": conclusion,
        "environment": "hosted",
        "source_pair": {"base_sha": "aaaa", "head_sha": "bbbb", "generation": 1},
        "jobs": jobs
    })
}

fn job_json(extra: serde_json::Value) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("name".to_string(), serde_json::json!("test"));
    map.insert(
        "purpose".to_string(),
        serde_json::json!("core_compile_test"),
    );
    map.insert("routing_owner".to_string(), serde_json::json!("core/ci"));
    map.insert("blocking".to_string(), serde_json::json!(true));
    map.insert("runner".to_string(), serde_json::json!("ubuntu-latest"));
    map.insert("conclusion".to_string(), serde_json::json!("passed"));
    map.insert(
        "timing".to_string(),
        serde_json::json!({"compile_seconds": 100}),
    );
    map.insert("first_failure".to_string(), serde_json::json!(false));
    map.insert("critical_path".to_string(), serde_json::json!(false));
    map.insert("cache".to_string(), serde_json::Value::Null);
    map.insert("compute_minutes".to_string(), serde_json::Value::Null);
    if let serde_json::Value::Object(extra_map) = extra {
        for (key, value) in extra_map {
            map.insert(key, value);
        }
    }
    serde_json::Value::Object(map)
}

fn receipt_json(runs: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({
        "schema_id": "cargo-allow.ci-performance-receipt.v1",
        "schema_version": 1,
        "window_from": "2026-09-06T00:00:00Z",
        "window_to": "2026-09-06T23:59:59Z",
        "generation": 1,
        "runs": runs,
        "limits": [],
        "critical_path_first_failure": [],
        "critical_path_full_matrix": [],
        "redundant_work_candidates": [],
        "cache_opportunities": [],
        "improvement_targets_owner": "#3753",
        "claim_boundary": "bounded observation"
    })
}

fn validate_json(receipt: &serde_json::Value) -> Vec<String> {
    let parsed: CiPerformanceReceiptV1 =
        serde_json::from_value(receipt.clone()).expect("fixtures deserialize");
    validate_ci_performance_receipt(&parsed)
}

#[test]
fn ci_performance_receipt_retains_a_failed_run_beside_green_ones() {
    // Negative control 1: a green-only window is a cherry-picked
    // baseline; failed and cancelled runs are retained.
    let green_only = receipt_json(vec![run("success", vec![job_json(serde_json::json!({}))])]);
    assert!(validate_json(&green_only).contains(&"green_only_window".to_string()));

    let retained = receipt_json(vec![
        run("success", vec![job_json(serde_json::json!({}))]),
        run(
            "failure",
            vec![job_json(serde_json::json!({
                "name": "coverage", "purpose": "coverage", "conclusion": "failed"
            }))],
        ),
        run(
            "cancelled",
            vec![job_json(serde_json::json!({
                "name": "test", "conclusion": "cancelled"
            }))],
        ),
    ]);
    let codes = validate_json(&retained);
    assert!(
        !codes.contains(&"green_only_window".to_string()),
        "failed and cancelled runs keep the window honest: {codes:?}"
    );
}

#[test]
fn ci_performance_receipt_keeps_timing_buckets_separate_and_honest() {
    // Negative control 2: queue time is never compilation time — the
    // buckets are separate fields and the human projection prints them
    // from one typed result.
    let parsed: CiPerformanceReceiptV1 = serde_json::from_value(receipt_json(vec![run(
        "success",
        vec![job_json(serde_json::json!({
            "timing": {"queue_seconds": 60, "compile_seconds": 100}
        }))],
    )]))
    .expect("fixture deserializes");
    let job = parsed.runs.first().and_then(|run| run.jobs.first());
    assert!(job.is_some(), "the fixture retains one job");
    if let Some(job) = job {
        assert_eq!(job.timing.queue_seconds, Some(60));
        assert_eq!(job.timing.compile_seconds, Some(100));
    }

    // Negative control 4: missing timing stays missing; a zero-filled
    // total over no parts fails the receipt.
    let zero_filled = receipt_json(vec![run(
        "success",
        vec![job_json(serde_json::json!({
            "timing": {}, "compute_minutes": 0
        }))],
    )]);
    assert!(
        validate_json(&zero_filled)
            .iter()
            .any(|code| code.starts_with("missing_timing_zero_filled"))
    );
}

#[test]
fn ci_performance_receipt_retains_rerun_attempts_as_themselves() {
    // Negative control 3: two attempts of one run are retained as two
    // attempts, never double-counted as independent clean runs.
    let mut attempt_two = run("success", vec![job_json(serde_json::json!({}))]);
    *attempt_two
        .pointer_mut("/attempt")
        .expect("the fixture retains the attempt field") = serde_json::json!(2);
    let receipt = receipt_json(vec![
        run("success", vec![job_json(serde_json::json!({}))]),
        attempt_two,
    ]);
    let codes = validate_json(&receipt);
    // Both attempts are retained; the distinct-attempt receipt is
    // valid, while a repeated (run_id, attempt) pair is not.
    assert!(!codes.contains(&"duplicate_run_attempt".to_string()));

    let duplicated = receipt_json(vec![
        run("success", vec![job_json(serde_json::json!({}))]),
        run("success", vec![job_json(serde_json::json!({}))]),
    ]);
    assert!(validate_json(&duplicated).contains(&"duplicate_run_attempt".to_string()));
}

#[test]
fn ci_performance_receipt_never_counts_skipped_jobs_as_passed() {
    // Negative control 5: skipped and cancelled jobs are neither
    // passed nor on the critical path.
    let receipt = receipt_json(vec![run(
        "success",
        vec![job_json(serde_json::json!({
            "conclusion": "skipped", "critical_path": true
        }))],
    )]);
    assert!(
        validate_json(&receipt)
            .iter()
            .any(|code| code.starts_with("non_passed_job_on_critical_path"))
    );
    let typed = CiJobConclusionV1::Skipped;
    assert!(!typed.is_terminal_success());
}

#[test]
fn ci_performance_receipt_requires_inventory_bound_purposes() {
    // Negative control 6: a job name alone never decides the purpose;
    // every job binds a routing owner.
    let orphan = receipt_json(vec![run(
        "success",
        vec![job_json(serde_json::json!({
            "routing_owner": ""
        }))],
    )]);
    assert!(
        validate_json(&orphan)
            .iter()
            .any(|code| code.starts_with("routing_owner_missing"))
    );
    // The purpose vocabulary is closed.
    let missing_purpose: Result<CiPerformanceReceiptV1, _> =
        serde_json::from_value(receipt_json(vec![run(
            "success",
            vec![job_json(serde_json::json!({
                "purpose": "looks_fast"
            }))],
        )]));
    assert!(missing_purpose.is_err(), "unknown purposes fail closed");
}

#[test]
fn ci_performance_receipt_binds_one_workflow_generation() {
    // Negative control 7: source pairs from different workflow
    // generations are never compared without classification.
    let mut receipt = receipt_json(vec![run("success", vec![job_json(serde_json::json!({}))])]);
    *receipt
        .pointer_mut("/runs/0/source_pair/generation")
        .expect("the fixture retains the source pair") = serde_json::json!(7);
    assert!(validate_json(&receipt).contains(&"mixed_workflow_generations".to_string()));
}

#[test]
fn ci_performance_receipt_never_treats_cache_action_as_hit() {
    // Negative control 8: action presence without restored bytes is
    // not a warm hit.
    let receipt = receipt_json(vec![run(
        "success",
        vec![job_json(serde_json::json!({
            "cache": {"action_present": true, "class": "warm_hit",
                      "restored_bytes": null, "saved_bytes": null}
        }))],
    )]);
    assert!(
        validate_json(&receipt)
            .iter()
            .any(|code| code.starts_with("cache_action_treated_as_hit"))
    );
    // With byte evidence the class is honest.
    let evidenced = receipt_json(vec![run(
        "success",
        vec![job_json(serde_json::json!({
            "cache": {"action_present": true, "class": "warm_hit",
                      "restored_bytes": 1048576, "saved_bytes": null}
        }))],
    )]);
    assert!(
        !validate_json(&evidenced)
            .iter()
            .any(|code| code.starts_with("cache_action_treated_as_hit"))
    );
}

#[test]
fn ci_performance_receipt_refuses_local_and_unbounded_input() {
    // Negative controls 9 and 10: local builds are not hosted
    // evidence, and the typed schema rejects unknown provider fields
    // (tokens, private paths) outright.
    let mut local = receipt_json(vec![run("success", vec![job_json(serde_json::json!({}))])]);
    *local
        .pointer_mut("/runs/0/environment")
        .expect("the fixture retains the environment") = serde_json::json!("local");
    assert!(validate_json(&local).contains(&"local_run_in_hosted_baseline".to_string()));

    let hostile_job = serde_json::json!({
        "name": "test", "purpose": "core_compile_test", "routing_owner": "core/ci",
        "blocking": true, "runner": "ubuntu-latest", "conclusion": "passed",
        "timing": {"compile_seconds": 1}, "first_failure": false,
        "critical_path": false, "cache": null, "compute_minutes": null,
        "github_token": "ghs_secret"
    });
    let parse: Result<CiPerformanceReceiptV1, _> =
        serde_json::from_value(receipt_json(vec![run("success", vec![hostile_job])]));
    assert!(
        parse.is_err(),
        "unknown provider fields (secrets) fail the bounded schema"
    );
}

#[test]
fn ci_performance_receipt_declares_the_3753_owner_and_boundary() {
    // Negative controls 11 and 12: the receipt owns measurement only;
    // targets are #3753's and the boundary excludes correctness.
    let mut receipt = receipt_json(vec![run("success", vec![job_json(serde_json::json!({}))])]);
    *receipt
        .pointer_mut("/improvement_targets_owner")
        .expect("the fixture retains the owner field") = serde_json::json!("   ");
    assert!(validate_json(&receipt).contains(&"improvement_targets_owner_missing".to_string()));
    serde_json::from_value::<CiPerformanceReceiptV1>(receipt_json(vec![run(
        "success",
        vec![job_json(serde_json::json!({}))],
    )]))
    .expect("a receipt with the real claim boundary deserializes");
    assert!(
        CI_PERFORMANCE_CLAIM_BOUNDARY.contains("is not product or release correctness evidence")
    );
}

#[test]
fn ci_performance_receipt_loads_the_retained_baseline() {
    let root = workspace_root();
    let text = read_workspace_file(&root, "docs/ci/receipts/ci-performance-baseline-v1.json");
    let receipt: CiPerformanceReceiptV1 =
        serde_json::from_str(&text).expect("the retained baseline parses");
    assert_eq!(receipt.generation, 1);
    assert_eq!(receipt.runs.len(), 4);
    assert!(
        receipt.runs.iter().any(|run| run.conclusion == "failure"),
        "the baseline retains the failed main-branch run"
    );
    assert!(
        receipt
            .runs
            .iter()
            .all(|run| run.environment == CiEnvironmentV1::Hosted)
    );
    assert_eq!(receipt.claim_boundary, CI_PERFORMANCE_CLAIM_BOUNDARY);
    let codes = validate_ci_performance_receipt(&receipt);
    assert!(
        codes.is_empty(),
        "the retained baseline satisfies the measurement law: {codes:?}"
    );
    // The critical path is platform-dominated and the candidates name
    // redundant work without prescribing a mechanism.
    assert!(
        receipt
            .critical_path_full_matrix
            .first()
            .is_some_and(|first| first.contains("windows"))
    );
    assert!(!receipt.redundant_work_candidates.is_empty());
}

#[test]
fn ci_performance_receipt_views_derive_from_one_result() {
    let mut fixture = receipt_json(vec![run(
        "failure",
        vec![job_json(serde_json::json!({
            "name": "coverage", "purpose": "coverage", "conclusion": "failed",
            "critical_path": true
        }))],
    )]);
    *fixture
        .pointer_mut("/critical_path_first_failure")
        .expect("the fixture retains the critical path") = serde_json::json!(["coverage"]);
    let receipt: CiPerformanceReceiptV1 =
        serde_json::from_value(fixture).expect("fixture deserializes");
    let json = render_json(&receipt);
    let roundtrip: CiPerformanceReceiptV1 =
        serde_json::from_str(json.as_str()).expect("the JSON view parses back");
    assert_eq!(roundtrip, receipt);
    let human = allow_report::render_ci_performance_receipt_human(&receipt);
    assert!(human.contains("run 1 ci.yml"));
    assert!(human.contains("purpose=coverage"));
    assert!(human.contains("conclusion=failed"));
    assert!(human.contains("first-failure critical path:"));
}

fn render_json(receipt: &CiPerformanceReceiptV1) -> String {
    allow_report::render_ci_performance_receipt_json(receipt).expect("serialization succeeds")
}

#[test]
fn ci_performance_receipt_binds_the_collector_and_inventory() {
    let root = workspace_root();
    let script = read_workspace_file(&root, "scripts/collect-ci-performance.sh");
    assert!(
        script.contains("policy/ci-job-inventory.toml"),
        "the collector consumes the checked proof-class inventory"
    );
    assert!(
        script.contains("never zero-filled") || script.contains("zero-filled"),
        "the collector documents the missingness law"
    );
    let inventory = read_workspace_file(&root, "policy/ci-job-inventory.toml");
    assert!(
        inventory.contains("second impact classifier beside #2569"),
        "the inventory stays measurement metadata"
    );
    // Every purpose class the checked inventory uses is in the closed
    // typed vocabulary.
    for purpose in [
        "core_compile_test",
        "windows_platform",
        "msrv",
        "coverage",
        "package_install",
        "integrated_dogfood",
        "static_pre_gate",
        "shared_consumer_test",
        "security_dependency",
        "intent_experimental",
        "proof_experimental",
        "artifact_diagnostics",
        "external_review",
        "release_rehearsal",
    ] {
        let parsed: Result<CiJobPurposeV1, _> = serde_json::from_value(serde_json::json!(purpose));
        assert!(
            parsed.is_ok(),
            "purpose {purpose} is in the typed vocabulary"
        );
    }
}
