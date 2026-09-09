//! Base/head dependency graph delta compiler tests (#3920 PR B): the
//! compiler correctly distinguishes upgrade, downgrade, source
//! change, lock-only movement, and transitive replacement from the
//! falsifying fixture corpus; deterministic inputs produce
//! deterministic outputs.

use crate::artifacts::dependency_graph_delta_v1::{
    DependencyGraphDeltaIdentityV1, DependencyGraphDeltaKindV1,
};
use crate::artifacts::dependency_graph_delta_compiler::compile_dependency_graph_delta;
use crate::artifacts::dependency_graph_delta_v1::dependency_graph_delta_fixtures;

#[test]
fn dependency_graph_delta_compiler_classifies_fixtures() {
    // Each fixture's base/head pair produces exactly the expected
    // kind. The compiler must distinguish every family from the
    // falsifying corpus.
    let fixtures = dependency_graph_delta_fixtures();
    assert!(!fixtures.is_empty());
    for fixture in &fixtures {
        let identity = default_identity();
        let receipt = compile_dependency_graph_delta(
            &identity,
            &fixture.base_manifest,
            &fixture.head_manifest,
            &fixture.base_lock,
            &fixture.head_lock,
        )
        .expect("compilation succeeds for well-formed inputs");
        assert!(
            receipt
                .rows
                .iter()
                .any(|row| row.kind == fixture.expected_kind),
            "fixture {}: expected kind {} in rows {:?}",
            fixture.id,
            fixture.expected_kind.as_str(),
            receipt.rows
        );
    }
}

#[test]
fn dependency_graph_delta_compiler_is_deterministic() {
    // Same inputs always produce the same output.
    let fixtures = dependency_graph_delta_fixtures();
    let fixture = fixtures.first().expect("the fixture corpus is non-empty");
    let identity = default_identity();
    let first = compile_dependency_graph_delta(
        &identity,
        &fixture.base_manifest,
        &fixture.head_manifest,
        &fixture.base_lock,
        &fixture.head_lock,
    )
    .expect("first compilation succeeds");
    let second = compile_dependency_graph_delta(
        &identity,
        &fixture.base_manifest,
        &fixture.head_manifest,
        &fixture.base_lock,
        &fixture.head_lock,
    )
    .expect("second compilation succeeds");
    assert_eq!(first, second);
}

#[test]
fn dependency_graph_delta_compiler_detects_upgrade() {
    let identity = default_identity();
    let receipt = compile_dependency_graph_delta(
        &identity,
        "[dependencies]\nserde = \"1\"\n",
        "[dependencies]\nserde = \"1\"\n",
        "[[package]]\nname = \"serde\"\nversion = \"1.0.200\"\nsource = \"registry\"\nchecksum = \"old\"\n",
        "[[package]]\nname = \"serde\"\nversion = \"1.0.228\"\nsource = \"registry\"\nchecksum = \"new\"\n",
    )
    .expect("compilation succeeds");
    assert!(
        receipt
            .rows
            .iter()
            .any(|row| row.kind == DependencyGraphDeltaKindV1::PackageUpgraded
                && row.package_name == "serde"),
        "the upgrade is detected: {:?}",
        receipt.rows
    );
}

#[test]
fn dependency_graph_delta_compiler_detects_downgrade() {
    let identity = default_identity();
    let receipt = compile_dependency_graph_delta(
        &identity,
        "[dependencies]\ntoml = \"1\"\n",
        "[dependencies]\ntoml = \"0.8\"\n",
        "[[package]]\nname = \"toml\"\nversion = \"1.1.4\"\n",
        "[[package]]\nname = \"toml\"\nversion = \"0.8.1\"\n",
    )
    .expect("compilation succeeds");
    assert!(
        receipt
            .rows
            .iter()
            .any(|row| row.kind == DependencyGraphDeltaKindV1::DirectRequirementLowered),
        "the manifest requirement lowering is detected: {:?}",
        receipt.rows
    );
    assert!(
        receipt
            .rows
            .iter()
            .any(|row| row.kind == DependencyGraphDeltaKindV1::PackageDowngraded),
        "the lockfile downgrade is detected: {:?}",
        receipt.rows
    );
}

#[test]
fn dependency_graph_delta_compiler_detects_source_change() {
    let identity = default_identity();
    let receipt = compile_dependency_graph_delta(
        &identity,
        "[dependencies]\nwidget = \"1\"\n",
        "[dependencies]\nwidget = { git = \"https://github.com/example/widget\" }\n",
        "[[package]]\nname = \"widget\"\nversion = \"1.0.0\"\nsource = \"registry\"\nchecksum = \"old\"\n",
        "[[package]]\nname = \"widget\"\nversion = \"1.0.0\"\nsource = \"git+https://github.com/example/widget\"\nchecksum = \"new\"\n",
    )
    .expect("compilation succeeds");
    assert!(
        receipt
            .rows
            .iter()
            .any(|row| row.kind == DependencyGraphDeltaKindV1::SourceOrChecksumChanged),
        "the source change is detected: {:?}",
        receipt.rows
    );
}

#[test]
fn dependency_graph_delta_compiler_detects_transitive_replacement() {
    // Count parity does not establish graph identity: one removed
    // package and one unrelated added package must both be visible.
    let identity = default_identity();
    let receipt = compile_dependency_graph_delta(
        &identity,
        "",
        "",
        "[[package]]\nname = \"alpha\"\nversion = \"1.0\"\n\n[[package]]\nname = \"beta\"\nversion = \"2.0\"\n",
        "[[package]]\nname = \"alpha\"\nversion = \"1.0\"\n\n[[package]]\nname = \"gamma\"\nversion = \"2.0\"\n",
    )
    .expect("compilation succeeds");
    assert!(
        receipt
            .rows
            .iter()
            .any(|row| row.kind == DependencyGraphDeltaKindV1::PackageRemoved
                && row.package_name == "beta"),
        "the removed package is detected: {:?}",
        receipt.rows
    );
    assert!(
        receipt
            .rows
            .iter()
            .any(|row| row.kind == DependencyGraphDeltaKindV1::PackageAdded
                && row.package_name == "gamma"),
        "the added package is detected: {:?}",
        receipt.rows
    );
}

#[test]
fn dependency_graph_delta_compiler_handles_empty_inputs() {
    // Empty inputs produce an empty receipt, not a crash.
    let identity = default_identity();
    let receipt = compile_dependency_graph_delta(&identity, "", "", "", "")
        .expect("empty inputs compile without error");
    assert!(receipt.rows.is_empty());
}

fn default_identity() -> DependencyGraphDeltaIdentityV1 {
    DependencyGraphDeltaIdentityV1 {
        base_commit: "aaa111".to_string(),
        head_commit: "bbb222".to_string(),
        base_manifest_set_digest: "sha256:v1:base".to_string(),
        head_manifest_set_digest: "sha256:v1:head".to_string(),
        base_lock_digest: "sha256:v1:base-lock".to_string(),
        head_lock_digest: "sha256:v1:head-lock".to_string(),
        product: "cargo-allow".to_string(),
        target: "x86_64-unknown-linux-gnu".to_string(),
    }
}
