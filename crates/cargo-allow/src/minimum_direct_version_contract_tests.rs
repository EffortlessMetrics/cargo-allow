//! Typed contract law tests for the #3903 direct minimum version
//! contract: the ten negative controls, the result vocabulary, and the
//! deterministic views.

use allow_report::{
    DirectDependencyClassV1, MINIMUM_DIRECT_VERSION_SCHEMA_ID, MinimumFloorResultV1,
    MinimumFloorRowV1, MinimumVersionProofReceiptV1, MinimumVersionProofRequestV1,
    MinimumVersionRowResultV1, evaluate_minimum_version_proof, render_minimum_proof_human,
    render_minimum_proof_json,
};

fn request() -> MinimumVersionProofRequestV1 {
    MinimumVersionProofRequestV1 {
        product: "cargo-allow".to_string(),
        package_roots: vec!["cargo-allow".to_string()],
        features: vec!["default".to_string()],
        target: "x86_64-pc-windows-msvc".to_string(),
        msrv: "1.95".to_string(),
        floors: vec![MinimumFloorRowV1 {
            package: "serde".to_string(),
            declared_requirement: "1.0".to_string(),
            selected_floor: "1.0.210".to_string(),
            class: DirectDependencyClassV1::Normal,
        }],
        proof_classes: vec!["check".to_string(), "test".to_string()],
        manifest_set_digest: "manifest-digest-1".to_string(),
        lock_digest: "lock-digest-1".to_string(),
    }
}

fn row(package: &str, result: MinimumFloorResultV1) -> MinimumVersionRowResultV1 {
    MinimumVersionRowResultV1 {
        package: package.to_string(),
        declared_requirement: "1.0".to_string(),
        tested_floor: "1.0.210".to_string(),
        resolved_version: "1.0.210".to_string(),
        source_identity: "registry:crates.io".to_string(),
        result,
        limitation: if result.is_clean() {
            None
        } else {
            Some("bounded limitation recorded".to_string())
        },
    }
}

fn receipt(rows: Vec<MinimumVersionRowResultV1>) -> MinimumVersionProofReceiptV1 {
    MinimumVersionProofReceiptV1 {
        schema_id: MINIMUM_DIRECT_VERSION_SCHEMA_ID.to_string(),
        schema_version: 1,
        product: "cargo-allow".to_string(),
        package_roots: vec!["cargo-allow".to_string()],
        msrv: "1.95".to_string(),
        toolchain: "1.95.0".to_string(),
        target: "x86_64-pc-windows-msvc".to_string(),
        manifest_set_digest: "manifest-digest-1".to_string(),
        lock_digest: "lock-digest-1".to_string(),
        rows,
        commands: vec!["cargo check --locked".to_string()],
        floor_lock_digest: "floor-lock-digest-1".to_string(),
        limitations: Vec::new(),
        claim_boundary: "bounded".to_string(),
    }
}

#[test]
fn minimum_direct_version_contract_proves_a_clean_row_set() {
    let evaluation = evaluate_minimum_version_proof(
        &request(),
        &receipt(vec![row("serde", MinimumFloorResultV1::Proven)]),
    );
    assert_eq!(
        evaluation.verdict,
        allow_report::MinimumProofVerdictV1::Complete
    );
    assert!(evaluation.reasons.is_empty());
}

#[test]
fn minimum_direct_version_contract_never_accepts_the_current_lock_as_floor_proof() {
    // Negative control 1: the receipt must name the tested floor and
    // resolved version separately from the ambient lock. A row whose
    // tested floor is the ambient lock version (newer than the floor)
    // is rejected as a substitution.
    let request = request();
    let mut substitution = receipt(vec![row("serde", MinimumFloorResultV1::Proven)]);
    *substitution
        .rows
        .first_mut()
        .expect("the fixture retains one row") = MinimumVersionRowResultV1 {
        tested_floor: "1.0.228".to_string(),
        resolved_version: "1.0.228".to_string(),
        ..row("serde", MinimumFloorResultV1::Proven)
    };
    let evaluation = evaluate_minimum_version_proof(&request, &substitution);
    assert_ne!(
        evaluation.verdict,
        allow_report::MinimumProofVerdictV1::Complete,
        "a compatible newer version cannot substitute for the declared floor"
    );
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("floor substitution")),
        "the substitution must surface as a named drift: {:?}",
        evaluation.reasons
    );
}

#[test]
fn minimum_direct_version_contract_distinct_dispositions_stay_distinct() {
    // Negative control 6: unavailable/yanked (resolver failure) and
    // compile-incompatible (unsupported combination) are different
    // vocabulary entries with different labels.
    assert_ne!(
        MinimumFloorResultV1::ResolverFailure.label(),
        MinimumFloorResultV1::UnsupportedCombination.label()
    );
    let resolver = row("serde", MinimumFloorResultV1::ResolverFailure);
    let unsupported = row("serde", MinimumFloorResultV1::UnsupportedCombination);
    assert_ne!(resolver.result, unsupported.result);
}

