//! Offline characterization for ThreeProductDogfoodSmokeV1 (#2558).

const SCHEMA_ID: &str = "cargo-allow.three-product-dogfood.v1";
const STAGE_SCHEMA_ID: &str = "cargo-allow.three-product-dogfood-stages.v1";
const EXAMPLE_RECEIPT: &str =
    include_str!("../../../docs/dogfood/receipts/three-product-dogfood-pass.example.json");
const STAGE_FIXTURE: &str =
    include_str!("../../../tests/fixtures/three-product-dogfood/pipeline-stages-v1.toml");
const STAGE_SCRIPT: &str = include_str!("../../../scripts/three-product-dogfood-smoke.sh");

const EXPECTED_STAGE_IDS: &[&str] = &[
    "source_change",
    "cargo_allow_audit",
    "cargo_allow_propose",
    "cargo_allow_check_no_new",
    "cargo_intent_change_status",
    "obligation_plan_bridge",
    "cargo_proof_plan",
    "cargo_proof_dry_run",
    "evidence_cargo_allow",
    "evidence_ripr",
    "evidence_hawk",
    "evidence_test",
    "contradiction_eval",
    "repair",
    "precommit_gate",
    "merge_ready_gate",
    "reconciliation",
];

#[test]
fn example_three_product_dogfood_matches_schema_constants() {
    let example: serde_json::Value = serde_json::from_str(EXAMPLE_RECEIPT)
        .unwrap_or_else(|err| std::panic::panic_any(format!("example receipt json: {err}")));
    assert_eq!(
        example.get("schema_id").and_then(serde_json::Value::as_str),
        Some(SCHEMA_ID)
    );
    assert_eq!(
        example.get("result").and_then(serde_json::Value::as_str),
        Some("Passed")
    );
    assert_eq!(
        example
            .pointer("/candidate/stage_fixture_schema_id")
            .and_then(serde_json::Value::as_str),
        Some(STAGE_SCHEMA_ID)
    );
    let boundary = example
        .get("claim_boundary")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("claim_boundary missing"));
    for required in [
        "stubbed_ripr_and_hawk_evidence",
        "no_physical_repository_extraction",
    ] {
        assert!(
            boundary.iter().any(|v| v.as_str() == Some(required)),
            "example claim_boundary missing {required}"
        );
    }
    let stubbed = example
        .get("stubbed")
        .and_then(serde_json::Value::as_object)
        .unwrap_or_else(|| std::panic::panic_any("stubbed missing"));
    assert!(stubbed.contains_key("ripr"));
    assert!(stubbed.contains_key("hawk"));
}

#[test]
fn dogfood_stage_fixture_lists_seventeen_pipeline_stages() {
    for stage_id in EXPECTED_STAGE_IDS {
        assert!(
            STAGE_FIXTURE.contains(&format!("id = \"{stage_id}\"")),
            "stage fixture missing {stage_id}"
        );
    }
    for execution in ["real", "bridged", "stubbed", "simulated"] {
        assert!(
            STAGE_FIXTURE.contains(execution),
            "stage fixture missing execution mode {execution}"
        );
    }
}

#[test]
fn dogfood_script_and_fixture_preserve_stage_execution_modes() {
    let expected = [
        ("source_change", "real"),
        ("cargo_allow_audit", "real"),
        ("cargo_allow_propose", "real"),
        ("cargo_allow_check_no_new", "real"),
        ("cargo_intent_change_status", "real"),
        ("obligation_plan_bridge", "bridged"),
        ("cargo_proof_plan", "explicit_unavailable"),
        ("cargo_proof_dry_run", "real"),
        ("evidence_cargo_allow", "real"),
        ("evidence_ripr", "stubbed"),
        ("evidence_hawk", "stubbed"),
        ("evidence_test", "stubbed"),
        ("contradiction_eval", "simulated"),
        ("repair", "real"),
        ("precommit_gate", "real"),
        ("merge_ready_gate", "simulated"),
        ("reconciliation", "simulated"),
    ];
    for (stage_id, execution) in expected {
        let stage_row = STAGE_FIXTURE
            .lines()
            .find(|line| line.contains(&format!("id = \"{stage_id}\"")))
            .unwrap_or_else(|| std::panic::panic_any(format!("fixture missing {stage_id}")));
        assert!(
            stage_row.contains(&format!("execution = \"{execution}\"")),
            "fixture mode drift for {stage_id}: {execution}"
        );
        assert!(
            STAGE_SCRIPT.contains(&format!("record_stage \"{stage_id}\" \"{execution}\"")),
            "script mode drift for {stage_id}: {execution}"
        );
    }
    let evidence_row = STAGE_FIXTURE
        .lines()
        .find(|line| line.contains("id = \"evidence_cargo_allow\""))
        .unwrap_or_else(|| std::panic::panic_any("evidence_cargo_allow fixture row missing"));
    assert!(
        evidence_row.contains("product = \"cargo-allow\""),
        "direct evidence must be attributed to cargo-allow, not cargo-proof"
    );
    assert!(
        STAGE_SCRIPT.contains("direct cargo-allow check is not evidence that cargo-proof"),
        "evidence_cargo_allow claim boundary must remain explicit"
    );
}
