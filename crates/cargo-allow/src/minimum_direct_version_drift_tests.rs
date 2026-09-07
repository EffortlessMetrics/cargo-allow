//! Drift-guard law tests and the live four-product grading (#3903 PR
//! D): identity movement (dependency, feature, target, MSRV,
//! package-set, or manifest movement shows up as one of the identity
//! digests, the MSRV, or the roots) stales or incompletes the affected
//! request, non-clean rows stay visible, only release-set drift blocks,
//! and every retained receipt on the current tree grades current
//! against the live manifest-derived observation.

use allow_report::{
    DRIFT_RELEASE_SET_PRODUCT, MinimumFloorResultV1, MinimumVersionDriftFloorRowV1,
    MinimumVersionDriftObservationV1, MinimumVersionDriftVerdictV1, MinimumVersionProofReceiptV1,
    MinimumVersionRowResultV1, evaluate_minimum_version_drift,
};

use crate::minimum_version_selection::{derive_selection, manifest_set_digest, workspace_msrv};

fn floor_row(package: &str) -> MinimumVersionDriftFloorRowV1 {
    MinimumVersionDriftFloorRowV1 {
        package: package.to_string(),
        declared_requirement: "1.0".to_string(),
        selected_floor: "1.0.150".to_string(),
    }
}

fn observation(product: &str) -> MinimumVersionDriftObservationV1 {
    MinimumVersionDriftObservationV1 {
        product: product.to_string(),
        package_roots: vec![format!("{product}-root")],
        msrv: "1.95".to_string(),
        manifest_set_digest: "manifests-current".to_string(),
        lock_digest: "lock-current".to_string(),
        floors: vec![floor_row("serde")],
    }
}

fn row(package: &str, result: MinimumFloorResultV1) -> MinimumVersionRowResultV1 {
    MinimumVersionRowResultV1 {
        package: package.to_string(),
        declared_requirement: "1.0".to_string(),
        tested_floor: "1.0.150".to_string(),
        resolved_version: "1.0.150".to_string(),
        source_identity: "registry:crates.io".to_string(),
        result,
        limitation: if result.is_clean() {
            None
        } else {
            Some("bounded limitation recorded".to_string())
        },
    }
}

fn receipt(product: &str, rows: Vec<MinimumVersionRowResultV1>) -> MinimumVersionProofReceiptV1 {
    MinimumVersionProofReceiptV1 {
        schema_id: "cargo-allow.minimum-direct-version.v1".to_string(),
        schema_version: 1,
        product: product.to_string(),
        package_roots: vec![format!("{product}-root")],
        msrv: "1.95".to_string(),
        toolchain: "1.95.0".to_string(),
        target: "x86_64-unknown-linux-gnu".to_string(),
        manifest_set_digest: "manifests-current".to_string(),
        lock_digest: "lock-current".to_string(),
        rows,
        commands: vec!["cargo check --locked".to_string()],
        floor_lock_digest: "sha256:v1:floor-lock".to_string(),
        limitations: Vec::new(),
        claim_boundary: "bounded".to_string(),
    }
}

#[test]
fn minimum_direct_version_drift_missing_receipt_is_unproven() {
    // A product with no retained receipt is unproven — never current —
    // and an advisory product's unproven state does not block.
    let evaluation = evaluate_minimum_version_drift(&observation("shared"), None);
    assert_eq!(evaluation.verdict, MinimumVersionDriftVerdictV1::Unproven);
    assert!(!evaluation.blocking, "advisory unproven never blocks");
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("no retained receipt")),
        "the missing receipt is named: {:?}",
        evaluation.reasons
    );
}

#[test]
fn minimum_direct_version_drift_wrong_product_receipt_is_unproven() {
    let foreign = receipt(
        "cargo-proof",
        vec![row("serde", MinimumFloorResultV1::Proven)],
    );
    let evaluation = evaluate_minimum_version_drift(&observation("shared"), Some(&foreign));
    assert_eq!(evaluation.verdict, MinimumVersionDriftVerdictV1::Unproven);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("receipt names product cargo-proof, not shared")),
        "the wrong product is named: {:?}",
        evaluation.reasons
    );
}

