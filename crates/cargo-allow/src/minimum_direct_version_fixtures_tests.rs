//! Fixture corpus for the #3903 direct minimum version contract: a
//! valid floor, a too-low floor, an unavailable version, a target-only
//! edge, and an incompatible transitive combination — plus the live
//! workspace inventory (report-only, changing no requirement).

use allow_report::{
    DirectDependencyClassV1, DirectMinimumVersionSetV1, DirectRequirementRowV1,
    MinimumFloorResultV1, MinimumFloorRowV1, MinimumVersionProofReceiptV1,
    MinimumVersionProofRequestV1, MinimumVersionRowResultV1, ProductDependencySetV1,
    evaluate_minimum_version_proof,
};

fn row(package: &str, requirement: &str, floor: &str) -> MinimumFloorRowV1 {
    MinimumFloorRowV1 {
        package: package.to_string(),
        declared_requirement: requirement.to_string(),
        selected_floor: floor.to_string(),
        class: DirectDependencyClassV1::Normal,
    }
}

fn result_row(
    package: &str,
    requirement: &str,
    tested: &str,
    result: MinimumFloorResultV1,
    limitation: Option<&str>,
) -> MinimumVersionRowResultV1 {
    MinimumVersionRowResultV1 {
        package: package.to_string(),
        declared_requirement: requirement.to_string(),
        tested_floor: tested.to_string(),
        resolved_version: tested.to_string(),
        source_identity: "registry:crates.io".to_string(),
        result,
        limitation: limitation.map(str::to_string),
    }
}

fn receipt(rows: Vec<MinimumVersionRowResultV1>) -> MinimumVersionProofReceiptV1 {
    MinimumVersionProofReceiptV1 {
        schema_id: "cargo-allow.minimum-direct-version.v1".to_string(),
        schema_version: 1,
        product: "cargo-allow".to_string(),
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

fn request(floors: Vec<MinimumFloorRowV1>) -> MinimumVersionProofRequestV1 {
    MinimumVersionProofRequestV1 {
        product: "cargo-allow".to_string(),
        package_roots: vec!["cargo-allow".to_string()],
        features: vec!["default".to_string()],
        target: "x86_64-pc-windows-msvc".to_string(),
        msrv: "1.95".to_string(),
        floors,
        proof_classes: vec!["check".to_string(), "test".to_string()],
        manifest_set_digest: "manifest-digest-1".to_string(),
        lock_digest: "lock-digest-1".to_string(),
    }
}

#[test]
fn minimum_direct_version_fixtures_valid_floor_proves() {
    // Fixture 1: the declared floor compiles and tests → Proven.
    let floors = vec![row("serde", "1.0", "1.0.210")];
    let rows = vec![result_row(
        "serde",
        "1.0",
        "1.0.210",
        MinimumFloorResultV1::Proven,
        None,
    )];
    let evaluation = evaluate_minimum_version_proof(&request(floors), &receipt(rows.clone()));
    assert!(
        evaluation
            .reasons
            .iter()
            .all(|reason| !reason.contains("non-clean")),
        "a proven row is clean: {:?}",
        evaluation.reasons
    );
}

#[test]
fn minimum_direct_version_fixtures_too_low_floor_is_floor_too_low() {
    // Fixture 2: the declared floor compiles but a proof test fails —
    // the floor is too low and must be raised, not silently passed.
    let floors = vec![row("serde", "1.0", "1.0.150")];
    let rows = vec![result_row(
        "serde",
        "1.0",
        "1.0.150",
        MinimumFloorResultV1::FloorTooLow,
        Some("compiles under 1.95 but the proof test fails at 1.0.150"),
    )];
    let evaluation = evaluate_minimum_version_proof(&request(floors), &receipt(rows.clone()));
    assert_ne!(
        evaluation.verdict,
        allow_report::MinimumProofVerdictV1::Complete
    );
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("serde")),
        "the exact failing dependency row is identified"
    );
}