#[test]
fn minimum_direct_version_contract_newer_toolchain_fails_loudly() {
    // Negative control 7: a Rust newer than the claimed MSRV does not
    // satisfy the rows silently.
    let mut receipt = receipt(vec![row("serde", MinimumFloorResultV1::Proven)]);
    receipt.toolchain = "1.99.0".to_string();
    let evaluation = evaluate_minimum_version_proof(&request(), &receipt);
    assert_eq!(
        evaluation.verdict,
        allow_report::MinimumProofVerdictV1::InstrumentFailure
    );
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("newer than the claimed MSRV"))
    );
}

#[test]
fn minimum_direct_version_contract_stales_on_identity_movement() {
    // Negative control 8: source/manifest/lock movement stales the
    // receipt.
    let mut request = request();
    request.manifest_set_digest = "manifest-digest-2".to_string();
    let evaluation = evaluate_minimum_version_proof(
        &request,
        &receipt(vec![row("serde", MinimumFloorResultV1::Proven)]),
    );
    assert_eq!(
        evaluation.verdict,
        allow_report::MinimumProofVerdictV1::Stale
    );
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("stale:"))
    );
}

#[test]
fn minimum_direct_version_contract_empty_denominator_is_never_proven() {
    // Negative control 9: zero selected rows is an instrument failure.
    let mut request = request();
    request.floors = Vec::new();
    let mut receipt = receipt(Vec::new());
    receipt.rows = Vec::new();
    let evaluation = evaluate_minimum_version_proof(&request, &receipt);
    assert_eq!(
        evaluation.verdict,
        allow_report::MinimumProofVerdictV1::InstrumentFailure
    );
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("empty denominator"))
    );
}

#[test]
fn minimum_direct_version_contract_product_sets_stay_separate() {
    // Negative control 5: one product's proof cannot satisfy another's
    // request, and negative control 10: a report-only product's
    // failure is scoped to that product.
    let mut request = request();
    request.product = "cargo-intent".to_string();
    let evaluation = evaluate_minimum_version_proof(
        &request,
        &receipt(vec![row("serde", MinimumFloorResultV1::FloorTooLow)]),
    );
    assert_eq!(
        evaluation.verdict,
        allow_report::MinimumProofVerdictV1::Incomplete
    );
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("product_mismatch")),
        "the product mismatch must be named even for a report-only product"
    );
}

#[test]
fn minimum_direct_version_contract_package_roots_must_match_the_request() {
    // A receipt recorded against different package roots is not this
    // request's proof even when the product label matches; receipts
    // that predate root recording (empty roots) stay governed by the
    // request's roots alone.
    let mut roots_request = request();
    roots_request.package_roots = vec!["allow-core".to_string()];
    let evaluation = evaluate_minimum_version_proof(
        &roots_request,
        &receipt(vec![row("serde", MinimumFloorResultV1::Proven)]),
    );
    assert_eq!(
        evaluation.verdict,
        allow_report::MinimumProofVerdictV1::Incomplete
    );
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("package_roots_mismatch")),
        "the roots mismatch must be named: {:?}",
        evaluation.reasons
    );

    let mut legacy = receipt(vec![row("serde", MinimumFloorResultV1::Proven)]);
    legacy.package_roots = Vec::new();
    let legacy_evaluation = evaluate_minimum_version_proof(&request(), &legacy);
    assert_eq!(
        legacy_evaluation.verdict,
        allow_report::MinimumProofVerdictV1::Complete,
        "a legacy receipt without recorded roots is governed by the request"
    );
}

#[test]
fn minimum_direct_version_contract_non_clean_rows_carry_limitations() {
    let mut unlimitated = receipt(vec![row("serde", MinimumFloorResultV1::FloorTooLow)]);
    let first = unlimitated
        .rows
        .first_mut()
        .expect("the fixture retains one row");
    first.limitation = None;
    let evaluation = evaluate_minimum_version_proof(&request(), &unlimitated);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("non-clean row without limitation")),
        "a non-clean row without a limitation must be named: {:?}",
        evaluation.reasons
    );
}

#[test]
fn minimum_direct_version_contract_receipt_rows_must_match_floors() {
    // A receipt row that is not in the request's selected floors is
    // named.
    let mut receipt = receipt(vec![row("serde", MinimumFloorResultV1::Proven)]);
    let row = row("rand", MinimumFloorResultV1::Proven);
    receipt.rows.push(row);
    let request = request();
    let evaluation = evaluate_minimum_version_proof(&request, &receipt);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("not in the request's selected floors")),
        "receipt rows outside the request are named: {:?}",
        evaluation.reasons
    );
}

