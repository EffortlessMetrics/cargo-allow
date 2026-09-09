//! Base/head dependency graph delta compiler tests (#3920 PR B): the
//! compiler correctly distinguishes upgrade, downgrade, source
//! change, lock-only movement, and transitive replacement from the
//! falsifying fixture corpus; deterministic inputs produce
//! deterministic outputs.

use crate::{
    DependencyGraphDeltaIdentityV1, DependencyGraphDeltaKindV1, compile_dependency_graph_delta,
    dependency_graph_delta_fixtures,
};

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
            fixture.base_manifest,
            fixture.head_manifest,
            fixture.base_lock,
            fixture.head_lock,
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
        fixture.base_manifest,
        fixture.head_manifest,
        fixture.base_lock,
        fixture.head_lock,
    )
    .expect("first compilation succeeds");
    let second = compile_dependency_graph_delta(
        &identity,
        fixture.base_manifest,
        fixture.head_manifest,
        fixture.base_lock,
        fixture.head_lock,
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
        receipt.rows.iter().any(
            |row| row.kind == DependencyGraphDeltaKindV1::PackageUpgraded
                && row.package_name == "serde"
        ),
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

#[test]
fn dependency_graph_delta_compiler_classifies_within_major_requirement_movement() {
    // Same-major movement with equal floor precision compares the
    // numeric floors; losing floor precision broadens the accepted
    // range. Crossing a major boundary is a raise or a lowering even
    // when precision is unchanged.
    let identity = default_identity();
    let receipt = compile_dependency_graph_delta(
        &identity,
        "[dependencies]\nleft = \"1.2\"\nright = \"1.0.5\"\nrise = \"0.8\"\nsink = \"1.5\"\nflat = \"01.0\"\n",
        "[dependencies]\nleft = \"1.5\"\nright = \"1.0\"\nrise = \"1\"\nsink = \"1.2\"\nflat = \"1.0\"\n",
        "",
        "",
    )
    .expect("compilation succeeds");
    let kinds: Vec<_> = receipt
        .rows
        .iter()
        .map(|row| (row.package_name.as_str(), row.kind))
        .collect();
    assert!(
        kinds.contains(&("left", DependencyGraphDeltaKindV1::DirectRequirementRaised)),
        "the floor raise within one major is detected: {:?}",
        kinds
    );
    assert!(
        kinds.contains(&(
            "right",
            DependencyGraphDeltaKindV1::RequirementRangeBroadened
        )),
        "the precision loss is detected as a broadened range: {:?}",
        kinds
    );
    assert!(
        kinds.contains(&("rise", DependencyGraphDeltaKindV1::DirectRequirementRaised)),
        "the major-boundary raise is detected: {:?}",
        kinds
    );
    assert!(
        kinds.contains(&("sink", DependencyGraphDeltaKindV1::DirectRequirementLowered)),
        "the same-precision lowering is detected: {:?}",
        kinds
    );
    assert!(
        kinds.contains(&("flat", DependencyGraphDeltaKindV1::NoSemanticGraphChange)),
        "an equal-valued textual change moved no boundary: {:?}",
        kinds
    );
}

#[test]
fn dependency_graph_delta_compiler_fails_closed_on_unorderable_requirements() {
    // Comparator-prefixed requirements cannot be ordered from syntax
    // alone: the movement is visible but its polarity is an instrument
    // failure, never a guessed direction.
    let identity = default_identity();
    let receipt = compile_dependency_graph_delta(
        &identity,
        "[dependencies]\ncaret = \"^1.0\"\n",
        "[dependencies]\ncaret = \"^2.0\"\n",
        "",
        "",
    )
    .expect("compilation succeeds");
    let kinds: Vec<_> = receipt
        .rows
        .iter()
        .map(|row| (row.package_name.as_str(), row.kind))
        .collect();
    assert!(
        kinds.contains(&(
            "caret",
            DependencyGraphDeltaKindV1::UnsupportedOrInstrumentFailure
        )),
        "the unorderable movement fails closed: {:?}",
        kinds
    );
    assert!(
        !kinds
            .iter()
            .any(|(name, kind)| *name == "caret" && kind.is_semantic()),
        "no polarity is guessed for the unorderable movement: {:?}",
        kinds
    );
}

#[test]
fn dependency_graph_delta_compiler_receipts_direct_requirement_add_and_remove() {
    // A direct requirement that appears or disappears is its own row;
    // a head-only requirement means the package's stable resolution is
    // not described as lock-only movement.
    let identity = default_identity();
    let receipt = compile_dependency_graph_delta(
        &identity,
        "[dependencies]\nkeeper = \"1\"\nghost = \"0.8\"\n",
        "[dependencies]\nkeeper = \"1\"\narrival = \"1\"\n",
        "[[package]]\nname = \"keeper\"\nversion = \"1.0.0\"\n\n\
         [[package]]\nname = \"arrival\"\nversion = \"1.0.0\"\n\n\
         [[package]]\nname = \"ghost\"\nversion = \"0.8.0\"\n",
        "[[package]]\nname = \"keeper\"\nversion = \"1.0.0\"\n\n\
         [[package]]\nname = \"arrival\"\nversion = \"1.0.0\"\n",
    )
    .expect("compilation succeeds");
    let kinds: Vec<_> = receipt
        .rows
        .iter()
        .map(|row| (row.package_name.as_str(), row.kind))
        .collect();
    assert!(
        kinds.contains(&(
            "arrival",
            DependencyGraphDeltaKindV1::DirectRequirementAdded
        )),
        "the added requirement is detected: {:?}",
        kinds
    );
    assert!(
        kinds.contains(&(
            "ghost",
            DependencyGraphDeltaKindV1::DirectRequirementRemoved
        )),
        "the removed requirement is detected: {:?}",
        kinds
    );
    assert!(
        !kinds.contains(&(
            "arrival",
            DependencyGraphDeltaKindV1::LockOnlyResolutionChanged
        )),
        "a head-only requirement is not lock-only: {:?}",
        kinds
    );
}

#[test]
fn dependency_graph_delta_compiler_tolerates_malformed_inputs() {
    // Unparseable manifest or lock text contributes no rows; lock
    // entries without a usable name or version are skipped.
    let identity = default_identity();
    let receipt = compile_dependency_graph_delta(
        &identity,
        "[dependencies\n",
        "[dependencies]\nserde = \"1\"\n",
        "not a lockfile [[",
        "[[package]]\nversion = \"1.0\"\n\n[[package]]\nname = \"\"\nversion = \"1.0\"\n",
    )
    .expect("malformed inputs compile without crashing");
    assert!(
        receipt.rows.iter().any(|row| row.kind
            == DependencyGraphDeltaKindV1::DirectRequirementAdded
            && row.package_name == "serde"),
        "the requirement from the parseable side is detected: {:?}",
        receipt.rows
    );
}

#[test]
fn dependency_graph_delta_compiler_resolves_workspace_inherited_requirements() {
    // Workspace-inherited entries resolve their requirement from the
    // workspace root's [workspace.dependencies]; unresolvable entries
    // and string specs degrade to an empty requirement with no
    // features.
    let manifest = "[dependencies]\nserde = { workspace = true }\n";
    let workspace_doc: toml::Value =
        toml::from_str("[workspace]\n[workspace.dependencies]\nserde = \"1.0\"\n")
            .expect("workspace table parses");
    let workspace_table = workspace_doc
        .get("workspace")
        .expect("workspace table present");
    let requirements =
        crate::artifacts::parse_manifest_requirements(manifest, Some(workspace_table));
    let serde = requirements
        .iter()
        .find(|req| req.name == "serde")
        .expect("the serde requirement is found");
    assert_eq!(serde.requirement, "1.0");
    assert!(serde.features.is_empty());

    let table_doc: toml::Value = toml::from_str(
        "[workspace]\n[workspace.dependencies]\nserde = { version = \"1.0\", features = [\"derive\"] }\n",
    )
    .expect("table-form workspace parses");
    let table_requirements = crate::artifacts::parse_manifest_requirements(
        manifest,
        Some(table_doc.get("workspace").expect("workspace table present")),
    );
    let serde = table_requirements
        .iter()
        .find(|req| req.name == "serde")
        .expect("the table-form serde requirement is found");
    assert_eq!(serde.requirement, "1.0");
    assert_eq!(serde.features, vec!["derive".to_string()]);

    let unresolvable = crate::artifacts::parse_manifest_requirements(manifest, None);
    let serde = unresolvable
        .iter()
        .find(|req| req.name == "serde")
        .expect("the unresolvable serde requirement is still recorded");
    assert_eq!(serde.requirement, "");

    let string_spec =
        crate::artifacts::parse_manifest_requirements("[dependencies]\ntoml = \"1\"\n", None);
    let toml_req = string_spec
        .iter()
        .find(|req| req.name == "toml")
        .expect("the string-spec requirement is found");
    assert_eq!(toml_req.requirement, "1");
    assert!(toml_req.features.is_empty());
}

#[test]
fn dependency_graph_delta_parser_preserves_lock_identity_and_order() {
    // Rows are name-sorted and carry native Cargo identity fields;
    // path-only entries are retained with an empty source.
    let lock = "[[package]]\nname = \"zeta\"\nversion = \"1.0\"\nsource = \"registry\"\nchecksum = \"c\"\n\n\
                [[package]]\nname = \"local-path\"\nversion = \"0.5\"\n\n\
                [[package]]\nname = \"alpha\"\nversion = \"2.0\"\nsource = \"registry\"\nchecksum = \"a\"\n";
    let packages = crate::artifacts::parse_lock_packages(lock);
    let names: Vec<&str> = packages
        .iter()
        .map(|package| package.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["alpha", "local-path", "zeta"],
        "rows are name-sorted"
    );
    let path_entry = &packages[1];
    assert_eq!(path_entry.version, "0.5");
    assert_eq!(path_entry.source, "");
    let alpha = &packages[0];
    assert_eq!(alpha.version, "2.0");
    assert_eq!(alpha.source, "registry");
    assert_eq!(alpha.checksum, "a");
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

#[test]
fn dependency_graph_delta_compiler_resolves_workspace_in_merged_manifests() {
    // A synthesized merged member manifest carries its own
    // [workspace.dependencies]; inherited requirement movement is
    // visible without a separate workspace input.
    let identity = default_identity();
    let base =
        "[workspace.dependencies]\ntoml = \"1\"\n\n[dependencies]\ntoml = { workspace = true }\n";
    let head = "[workspace.dependencies]\ntoml = \"1\"\n\n[dependencies]\n";
    let receipt = compile_dependency_graph_delta(&identity, base, head, "", "")
        .expect("merged manifests compile");
    assert!(
        receipt.rows.iter().any(|row| row.kind
            == DependencyGraphDeltaKindV1::DirectRequirementRemoved
            && row.package_name == "toml"),
        "the inherited requirement removal is detected: {:?}",
        receipt.rows
    );
}
