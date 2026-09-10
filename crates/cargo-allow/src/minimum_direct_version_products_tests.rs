//! Product-isolation tests for the #3903 PR C advisory product rows:
//! one product's unsupported floor never fails another product's
//! evaluation, each product receipt certifies exactly its own package
//! closure and declared floors (derived from the checked-in manifests
//! through the shared selection module), report-only products stay
//! advisory (check is their bounded proof class and their receipts
//! claim no unexecuted commands), and the shell registry stays the
//! exact mirror of the shared registry.

use allow_report::{
    DirectDependencyClassV1, MinimumFloorResultV1, MinimumFloorRowV1, MinimumProofVerdictV1,
    MinimumVersionProofReceiptV1, MinimumVersionProofRequestV1, MinimumVersionRowResultV1,
    evaluate_minimum_version_proof,
};

use crate::minimum_version_selection::{
    derive_selection as shared_derive_selection, product_roots as shared_product_roots,
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
        package_roots: vec![format!("{product}-root")],
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

fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"),
    )
    .join("../..")
    .canonicalize()
    .expect("workspace root resolves")
}

fn product_roots(product: &str) -> &'static [&'static str] {
    shared_product_roots(product).expect("test products stay registered")
}

/// The tests' view of the shared selection: (package, requirement,
/// floor) triples plus the closure membership.
struct ProductSelection {
    closure: Vec<String>,
    floors: Vec<(String, String, String)>,
}

fn derive_selection(root: &std::path::Path, product: &str) -> ProductSelection {
    let shared = shared_derive_selection(root, product)
        .unwrap_or_else(|error| panic!("the {product} selection derives: {error}"));
    ProductSelection {
        closure: shared.closure,
        floors: shared.floors,
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
        MinimumProofVerdictV1::Incomplete,
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
        MinimumProofVerdictV1::Complete,
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
fn minimum_direct_version_products_reject_a_foreign_product_receipt() {
    // A shared request evaluated against a cargo-allow receipt with
    // every other input aligned stays Incomplete and names both the
    // product and package-roots mismatch: an evaluator that ignored
    // receipt.product (or the roots) would wrongly pass this pairing.
    let shared_request = request("shared", vec![floor("toml", "1.1", "1.1.4")]);
    let mut foreign = receipt(
        "cargo-allow",
        vec![row("toml", "1.1", "1.1.4", MinimumFloorResultV1::Proven)],
    );
    foreign.manifest_set_digest = shared_request.manifest_set_digest.clone();
    foreign.lock_digest = shared_request.lock_digest.clone();
    let evaluation = evaluate_minimum_version_proof(&shared_request, &foreign);
    assert_eq!(
        evaluation.verdict,
        MinimumProofVerdictV1::Incomplete,
        "a foreign product's receipt never satisfies a shared request"
    );
    assert!(
        evaluation.reasons.iter().any(
            |reason| reason.contains("product_mismatch: receipt cargo-allow vs request shared")
        ),
        "the product mismatch is named: {:?}",
        evaluation.reasons
    );
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("package_roots_mismatch")),
        "the roots mismatch is named alongside the product: {:?}",
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
            MinimumProofVerdictV1::Complete,
            "an advisory product with a clean floor still proves"
        );
    }
}

#[test]
fn minimum_direct_version_products_script_registry_matches() {
    // The shell registry is the proof lane's authority; the shared
    // selection module's registry must stay its exact mirror, and the
    // shell must fail closed on an unregistered product rather than
    // mint a label-only receipt.
    let script = std::fs::read_to_string(workspace_root().join("scripts/proof-direct-floors.sh"))
        .expect("the proof script reads");
    let body_start = script
        .find("product_roots() {")
        .expect("the shell registry function is present");
    let body_end = script
        .match_indices("\n}")
        .map(|(index, _)| index)
        .find(|&index| index > body_start)
        .expect("the shell registry function closes");
    // Containment is checked by byte position between the two markers
    // instead of slicing, so a malformed marker fails the test rather
    // than panicking on a split character boundary.
    let in_registry_body = |needle: &str| {
        script
            .match_indices(needle)
            .any(|(index, _)| index > body_start && index < body_end)
    };
    for (product, roots) in crate::minimum_version_selection::PRODUCT_ROOTS {
        for package_root in *roots {
            assert!(
                in_registry_body(package_root),
                "the shell registry must map {product} to root {package_root}"
            );
        }
    }
    assert!(
        in_registry_body("*)") && in_registry_body("return 1"),
        "an unregistered product must fail closed in the shell registry"
    );
}

#[test]
fn minimum_direct_version_products_advisory_collector_pins_each_product() {
    // The advisory collector must pin the product identity, receipt
    // path, and check-only class per product: an inherited
    // CI_PROOF_PRODUCT must never make one product's proof write
    // another product's receipt.
    let script =
        std::fs::read_to_string(workspace_root().join("scripts/proof-advisory-products.sh"))
            .expect("the advisory collector reads");
    assert!(
        script.contains("export CI_PROOF_CLASSES=\"check\""),
        "the collector pins the check-only class"
    );
    assert!(
        script.contains("for product in shared cargo-intent cargo-proof; do"),
        "the collector iterates exactly the advisory products"
    );
    assert!(
        script.contains("export CI_PROOF_PRODUCT=\"$product\""),
        "the collector pins the product identity inside its product loop"
    );
    assert!(
        script.contains("direct-floor-proof-$product-v1.json"),
        "the collector names each product's own receipt artifact"
    );
}