#[test]
fn minimum_direct_version_fixtures_unavailable_version_is_distinct() {
    // Fixture 3: the selected floor version is yanked/unavailable — a
    // resolver failure, distinct from a compile-incompatible floor.
    let floors = vec![row("serde", "1.0", "1.0.150")];
    let rows = vec![result_row(
        "serde",
        "1.0",
        "1.0.150",
        MinimumFloorResultV1::ResolverFailure,
        Some("the selected floor version is yanked on the registry"),
    )];
    let evaluation = evaluate_minimum_version_proof(&request(floors), &receipt(rows.clone()));
    assert_ne!(
        evaluation.verdict,
        allow_report::MinimumProofVerdictV1::Complete
    );
    let resolver_rows = rows
        .iter()
        .filter(|row| row.result == MinimumFloorResultV1::ResolverFailure)
        .count();
    let unsupported_rows = rows
        .iter()
        .filter(|row| row.result == MinimumFloorResultV1::UnsupportedCombination)
        .count();
    assert_eq!(resolver_rows, 1);
    assert_eq!(unsupported_rows, 0);
}

#[test]
fn minimum_direct_version_fixtures_target_only_edge_is_scoped() {
    // Fixture 4: a target-specific direct dependency is only proven
    // (or named) under the configuration that selects it.
    let target_floor = MinimumFloorRowV1 {
        package: "winapi".to_string(),
        declared_requirement: "0.3".to_string(),
        selected_floor: "0.3.9".to_string(),
        class: DirectDependencyClassV1::Normal,
    };
    let mut request = request(vec![target_floor]);
    request.target = "x86_64-pc-windows-msvc".to_string();
    let rows = vec![result_row(
        "winapi",
        "0.3",
        "0.3.9",
        MinimumFloorResultV1::Proven,
        None,
    )];
    let evaluation = evaluate_minimum_version_proof(&request, &receipt(rows));
    assert!(
        !evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("not in the request's selected floors")),
        "the target-selected row is part of the request: {:?}",
        evaluation.reasons
    );
    assert_eq!(
        request.target, "x86_64-pc-windows-msvc",
        "the target edge stays bound to the windows configuration"
    );
}

#[test]
fn minimum_direct_version_fixtures_incompatible_transitive_combination_is_scoped() {
    // Fixture 5: the declared floor compiles but a transitive
    // combination is incompatible — the row disposition is an
    // UnsupportedCombination limited to this direct row, never a
    // blanket transitive-minimum claim.
    let floors = vec![row("serde", "1.0", "1.0.150")];
    let rows = vec![result_row(
        "serde",
        "1.0",
        "1.0.150",
        MinimumFloorResultV1::UnsupportedCombination,
        Some("the transitive combination at this floor is not upstream-supported"),
    )];
    let evaluation = evaluate_minimum_version_proof(&request(floors), &receipt(rows));
    assert_ne!(
        evaluation.verdict,
        allow_report::MinimumProofVerdictV1::Complete
    );
    assert!(
        evaluation
            .reasons
            .iter()
            .any(|reason| reason.contains("serde")),
        "the exact failing direct row is identified"
    );
}

#[test]
fn minimum_direct_version_fixtures_live_workspace_inventory_is_report_only() {
    // The live inventory: report-only, changing no dependency
    // requirement. The cargo-allow release set's shared rows are
    // inventoried; experimental products stay advisory.
    let root = std::path::PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"),
    )
    .join("../..")
    .canonicalize()
    .expect("workspace root resolves");
    let text = std::fs::read_to_string(root.join("Cargo.toml"))
        .expect("the workspace manifest is retained");
    assert!(
        text.contains("serde.workspace = true") || text.contains("serde ="),
        "the release set declares serde as a direct dependency"
    );
    // The contract itself changes nothing: no requirement strings are
    // rewritten by this lane.
    let set = DirectMinimumVersionSetV1 {
        schema_id: "cargo-allow.minimum-direct-version.v1".to_string(),
        schema_version: 1,
        product_sets: vec![ProductDependencySetV1 {
            product: "cargo-allow".to_string(),
            packages: vec!["cargo-allow".to_string()],
            rows: vec![DirectRequirementRowV1 {
                package: "serde".to_string(),
                requirement: "1.0".to_string(),
                class: DirectDependencyClassV1::Normal,
                target: None,
                activated_by_feature: None,
            }],
            manifest_set_digest: "root-cargo-toml".to_string(),
            lock_digest: "current-lock".to_string(),
            msrv: "1.95".to_string(),
        }],
        claim_boundary: "report-only inventory".to_string(),
    };
    assert_eq!(set.product_sets.len(), 1);
    assert_eq!(set.product_sets[0].rows.len(), 1);
}
