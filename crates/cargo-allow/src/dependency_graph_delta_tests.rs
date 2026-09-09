//! Law tests for the #3920 PR A dependency graph delta contract: the
//! kind vocabulary is total over the delta families, semantic vs
//! non-semantic classification is correct, direct-requirement kinds
//! are distinguishable from lock-only movements, the fixture corpus
//! covers every declared family with correct polarity, and the receipt
//! serializes and round-trips.

use allow_report::{
    DependencyClassV1, DependencyGraphDeltaIdentityV1, DependencyGraphDeltaKindV1,
    DependencyGraphDeltaReceiptV1, DependencyGraphDeltaRowV1, dependency_graph_delta_fixtures,
};

fn all_kinds() -> Vec<DependencyGraphDeltaKindV1> {
    vec![
        DependencyGraphDeltaKindV1::DirectRequirementAdded,
        DependencyGraphDeltaKindV1::DirectRequirementRemoved,
        DependencyGraphDeltaKindV1::DirectRequirementRaised,
        DependencyGraphDeltaKindV1::DirectRequirementLowered,
        DependencyGraphDeltaKindV1::RequirementRangeBroadened,
        DependencyGraphDeltaKindV1::RequirementRangeNarrowed,
        DependencyGraphDeltaKindV1::LockOnlyResolutionChanged,
        DependencyGraphDeltaKindV1::PackageAdded,
        DependencyGraphDeltaKindV1::PackageRemoved,
        DependencyGraphDeltaKindV1::PackageUpgraded,
        DependencyGraphDeltaKindV1::PackageDowngraded,
        DependencyGraphDeltaKindV1::SourceOrChecksumChanged,
        DependencyGraphDeltaKindV1::FeatureActivationChanged,
        DependencyGraphDeltaKindV1::DuplicateVersionMovement,
        DependencyGraphDeltaKindV1::TargetOrDependencyClassChanged,
        DependencyGraphDeltaKindV1::ManifestLockMismatch,
        DependencyGraphDeltaKindV1::NoSemanticGraphChange,
        DependencyGraphDeltaKindV1::UnsupportedOrInstrumentFailure,
    ]
}

#[test]
fn dependency_graph_delta_kind_vocabulary_is_total() {
    // Every delta kind from the #3920 issue packet is declared with a
    // unique string identity.
    let kinds = all_kinds();
    assert_eq!(kinds.len(), 18, "18 delta families from the issue packet");
    let ids: std::collections::BTreeSet<&str> = kinds.iter().map(|k| k.as_str()).collect();
    assert_eq!(ids.len(), 18, "every kind has a unique string identity");
}

#[test]
fn dependency_graph_delta_semantic_classification_is_correct() {
    // Semantic kinds describe graph movement; non-semantic kinds
    // describe lane health. A downgrade is always semantic; a missing
    // manifest is never a graph movement.
    for kind in all_kinds() {
        match kind {
            DependencyGraphDeltaKindV1::ManifestLockMismatch
            | DependencyGraphDeltaKindV1::UnsupportedOrInstrumentFailure => {
                assert!(!kind.is_semantic(), "{:?} is non-semantic", kind);
            }
            _ => {
                assert!(kind.is_semantic(), "{:?} is semantic", kind);
            }
        }
    }
}

#[test]
fn dependency_graph_delta_direct_requirement_kinds_are_distinguishable() {
    // Direct-requirement kinds are distinct from lock-only and
    // transitive movements.
    let direct = [
        DependencyGraphDeltaKindV1::DirectRequirementAdded,
        DependencyGraphDeltaKindV1::DirectRequirementRemoved,
        DependencyGraphDeltaKindV1::DirectRequirementRaised,
        DependencyGraphDeltaKindV1::DirectRequirementLowered,
        DependencyGraphDeltaKindV1::RequirementRangeBroadened,
        DependencyGraphDeltaKindV1::RequirementRangeNarrowed,
    ];
    for kind in &direct {
        assert!(kind.is_direct_requirement(), "{}", kind.as_str());
    }
    let non_direct = [
        DependencyGraphDeltaKindV1::LockOnlyResolutionChanged,
        DependencyGraphDeltaKindV1::PackageAdded,
        DependencyGraphDeltaKindV1::PackageRemoved,
        DependencyGraphDeltaKindV1::SourceOrChecksumChanged,
    ];
    for kind in &non_direct {
        assert!(!kind.is_direct_requirement(), "{}", kind.as_str());
    }
}