#[test]
fn minimum_direct_version_drift_identity_movement_stales_with_named_reasons() {
    // Negative control 8: every identity movement — MSRV, manifests,
    // lock, or package roots — invalidates the evidence and names the
    // exact movement.
    let current = receipt(
        "cargo-allow",
        vec![row("serde", MinimumFloorResultV1::Proven)],
    );

    let mut msrv_moved = observation("cargo-allow");
    msrv_moved.msrv = "1.96".to_string();
    let evaluation = evaluate_minimum_version_drift(&msrv_moved, Some(&current));
    assert_eq!(evaluation.verdict, MinimumVersionDriftVerdictV1::Stale);
    assert!(evaluation.blocking, "release-set drift blocks");
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("msrv moved: receipt 1.95 vs current 1.96")),
        "the msrv movement is named: {:?}",
        evaluation.reasons
    );

    let mut manifests_moved = observation("cargo-allow");
    manifests_moved.manifest_set_digest = "manifests-moved".to_string();
    let evaluation = evaluate_minimum_version_drift(&manifests_moved, Some(&current));
    assert_eq!(evaluation.verdict, MinimumVersionDriftVerdictV1::Stale);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("manifests moved")),
        "the manifest movement is named: {:?}",
        evaluation.reasons
    );

    let mut lock_moved = observation("cargo-allow");
    lock_moved.lock_digest = "lock-moved".to_string();
    let evaluation = evaluate_minimum_version_drift(&lock_moved, Some(&current));
    assert_eq!(evaluation.verdict, MinimumVersionDriftVerdictV1::Stale);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("Cargo.lock moved")),
        "the lock movement is named: {:?}",
        evaluation.reasons
    );

    let mut roots_moved = observation("cargo-allow");
    roots_moved.package_roots = vec!["allow-core".to_string()];
    let evaluation = evaluate_minimum_version_drift(&roots_moved, Some(&current));
    assert_eq!(evaluation.verdict, MinimumVersionDriftVerdictV1::Stale);
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("package roots moved")),
        "the roots movement is named: {:?}",
        evaluation.reasons
    );
}

#[test]
fn minimum_direct_version_drift_coverage_gaps_are_named_in_both_directions() {
    // A declared floor without a receipt row and a receipt row that is
    // no longer declared are both named, and the evaluation is
    // incomplete rather than silently passing.
    let mut current = observation("cargo-allow");
    current.floors = vec![floor_row("serde"), floor_row("clap")];
    let stale_row = receipt(
        "cargo-allow",
        vec![
            row("serde", MinimumFloorResultV1::Proven),
            row("rayon", MinimumFloorResultV1::Proven),
        ],
    );
    let evaluation = evaluate_minimum_version_drift(&current, Some(&stale_row));
    assert_eq!(evaluation.verdict, MinimumVersionDriftVerdictV1::Incomplete);
    assert!(evaluation.blocking, "release-set incompleteness blocks");
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("no receipt row for declared floor clap")),
        "the missing floor row is named: {:?}",
        evaluation.reasons
    );
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("receipt row rayon is no longer a declared floor")),
        "the orphaned receipt row is named: {:?}",
        evaluation.reasons
    );
}

#[test]
fn minimum_direct_version_drift_non_clean_rows_stay_visible() {
    // Unsupported and instrument-failed rows never default to
    // current-lock success: the evaluation stays incomplete and names
    // the dispositions.
    let failing = receipt(
        "cargo-allow",
        vec![row("serde", MinimumFloorResultV1::UnsupportedCombination)],
    );
    let evaluation = evaluate_minimum_version_drift(&observation("cargo-allow"), Some(&failing));
    assert_eq!(evaluation.verdict, MinimumVersionDriftVerdictV1::Incomplete);
    assert!(evaluation.blocking, "release-set non-clean rows block");
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("non-clean dispositions: serde")),
        "the non-clean disposition is named: {:?}",
        evaluation.reasons
    );
}

#[test]
fn minimum_direct_version_drift_aligned_receipt_is_current() {
    let evaluation = evaluate_minimum_version_drift(
        &observation("cargo-allow"),
        Some(&receipt(
            "cargo-allow",
            vec![row("serde", MinimumFloorResultV1::Proven)],
        )),
    );
    assert_eq!(evaluation.verdict, MinimumVersionDriftVerdictV1::Current);
    assert!(!evaluation.blocking, "a current release set does not block");
    assert!(evaluation.reasons.is_empty());
    assert_eq!(
        evaluation.schema_id,
        "cargo-allow.minimum-direct-version-drift.v1"
    );
}