#[test]
fn minimum_direct_version_products_retained_advisory_receipts_are_clean() {
    // The retained advisory product receipts (#3903 PR C): each one
    // certifies exactly its own product's package closure and declared
    // floors as derived from the checked-in manifests, carries the
    // product's roots, proves every row at its floor under 1.95, and
    // claims only the commands the advisory check class executed.
    let root = workspace_root();
    for product in ["shared", "cargo-intent", "cargo-proof"] {
        let path = root
            .join("docs/ci/receipts")
            .join(format!("direct-floor-proof-{product}-v1.json"));
        let text = std::fs::read_to_string(&path)
            .expect("the retained advisory receipt is present in the tree");
        let receipt: MinimumVersionProofReceiptV1 =
            serde_json::from_str(&text).expect("the retained receipt parses");
        let selection = derive_selection(&root, product);

        assert_eq!(receipt.product, product);
        assert_eq!(receipt.msrv, "1.95");
        assert_eq!(receipt.toolchain, "1.95.0");
        assert_eq!(
            receipt.package_roots,
            product_roots(product)
                .iter()
                .map(|name| (*name).to_string())
                .collect::<Vec<_>>(),
            "the retained {product} receipt records the product's own roots"
        );
        for package_root in product_roots(product) {
            assert!(
                selection
                    .closure
                    .iter()
                    .any(|member| member == package_root),
                "the derived {product} closure retains its own root {package_root}"
            );
        }

        assert!(
            !receipt.rows.is_empty(),
            "the {product} receipt certifies a non-empty floor inventory"
        );
        assert_eq!(
            receipt.rows.len(),
            selection.floors.len(),
            "the {product} receipt certifies exactly the product's own floors, not another product's: {:?} vs {:?}",
            receipt
                .rows
                .iter()
                .map(|row| &row.package)
                .collect::<Vec<_>>(),
            selection
                .floors
                .iter()
                .map(|floor| &floor.0)
                .collect::<Vec<_>>(),
        );
        for (row, (package, requirement, floor)) in receipt.rows.iter().zip(&selection.floors) {
            assert_eq!(
                row.package, *package,
                "the {product} rows stay in inventory order"
            );
            assert_eq!(row.declared_requirement, *requirement);
            assert_eq!(row.tested_floor, *floor);
            assert_eq!(
                row.result,
                MinimumFloorResultV1::Proven,
                "every retained {product} row proved at its floor"
            );
            assert!(row.limitation.is_none());
        }

        // Advisory posture: check-only execution evidence, no test or
        // package claims, and a claim boundary scoped to this product
        // rather than the cargo-allow release set.
        assert!(
            receipt
                .commands
                .iter()
                .any(|command| command.starts_with("cargo check --locked")),
            "the {product} receipt records the check class it ran"
        );
        assert!(
            receipt
                .commands
                .iter()
                .all(|command| !command.starts_with("cargo test")
                    && !command.starts_with("cargo package")),
            "an advisory check-only receipt must not claim test or package runs: {:?}",
            receipt.commands
        );
        assert!(
            receipt.claim_boundary.contains(product)
                && receipt.claim_boundary.contains("Advisory, report-only"),
            "the {product} claim boundary names the product and its advisory posture"
        );
        assert!(
            !receipt
                .claim_boundary
                .contains("release set's declared direct"),
            "the {product} receipt must not claim the cargo-allow release set"
        );
        assert!(receipt.floor_lock_digest.starts_with("sha256:v1:"));
    }
}

#[test]
fn minimum_direct_version_products_cargo_allow_receipt_certifies_its_closure() {
    // The retained release-set receipt (regenerated by PR D with the
    // product-scoped script): it records the cargo-allow roots, runs
    // the full check/test/package classes over the closure, and its
    // rows are exactly the cargo-allow product closure's declared
    // floors derived from the checked-in manifests.
    let root = workspace_root();
    let text = std::fs::read_to_string(
        root.join("docs/ci/receipts/direct-floor-proof-cargo-allow-v1.json"),
    )
    .expect("the retained release-set receipt is present");
    let receipt: MinimumVersionProofReceiptV1 =
        serde_json::from_str(&text).expect("the retained receipt parses");
    let selection = derive_selection(&root, "cargo-allow");
    assert_eq!(
        receipt.package_roots,
        product_roots("cargo-allow")
            .iter()
            .map(|name| (*name).to_string())
            .collect::<Vec<_>>(),
        "the release-set receipt records the cargo-allow roots"
    );
    assert!(
        receipt
            .commands
            .iter()
            .any(|command| command.starts_with("cargo test --locked")),
        "the release-set receipt ran the test class"
    );
    assert!(
        receipt
            .commands
            .iter()
            .any(|command| command.starts_with("cargo package -p")),
        "the release-set receipt ran the package class"
    );
    assert_eq!(
        receipt.rows.len(),
        selection.floors.len(),
        "the release-set receipt certifies exactly the cargo-allow closure's floors"
    );
    for (row, (package, _, floor)) in receipt.rows.iter().zip(&selection.floors) {
        assert_eq!(row.package, *package);
        assert_eq!(row.tested_floor, *floor);
    }
}