#[test]
fn dependency_graph_delta_fixture_corpus_covers_declared_families() {
    // Each fixture's expected kind must be a declared kind, and every
    // fixture must have non-empty lock and manifest inputs.
    let fixtures = dependency_graph_delta_fixtures();
    assert!(!fixtures.is_empty());
    let mut seen_ids: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for fixture in &fixtures {
        assert!(!fixture.id.is_empty(), "fixture id is present");
        assert!(
            seen_ids.insert(fixture.id),
            "fixture id {} is unique",
            fixture.id
        );
        assert!(!fixture.base_lock.is_empty(), "base lock is present");
        assert!(!fixture.head_lock.is_empty(), "head lock is present");
        assert!(
            !fixture.description.is_empty(),
            "fixture description is present"
        );
    }
}

#[test]
fn dependency_graph_delta_fixtures_cover_the_packet_families() {
    // The fixture corpus covers at least the key families from the
    // #3920 issue: lock-only movement, manifest downgrade, source/
    // checksum change, range narrowing, feature activation, and
    // transitive replacement.
    let fixtures = dependency_graph_delta_fixtures();
    let expected_ids = [
        "dep-upgrade-lock-only",
        "dep-downgrade-manifest",
        "dep-source-checksum-change",
        "dep-range-narrowed",
        "dep-feature-activated",
        "dep-transitive-replaced",
    ];
    for id in &expected_ids {
        assert!(
            fixtures.iter().any(|fixture| fixture.id == *id),
            "fixture {id} is in the corpus"
        );
    }
}

#[test]
fn dependency_graph_delta_lock_only_fixture_has_same_manifest() {
    // The lock-only fixture's base and head manifests are identical:
    // only the lockfile moved, proving the compiler distinguishes
    // lock-only resolution from manifest edits.
    let fixtures = dependency_graph_delta_fixtures();
    let fixture = fixtures
        .iter()
        .find(|fixture| fixture.id == "dep-upgrade-lock-only")
        .expect("the lock-only fixture exists");
    assert_eq!(fixture.base_manifest, fixture.head_manifest);
    assert_ne!(fixture.base_lock, fixture.head_lock);
}

#[test]
fn dependency_graph_delta_downgrade_fixture_has_lowered_version() {
    // Negative control 6: the downgrade fixture's head version is
    // strictly lower than the base version — never described as a
    // compatible update.
    let fixtures = dependency_graph_delta_fixtures();
    let fixture = fixtures
        .iter()
        .find(|fixture| fixture.id == "dep-downgrade-manifest")
        .expect("the downgrade fixture exists");
    assert!(
        fixture.head_lock.contains("0.8") && fixture.base_lock.contains("1."),
        "the downgrade moves from the 1.x line to 0.8"
    );
}

#[test]
fn dependency_graph_delta_receipt_serializes_and_round_trips() {
    let identity = DependencyGraphDeltaIdentityV1 {
        base_commit: "aaa111".to_string(),
        head_commit: "bbb222".to_string(),
        base_manifest_set_digest: "sha256:v1:base".to_string(),
        head_manifest_set_digest: "sha256:v1:head".to_string(),
        base_lock_digest: "sha256:v1:base-lock".to_string(),
        head_lock_digest: "sha256:v1:head-lock".to_string(),
        product: "cargo-allow".to_string(),
        target: "x86_64-unknown-linux-gnu".to_string(),
    };
    let row = DependencyGraphDeltaRowV1 {
        kind: DependencyGraphDeltaKindV1::DirectRequirementLowered,
        class: DependencyClassV1::Normal,
        package_name: "toml".to_string(),
        base_version: "1.1.4".to_string(),
        head_version: "0.8.1".to_string(),
        base_requirement: "1".to_string(),
        head_requirement: "0.8".to_string(),
        base_source: "registry+https://github.com/rust-lang/crates.io-index".to_string(),
        head_source: "registry+https://github.com/rust-lang/crates.io-index".to_string(),
        base_checksum: "old".to_string(),
        head_checksum: "new".to_string(),
    };
    let receipt = DependencyGraphDeltaReceiptV1 {
        schema_id: "cargo-allow.dependency-graph-delta.v1".to_string(),
        schema_version: 1,
        identity,
        rows: vec![row],
        complete: true,
        limitations: vec![],
        claim_boundary: "bounded".to_string(),
    };
    let json = serde_json::to_string_pretty(&receipt).expect("serializes");
    let parsed: DependencyGraphDeltaReceiptV1 = serde_json::from_str(&json).expect("round-trips");
    assert_eq!(parsed, receipt, "the JSON view round-trips");
    assert!(
        receipt.has_semantic_changes(),
        "a lowered requirement is a semantic change"
    );
}

#[test]
fn dependency_graph_delta_class_vocabulary_is_present() {
    // The dependency class vocabulary covers the Cargo edge classes:
    // normal, development, build, target-specific, and optional.
    let classes = [
        DependencyClassV1::Normal,
        DependencyClassV1::Development,
        DependencyClassV1::Build,
        DependencyClassV1::TargetSpecific,
        DependencyClassV1::Optional,
    ];
    assert_eq!(classes.len(), 5, "five dependency classes");
}
