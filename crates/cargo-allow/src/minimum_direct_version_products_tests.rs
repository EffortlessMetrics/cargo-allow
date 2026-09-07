//! Product-isolation tests for the #3903 PR C advisory product rows:
//! one product's unsupported floor never fails another product's
//! evaluation, each product set carries its own MSRV and manifest/lock
//! identity, and report-only products stay advisory (check is their
//! bounded proof class; test/package enforcement requires the owning
//! package-family authority).

use allow_report::{
    DirectDependencyClassV1, MinimumFloorResultV1, MinimumFloorRowV1, MinimumVersionProofReceiptV1,
    MinimumVersionProofRequestV1, MinimumVersionRowResultV1, evaluate_minimum_version_proof,
};

fn floor(package: &str, requirement: &str, selected: &str) -> MinimumFloorRowV1 {
    MinimumFloorRowV1 {
        package: package.to_string(),
        declared_requirement: requirement.to_string(),
        selected_floor: selected.to_string(),
        class: DirectDependencyClassV1::Normal,
    }
}

fn row(
    package: &str,
    requirement: &str,
    tested: &str,
    result: MinimumFloorResultV1,
) -> MinimumVersionRowResultV1 {
    MinimumVersionRowResultV1 {
        package: package.to_string(),
        declared_requirement: requirement.to_string(),
        tested_floor: tested.to_string(),
        resolved_version: tested.to_string(),
        source_identity: "registry:crates.io".to_string(),
        result,
        limitation: if result.is_clean() {
            None
        } else {
            Some("bounded limitation recorded".to_string())
        },
    }
}

fn request(product: &str, floors: Vec<MinimumFloorRowV1>) -> MinimumVersionProofRequestV1 {
    MinimumVersionProofRequestV1 {
        product: product.to_string(),
        package_roots: vec![format!("{product}-root")],
        features: vec!["default".to_string()],
        target: "x86_64-unknown-linux-gnu".to_string(),
        msrv: "1.95".to_string(),
        floors,
        proof_classes: vec!["check".to_string()],
        manifest_set_digest: format!("{product}-manifests"),
        lock_digest: format!("{product}-lock"),
    }
}

fn receipt(product: &str, rows: Vec<MinimumVersionRowResultV1>) -> MinimumVersionProofReceiptV1 {
    MinimumVersionProofReceiptV1 {
        schema_id: "cargo-allow.minimum-direct-version.v1".to_string(),
        schema_version: 1,
        product: product.to_string(),
        msrv: "1.95".to_string(),
        toolchain: "1.95.0".to_string(),
        target: "x86_64-unknown-linux-gnu".to_string(),
        manifest_set_digest: format!("{product}-manifests"),
        lock_digest: format!("{product}-lock"),
        rows,
        commands: vec!["cargo check --locked".to_string()],
        floor_lock_digest: "sha256:v1:floor-lock".to_string(),
        limitations: Vec::new(),
        claim_boundary: "bounded".to_string(),
    }
}

#[test]
fn minimum_direct_version_products_stay_isolated() {
    // The cargo-allow release set carries an unsupported floor while
    // the shared product proves clean: the shared product's evaluation
    // stays Complete and the cargo-allow evaluation stays Incomplete —
    // no cross-product leakage in either direction.
    let cargo_allow = evaluate_minimum_version_proof(
        &request("cargo-allow", vec![floor("serde", "1.0", "1.0.150")]),
        &receipt(
            "cargo-allow",
            vec![row(
                "serde",
                "1.0",
                "1.0.150",
                MinimumFloorResultV1::FloorTooLow,
            )],
        ),
    );
    assert_eq!(
        cargo_allow.verdict,
        allow_report::MinimumProofVerdictV1::Incomplete,
        "the release set's FloorTooLow stays its own"
    );

    let shared = evaluate_minimum_version_proof(
        &request("shared", vec![floor("toml", "1.1", "1.1.4")]),
        &receipt(
            "shared",
            vec![row("toml", "1.1", "1.1.4", MinimumFloorResultV1::Proven)],
        ),
    );
    assert_eq!(
        shared.verdict,
        allow_report::MinimumProofVerdictV1::Complete,
        "the shared product's clean floor is independent"
    );
}

#[test]
fn minimum_direct_version_products_identify_their_own_rows() {
    // A receipt row that names a package from a different product's
    // closure is named as outside the request's selected floors.
    let evaluation = evaluate_minimum_version_proof(
        &request("shared", vec![floor("toml", "1.1", "1.1.4")]),
        &receipt(
            "shared",
            vec![row(
                "cargo-allow-only-dep",
                "1.0",
                "1.0.1",
                MinimumFloorResultV1::Proven,
            )],
        ),
    );
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("not in the request's selected floors")),
        "the foreign row is named as missing, not silently accepted: {:?}",
        evaluation.reasons
    );
}

#[test]
fn minimum_direct_version_products_keep_advisory_posture() {
    // Report-only products (shared, cargo-intent, cargo-proof) stay
    // advisory: their receipts carry the check-only proof class and
    // their results never gate the cargo-allow release set.
    for product in ["shared", "cargo-intent", "cargo-proof"] {
        let request = request(product, vec![floor("toml", "1.1", "1.1.4")]);
        assert_eq!(request.proof_classes, vec!["check".to_string()]);
        let receipt = receipt(
            product,
            vec![row("toml", "1.1", "1.1.4", MinimumFloorResultV1::Proven)],
        );
        let evaluation = evaluate_minimum_version_proof(&request, &receipt);
        assert_eq!(
            evaluation.verdict,
            allow_report::MinimumProofVerdictV1::Complete,
            "an advisory product with a clean floor still proves"
        );
    }
}

#[test]
fn minimum_direct_version_products_retained_advisory_receipts_are_clean() {
    // The retained advisory product receipts (#3903 PR C): the shared,
    // cargo-intent, and cargo-proof floors proved under the 1.95
    // toolchain, each in its own receipt with its own identity.
    let root = std::path::PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"),
    )
    .join("../..")
    .canonicalize()
    .expect("workspace root resolves");
    for product in ["shared", "cargo-intent", "cargo-proof"] {
        let path = root
            .join("docs/ci/receipts")
            .join(format!("direct-floor-proof-{product}-v1.json"));
        let text = std::fs::read_to_string(&path)
            .expect("the retained advisory receipt is present in the tree");
        let receipt: MinimumVersionProofReceiptV1 =
            serde_json::from_str(&text).expect("the retained receipt parses");
        assert_eq!(receipt.product, product);
        assert_eq!(receipt.msrv, "1.95");
        assert!(
            receipt.rows.iter().all(|row| row.result.is_clean()),
            "the retained {product} window proved clean"
        );
        assert!(receipt.floor_lock_digest.starts_with("sha256:v1:"));
    }
}