#[test]
fn minimum_direct_version_contract_views_derive_from_one_result() {
    let evaluation = evaluate_minimum_version_proof(
        &request(),
        &receipt(vec![row("serde", MinimumFloorResultV1::Proven)]),
    );
    let json = render_minimum_proof_json(&evaluation).expect("serialization succeeds");
    let roundtrip: allow_report::MinimumProofEvaluationV1 =
        serde_json::from_str(json.as_str()).expect("the JSON view parses back");
    assert_eq!(roundtrip, evaluation);
    let human = render_minimum_proof_human(&evaluation);
    assert!(human.contains("verdict=complete"));
    assert!(human.contains("claim boundary:"));
}

#[test]
fn minimum_direct_version_contract_vocabulary_is_fully_labeled() {
    // Every floor result label and cleanliness flag is exercised so the
    // vocabulary cannot silently drift.
    for (result, label, clean) in [
        (MinimumFloorResultV1::Proven, "proven", true),
        (MinimumFloorResultV1::FloorTooLow, "floor_too_low", false),
        (
            MinimumFloorResultV1::UnsupportedCombination,
            "unsupported_combination",
            false,
        ),
        (
            MinimumFloorResultV1::ResolverFailure,
            "resolver_failure",
            false,
        ),
        (
            MinimumFloorResultV1::PackageMetadataMismatch,
            "package_metadata_mismatch",
            false,
        ),
        (
            MinimumFloorResultV1::InstrumentFailure,
            "instrument_failure",
            false,
        ),
        (MinimumFloorResultV1::NotClaimed, "not_claimed", false),
    ] {
        assert_eq!(result.label(), label);
        assert_eq!(result.is_clean(), clean);
    }
    for (class, label) in [
        (DirectDependencyClassV1::Normal, "normal"),
        (DirectDependencyClassV1::Dev, "dev"),
        (DirectDependencyClassV1::Build, "build"),
    ] {
        assert_eq!(class.label(), label);
    }
}

#[test]
fn minimum_direct_version_contract_schema_mismatch_is_named() {
    let mut wrong = receipt(vec![row("serde", MinimumFloorResultV1::Proven)]);
    wrong.schema_id = "not-the-schema".to_string();
    let evaluation = evaluate_minimum_version_proof(&request(), &wrong);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("schema_mismatch"))
    );
}

#[test]
fn minimum_direct_version_contract_set_round_trips() {
    // The typed denominator round-trips through serde unchanged.
    let set: allow_report::DirectMinimumVersionSetV1 = serde_json::from_str(
        r#"{
        "schema_id": "cargo-allow.minimum-direct-version.v1",
        "schema_version": 1,
        "product_sets": [{
            "product": "cargo-allow",
            "packages": ["cargo-allow"],
            "rows": [{"package": "serde", "requirement": "1.0",
                      "class": "normal"}],
            "manifest_set_digest": "m",
            "lock_digest": "l",
            "msrv": "1.95"
        }],
        "claim_boundary": "bounded"
    }"#,
    )
    .expect("the typed set parses");
    let reserialized = serde_json::to_string_pretty(&set).expect("serialization succeeds");
    let roundtrip: allow_report::DirectMinimumVersionSetV1 =
        serde_json::from_str(reserialized.as_str()).expect("the round-trip parses");
    assert_eq!(roundtrip, set, "serialization must not drift the set");
    assert_eq!(set.product_sets[0].rows[0].package, "serde");
}

#[test]
fn minimum_direct_version_contract_resolved_version_must_equal_tested_floor() {
    let mut receipt = receipt(vec![row("serde", MinimumFloorResultV1::Proven)]);
    let mut row = row("serde", MinimumFloorResultV1::Proven);
    row.resolved_version = "1.0.228".to_string();
    receipt.rows = vec![row];
    let evaluation = evaluate_minimum_version_proof(&request(), &receipt);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("resolved substitution")),
        "the ambient lock version cannot silently stand in: {:?}",
        evaluation.reasons
    );
}

#[test]
fn minimum_direct_version_contract_empty_commands_and_target_drift_are_named() {
    let mut no_commands = receipt(vec![row("serde", MinimumFloorResultV1::Proven)]);
    no_commands.commands = Vec::new();
    no_commands.target = "aarch64-apple-darwin".to_string();
    let evaluation = evaluate_minimum_version_proof(&request(), &no_commands);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("records no executed commands")),
        "empty command evidence must be named"
    );
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("target mismatch")),
        "a drifted target must be named"
    );
}

#[test]
fn minimum_direct_version_contract_schema_version_drift_is_named() {
    let mut receipt = receipt(vec![row("serde", MinimumFloorResultV1::Proven)]);
    receipt.schema_version = 2;
    let evaluation = evaluate_minimum_version_proof(&request(), &receipt);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("schema_version_mismatch")),
        "unsupported schema versions must be named"
    );
}
