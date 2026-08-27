//! Offline characterization for SourceCandidateSmokeReceiptV1
//! (#2278 / #2373 / #2387 / #2396 / #2398 / #2400 / #2402 / #2403).
//!
//! The installed first-hour + lifecycle journey itself lives in
//! `scripts/source-candidate-smoke.sh` so release harnesses can invoke Cargo
//! without violating the product source-tree invariant that Rust sources must
//! not spawn Cargo/compiler tools.

const SCHEMA_ID: &str = "cargo-allow.source-candidate-smoke-receipt.v1";
const EXAMPLE_RECEIPT: &str =
    include_str!("../../../docs/dogfood/receipts/source-candidate-smoke-pass.example.json");
const SCHEMA_DOC: &str = include_str!(
    "../../../docs/dogfood/fixtures/release/source-candidate-smoke-receipt.v1.schema.json"
);

const STEPS_EXPECTED: &[&str] = &[
    "version",
    "doctor_no_policy",
    "audit_with_finding",
    "bootstrap_propose_write",
    "check_no_new_pass",
    "list_explain_worklist",
    "refresh_location_drift_preview_write",
    "diff_against_exact_base",
    "prune_stale_preview_write",
    "final_check_no_new",
    "policy_rollback_after_prune",
];

#[test]
fn example_source_candidate_smoke_receipt_matches_schema_constants() {
    assert!(
        SCHEMA_DOC.contains(SCHEMA_ID),
        "schema fixture must pin {SCHEMA_ID}"
    );
    assert!(
        SCHEMA_DOC.contains("negative_controls"),
        "schema must include negative_controls for #2373/#2387/#2396/#2398/#2400/#2402/#2403"
    );
    assert!(
        SCHEMA_DOC.contains("NetworkRequired"),
        "schema must enumerate NetworkRequired for unexpected-network fails"
    );
    assert!(
        SCHEMA_DOC.contains("RecoveryFailed"),
        "schema must enumerate RecoveryFailed for failed-policy-rollback fails"
    );
    assert!(
        SCHEMA_DOC.contains("NotProven"),
        "schema must enumerate NotProven for optional-profile-without-assets fails"
    );
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
    let expected = example
        .pointer("/journey/steps_expected")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("steps_expected missing"));
    assert_eq!(expected.len(), STEPS_EXPECTED.len());
    for (idx, step) in STEPS_EXPECTED.iter().enumerate() {
        assert_eq!(
            expected.get(idx).and_then(serde_json::Value::as_str),
            Some(*step)
        );
    }
    let claim_boundary = example
        .get("claim_boundary")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("claim_boundary missing"));
    assert!(
        claim_boundary
            .iter()
            .any(|v| v.as_str() == Some("post_install_source_hidden_ordinary_scan")),
        "example must claim post-install source-hidden ordinary scan"
    );
    assert!(
        claim_boundary
            .iter()
            .any(|v| v.as_str() == Some("ordinary_scan_does_not_require_network")),
        "example must claim ordinary scan does not require network"
    );
    assert!(
        claim_boundary
            .iter()
            .any(|v| v.as_str() == Some("policy_rollback_after_prune")),
        "example must claim policy rollback after prune"
    );
    assert!(
        claim_boundary
            .iter()
            .any(|v| v.as_str() == Some("packaged_asset_omit_rebuild")),
        "example must claim packaged asset omit rebuild"
    );
    assert!(
        claim_boundary
            .iter()
            .any(|v| v.as_str() == Some("optional_profile_without_assets_not_proven")),
        "example must claim optional profile without assets is NotProven"
    );
    let negatives = example
        .get("negative_controls")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("negative_controls missing"));
    let ids: Vec<&str> = negatives
        .iter()
        .filter_map(|v| v.get("id").and_then(serde_json::Value::as_str))
        .collect();
    for required in [
        "omitted_journey_step_cannot_claim_passed",
        "prune_preview_apply_subject_agree",
        "refresh_preview_apply_subject_agree",
        "malformed_smoke_receipt_cannot_claim_passed",
        "post_install_source_hidden_ordinary_scan",
        "missing_asset_not_satisfied_by_source_checkout",
        "wrong_installed_binary_version",
        "ordinary_scan_does_not_require_network",
        "unexpected_network_requirement_during_ordinary_scan",
        "failed_policy_rollback_after_prune",
        "optional_profile_without_packaged_assets",
    ] {
        assert!(
            ids.contains(&required),
            "example receipt missing negative control {required}"
        );
    }
    let network_required = negatives.iter().find(|v| {
        v.get("id").and_then(serde_json::Value::as_str)
            == Some("unexpected_network_requirement_during_ordinary_scan")
    });
    assert_eq!(
        network_required
            .and_then(|v| v.get("result_class"))
            .and_then(serde_json::Value::as_str),
        Some("NetworkRequired"),
        "unexpected-network control must classify NetworkRequired"
    );
    let recovery_failed = negatives.iter().find(|v| {
        v.get("id").and_then(serde_json::Value::as_str)
            == Some("failed_policy_rollback_after_prune")
    });
    assert_eq!(
        recovery_failed
            .and_then(|v| v.get("result_class"))
            .and_then(serde_json::Value::as_str),
        Some("RecoveryFailed"),
        "failed-policy-rollback control must classify RecoveryFailed"
    );
    let not_proven = negatives.iter().find(|v| {
        v.get("id").and_then(serde_json::Value::as_str)
            == Some("optional_profile_without_packaged_assets")
    });
    assert_eq!(
        not_proven
            .and_then(|v| v.get("result_class"))
            .and_then(serde_json::Value::as_str),
        Some("NotProven"),
        "optional-profile-without-assets control must classify NotProven"
    );
    assert!(
        not_proven
            .and_then(|v| v.get("detail"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|detail| detail.contains("codex-pack")),
        "NotProven detail must record selected optional profile codex-pack"
    );
    let limitations = example
        .get("limitations")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("limitations missing"));
    assert!(
        limitations
            .iter()
            .any(|v| v.as_str() == Some("package_set_not_consumed_from_isolated_registry")),
        "example receipt must record ExactCandidate isolation limitation"
    );
    assert!(
        limitations
            .iter()
            .any(|v| v.as_str() == Some("source_checkout_not_denied_during_install")),
        "example must still record that path install uses the source checkout"
    );
    assert!(
        !limitations
            .iter()
            .any(|v| v.as_str() == Some("checkout_denial_negative_deferred")),
        "example must not claim checkout-denial remains deferred after #2396"
    );
    assert!(
        !limitations
            .iter()
            .any(|v| v.as_str() == Some("omit_packaged_asset_rebuild_not_executed")),
        "example must not claim package-rebuild omit remains deferred after #2402"
    );
    assert!(
        !limitations
            .iter()
            .any(|v| v.as_str() == Some("optional_profile_without_assets_not_executed")),
        "example must not claim optional-profile-without-assets remains deferred after #2403"
    );
    let missing_asset = negatives.iter().find(|v| {
        v.get("id").and_then(serde_json::Value::as_str)
            == Some("missing_asset_not_satisfied_by_source_checkout")
    });
    assert_eq!(
        missing_asset
            .and_then(|v| v.get("result_class"))
            .and_then(serde_json::Value::as_str),
        Some("MissingAsset"),
        "package-rebuild omit control must classify MissingAsset"
    );
    assert!(
        missing_asset
            .and_then(|v| v.get("detail"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|detail| detail.contains("package-rebuild omit")),
        "MissingAsset detail must record package-rebuild omit execution"
    );
}
