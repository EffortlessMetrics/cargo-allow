//! Offline characterization for ExactCandidateInteropSmokeV1 (#2605).

const SCHEMA_ID: &str = "cargo-allow.exact-candidate-interop.v1";
const JOURNEY_SCHEMA_ID: &str = "cargo-allow.exact-candidate-interop-journeys.v1";
const EXAMPLE_RECEIPT: &str =
    include_str!("../../../docs/dogfood/receipts/exact-candidate-interop-pass.example.json");
const JOURNEY_FIXTURE: &str =
    include_str!("../../../docs/dogfood/fixtures/release/exact-candidate-interop-journeys.toml");

const EXPECTED_JOURNEYS: &[(&str, &str)] = &[
    ("A", "cargo-allow"),
    ("B", "cargo-intent"),
    ("C", "cargo-proof"),
    ("D", "cargo-proof"),
    ("E", "cargo-allow"),
];

#[test]
fn example_exact_candidate_interop_matches_schema_constants() {
    let example: serde_json::Value = serde_json::from_str(EXAMPLE_RECEIPT)
        .unwrap_or_else(|err| panic!("example receipt json: {err}"));
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
            .pointer("/candidate/journey_fixture_schema_id")
            .and_then(serde_json::Value::as_str),
        Some(JOURNEY_SCHEMA_ID)
    );
    let journeys = example
        .get("journeys")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("journeys missing"));
    assert_eq!(journeys.len(), EXPECTED_JOURNEYS.len());
    for (idx, (id, product)) in EXPECTED_JOURNEYS.iter().enumerate() {
        let entry = journeys
            .get(idx)
            .unwrap_or_else(|| std::panic::panic_any("journey entry missing"));
        assert_eq!(
            entry.get("id").and_then(serde_json::Value::as_str),
            Some(*id)
        );
        assert_eq!(
            entry.get("product").and_then(serde_json::Value::as_str),
            Some(*product)
        );
    }
    let boundary = example
        .get("claim_boundary")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("claim_boundary missing"));
    for required in [
        "outside_monorepo_consumer",
        "journey_a_cargo_allow_alone",
        "journey_b_cargo_intent_alone",
        "journey_c_cargo_proof_explicit_unavailable_plan",
        "journey_d_independent_cargo_allow_and_proof_dry_run",
        "journey_e_cargo_allow_delegates_cargo_intent",
        "no_workspace_target_debug_binary",
    ] {
        assert!(
            boundary.iter().any(|v| v.as_str() == Some(required)),
            "example claim_boundary missing {required}"
        );
    }
}

#[test]
fn interop_journey_fixture_lists_five_journeys() {
    for (id, product) in EXPECTED_JOURNEYS {
        assert!(
            JOURNEY_FIXTURE.contains(&format!("id = \"{id}\"")),
            "journey fixture missing id {id}"
        );
        assert!(
            JOURNEY_FIXTURE.contains(&format!("product = \"{product}\"")),
            "journey fixture missing product {product}"
        );
    }
    for scenario in [
        "absent",
        "compatible",
        "incompatible",
        "stale",
        "malformed",
        "partial",
        "wrong_snapshot",
    ] {
        assert!(
            JOURNEY_FIXTURE.contains(scenario),
            "journey fixture missing scenario class {scenario}"
        );
    }
}