#[test]
fn minimum_direct_version_drift_legacy_roots_binding_stays_optional() {
    // A receipt that predates root recording (empty roots) carries no
    // roots binding; the observation's roots govern coverage alone.
    let mut legacy = receipt("shared", vec![row("serde", MinimumFloorResultV1::Proven)]);
    legacy.package_roots = Vec::new();
    let evaluation = evaluate_minimum_version_drift(&observation("shared"), Some(&legacy));
    assert_eq!(
        evaluation.verdict,
        MinimumVersionDriftVerdictV1::Current,
        "no roots movement is invented for a legacy receipt: {:?}",
        evaluation.reasons
    );
}

#[test]
fn minimum_direct_version_drift_blocking_follows_the_release_set_only() {
    // Negative control 10: the identical stale state blocks for the
    // cargo-allow release set and stays advisory for a report-only
    // product.
    for product in ["cargo-allow", "shared", "cargo-intent", "cargo-proof"] {
        let mut moved = observation(product);
        moved.manifest_set_digest = "manifests-moved".to_string();
        let evaluation = evaluate_minimum_version_drift(
            &moved,
            Some(&receipt(
                product,
                vec![row("serde", MinimumFloorResultV1::Proven)],
            )),
        );
        assert_eq!(evaluation.verdict, MinimumVersionDriftVerdictV1::Stale);
        assert_eq!(
            evaluation.blocking,
            product == DRIFT_RELEASE_SET_PRODUCT,
            "only the release set blocks ({product})"
        );
    }
}

#[test]
fn minimum_direct_version_drift_retained_receipts_are_current_with_the_live_tree() {
    // The live drift guard: every retained receipt must grade current
    // against the live manifest-derived observation. Any dependency,
    // feature, target, MSRV, package-set, or manifest movement without
    // a regenerated receipt fails right here, naming the drift.
    let root = std::path::PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"),
    )
    .join("../..")
    .canonicalize()
    .expect("workspace root resolves");
    let msrv = workspace_msrv(&root).expect("the workspace MSRV derives");
    for product in crate::minimum_version_selection::products() {
        let selection = derive_selection(&root, product)
            .unwrap_or_else(|error| panic!("the {product} selection derives: {error}"));
        let observation = MinimumVersionDriftObservationV1 {
            product: product.to_string(),
            package_roots: selection.roots.clone(),
            msrv: msrv.clone(),
            manifest_set_digest: manifest_set_digest(&root).expect("the digest derives"),
            lock_digest: crate::minimum_version_selection::lock_digest(&root)
                .expect("the digest derives"),
            floors: selection
                .floors
                .iter()
                .map(
                    |(package, requirement, floor)| MinimumVersionDriftFloorRowV1 {
                        package: package.clone(),
                        declared_requirement: requirement.clone(),
                        selected_floor: floor.clone(),
                    },
                )
                .collect(),
        };
        let receipt_text = std::fs::read_to_string(root.join(
            crate::minimum_version_selection::retained_receipt_path(product),
        ))
        .unwrap_or_else(|error| panic!("the retained {product} receipt reads: {error}"));
        let receipt: MinimumVersionProofReceiptV1 = serde_json::from_str(&receipt_text)
            .unwrap_or_else(|error| panic!("the retained {product} receipt parses: {error}"));

        let evaluation = evaluate_minimum_version_drift(&observation, Some(&receipt));
        assert_eq!(
            evaluation.verdict,
            MinimumVersionDriftVerdictV1::Current,
            "the retained {product} proof drifted from the live tree: {:?}",
            evaluation.reasons
        );
        assert!(!evaluation.blocking, "a current product never blocks");
        assert!(
            !receipt.rows.is_empty(),
            "the {product} receipt certifies a non-empty floor inventory"
        );
        assert_eq!(
            receipt.rows.len(),
            observation.floors.len(),
            "the {product} receipt covers exactly the product's own floors"
        );
    }
}
