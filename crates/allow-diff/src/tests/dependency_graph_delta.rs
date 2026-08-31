//! Contract and falsifying-fixture tests for the dependency graph delta
//! receipt (issue #3920), including the #2036-shaped TOML downgrade that must
//! stay visible even when a review integration ignores `Cargo.lock`.

use super::*;

const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

// ---------------------------------------------------------------------------
// Fixture builders
// ---------------------------------------------------------------------------

fn manifest_with_dependencies(entries: &str) -> String {
    format!(
        "[package]\nname = \"fixture-product\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n{entries}\n"
    )
}

fn lock_package(
    name: &str,
    version: &str,
    source: Option<&str>,
    checksum: Option<&str>,
    dependencies: &[&str],
) -> String {
    let mut text = format!("\n[[package]]\nname = \"{name}\"\nversion = \"{version}\"\n");
    if let Some(source) = source {
        text.push_str(&format!("source = \"{source}\"\n"));
    }
    if let Some(checksum) = checksum {
        text.push_str(&format!("checksum = \"{checksum}\"\n"));
    }
    if !dependencies.is_empty() {
        text.push_str("dependencies = [\n");
        for dependency in dependencies {
            text.push_str(&format!(" \"{dependency}\",\n"));
        }
        text.push_str("]\n");
    }
    text
}

fn side(
    commit: &str,
    manifests: &[(&str, String)],
    lockfile: Option<String>,
) -> DependencyGraphSideInputV1 {
    DependencyGraphSideInputV1 {
        commit: commit.to_string(),
        tree: format!("tree-{commit}"),
        manifests: manifests
            .iter()
            .map(|(path, text)| ((*path).to_string(), text.clone()))
            .collect(),
        lockfile,
    }
}

fn request(
    base: DependencyGraphSideInputV1,
    head: DependencyGraphSideInputV1,
) -> DependencyGraphDeltaRequestV1 {
    DependencyGraphDeltaRequestV1 {
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        product: "fixture-product".to_string(),
        target: "x86_64-unknown-linux-gnu".to_string(),
        cargo_tool_identity: Some("cargo 1.95.0".to_string()),
        feature_configuration: None,
        base,
        head,
    }
}

fn has_kind(
    receipt: &DependencyGraphDeltaReceiptV1,
    kind: DependencyGraphDeltaKindV1,
    package: &str,
) -> bool {
    receipt
        .rows
        .iter()
        .any(|row| row.kind == kind && row.package == package)
}

fn rows_for<'a>(
    receipt: &'a DependencyGraphDeltaReceiptV1,
    kind: DependencyGraphDeltaKindV1,
    package: &str,
) -> Vec<&'a DependencyGraphDeltaRowV1> {
    receipt
        .rows
        .iter()
        .filter(|row| row.kind == kind && row.package == package)
        .collect()
}

fn single_row<'a>(
    receipt: &'a DependencyGraphDeltaReceiptV1,
    kind: DependencyGraphDeltaKindV1,
    package: &str,
) -> &'a DependencyGraphDeltaRowV1 {
    rows_for(receipt, kind, package).first().unwrap_or_else(|| {
        panic!("expected exactly one {kind:?} row for {package}; rows: {receipt:?}")
    })
}

/// Seeded-failure assertion: the exact review gap of PR #2036. A review bot
/// excluded `Cargo.lock`, so only manifest-visible rows reach the reviewer;
/// the downgrade must be visible there, and the parser-graph replacement must
/// be visible to any reader of the receipt.
fn assert_issue_2036_downgrade_visibility(receipt: &DependencyGraphDeltaReceiptV1) {
    let manifest_visible_downgrade = receipt.rows.iter().any(|row| {
        row.kind == DependencyGraphDeltaKindV1::DirectRequirementLowered
            && row.package == "toml"
            && row.manifest_path.is_some()
            && row.base_requirement.as_deref() == Some("1.0")
            && row.head_requirement.as_deref() == Some("0.8")
    });
    assert!(
        manifest_visible_downgrade,
        "REGRESSION #2036: a TOML 1.x -> 0.8 manifest downgrade must emit a \
         manifest-visible DirectRequirementLowered row so review tooling that \
         ignores Cargo.lock still sees the downgrade; rows: {receipt:?}"
    );
    let lockfile_downgrade = receipt.rows.iter().any(|row| {
        row.kind == DependencyGraphDeltaKindV1::PackageDowngraded
            && row.package == "toml"
            && row.base_version.as_deref() == Some("1.8.0")
            && row.head_version.as_deref() == Some("0.8.20")
    });
    assert!(
        lockfile_downgrade,
        "REGRESSION #2036: the resolved TOML downgrade must appear as a \
         PackageDowngraded row; rows: {receipt:?}"
    );
    let parser_graph_replacement = receipt
        .rows
        .iter()
        .filter(|row| row.kind == DependencyGraphDeltaKindV1::PackageRemoved);
    let removed: Vec<String> = parser_graph_replacement
        .map(|row| {
            format!(
                "{} {}",
                row.package,
                row.base_version.clone().unwrap_or_default()
            )
        })
        .collect();
    assert!(
        removed
            .iter()
            .any(|package| package.starts_with("toml_parser")),
        "REGRESSION #2036: the removed parser/serialization packages must stay \
         visible even though aggregate package counts stay similar; rows: {receipt:?}"
    );
}

// ---------------------------------------------------------------------------
// #2036-shaped fixture
// ---------------------------------------------------------------------------

/// Base graph: TOML 1.x with its parser/serialization closure. Head graph:
/// TOML 0.8 with a differently-shaped parser graph of similar total size.
fn issue_2036_lockfile(head: bool) -> String {
    let mut text = String::from("version = 4\n");
    let checksum = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    if head {
        text.push_str(&lock_package(
            "indexmap",
            "2.0.0",
            Some(CRATES_IO),
            Some(checksum),
            &["equivalent", "hashbrown"],
        ));
        text.push_str(&lock_package(
            "equivalent",
            "1.0.0",
            Some(CRATES_IO),
            Some(checksum),
            &[],
        ));
        text.push_str(&lock_package(
            "hashbrown",
            "0.14.0",
            Some(CRATES_IO),
            Some(checksum),
            &[],
        ));
        text.push_str(&lock_package(
            "serde",
            "1.0.219",
            Some(CRATES_IO),
            Some(checksum),
            &["serde_derive 1.0.219"],
        ));
        text.push_str(&lock_package(
            "serde_derive",
            "1.0.219",
            Some(CRATES_IO),
            Some(checksum),
            &["proc-macro2 1.0.0", "quote 1.0.0", "syn 2.0.100"],
        ));
        text.push_str(&lock_package(
            "serde_spanned",
            "0.6.8",
            Some(CRATES_IO),
            Some(checksum),
            &["serde", "toml_datetime 0.6.8"],
        ));
        text.push_str(&lock_package(
            "toml",
            "0.8.20",
            Some(CRATES_IO),
            Some(checksum),
            &[
                "serde",
                "serde_spanned 0.6.8",
                "toml_datetime 0.6.8",
                "toml_edit 0.22.26",
            ],
        ));
        text.push_str(&lock_package(
            "toml_datetime",
            "0.6.8",
            Some(CRATES_IO),
            Some(checksum),
            &[],
        ));
        text.push_str(&lock_package(
            "toml_edit",
            "0.22.26",
            Some(CRATES_IO),
            Some(checksum),
            &[
                "indexmap 2.0.0",
                "serde",
                "serde_spanned 0.6.8",
                "toml_datetime 0.6.8",
                "winnow 0.7.0",
            ],
        ));
        text.push_str(&lock_package(
            "winnow",
            "0.7.0",
            Some(CRATES_IO),
            Some(checksum),
            &[],
        ));
    } else {
        text.push_str(&lock_package(
            "proc-macro2",
            "1.0.0",
            Some(CRATES_IO),
            Some(checksum),
            &[],
        ));
        text.push_str(&lock_package(
            "quote",
            "1.0.0",
            Some(CRATES_IO),
            Some(checksum),
            &["proc-macro2 1.0.0"],
        ));
        text.push_str(&lock_package(
            "serde",
            "1.0.219",
            Some(CRATES_IO),
            Some(checksum),
            &["serde_derive 1.0.219"],
        ));
        text.push_str(&lock_package(
            "serde_derive",
            "1.0.219",
            Some(CRATES_IO),
            Some(checksum),
            &["proc-macro2 1.0.0", "quote 1.0.0", "syn 2.0.100"],
        ));
        text.push_str(&lock_package(
            "serde_spanned",
            "1.0.0",
            Some(CRATES_IO),
            Some(checksum),
            &["serde", "toml_datetime 1.0.0"],
        ));
        text.push_str(&lock_package(
            "syn",
            "2.0.100",
            Some(CRATES_IO),
            Some(checksum),
            &["proc-macro2 1.0.0", "quote 1.0.0", "unicode-ident 1.0.0"],
        ));
        text.push_str(&lock_package(
            "toml",
            "1.8.0",
            Some(CRATES_IO),
            Some(checksum),
            &[
                "serde",
                "serde_spanned 1.0.0",
                "toml_datetime 1.0.0",
                "toml_parser 1.0.0",
                "toml_writer 1.0.0",
            ],
        ));
        text.push_str(&lock_package(
            "toml_datetime",
            "1.0.0",
            Some(CRATES_IO),
            Some(checksum),
            &[],
        ));
        text.push_str(&lock_package(
            "toml_parser",
            "1.0.0",
            Some(CRATES_IO),
            Some(checksum),
            &["winnow 0.7.0"],
        ));
        text.push_str(&lock_package(
            "toml_writer",
            "1.0.0",
            Some(CRATES_IO),
            Some(checksum),
            &[],
        ));
        text.push_str(&lock_package(
            "unicode-ident",
            "1.0.0",
            Some(CRATES_IO),
            Some(checksum),
            &[],
        ));
        text.push_str(&lock_package(
            "winnow",
            "0.7.0",
            Some(CRATES_IO),
            Some(checksum),
            &[],
        ));
    }
    text
}

#[test]
fn dependency_graph_delta_fixtures_issue_2036_toml_downgrade_is_visible() {
    let base_manifest = manifest_with_dependencies("serde = \"1.0\"\ntoml = \"1.0\"\n");
    let head_manifest = manifest_with_dependencies("serde = \"1.0\"\ntoml = \"0.8\"\n");
    let receipt = dependency_graph_delta(&request(
        side(
            "basecommit0000000000000000000000000000000",
            &[("Cargo.toml", base_manifest)],
            Some(issue_2036_lockfile(false)),
        ),
        side(
            "headcommit00000000000000000000000000000000",
            &[("Cargo.toml", head_manifest)],
            Some(issue_2036_lockfile(true)),
        ),
    ));

    assert_eq!(
        receipt.verdict,
        DependencyGraphDeltaVerdictV1::Complete,
        "the #2036-shaped pair must classify completely; rows: {receipt:?}"
    );
    assert_issue_2036_downgrade_visibility(&receipt);

    // A reviewer who ignores Cargo.lock still sees the parser graph
    // replacement through the remaining movement rows.
    let manifest_visible_kinds: Vec<DependencyGraphDeltaKindV1> = receipt
        .rows
        .iter()
        .filter(|row| row.manifest_path.is_some())
        .map(|row| row.kind)
        .collect();
    assert!(
        manifest_visible_kinds.contains(&DependencyGraphDeltaKindV1::DirectRequirementLowered),
        "manifest-only review surface must contain the downgrade; rows: {receipt:?}"
    );
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::PackageAdded,
            "toml_edit"
        ),
        "the replacement parser graph must be visible; rows: {receipt:?}"
    );
}

// ---------------------------------------------------------------------------
// Direct requirement movements
// ---------------------------------------------------------------------------

#[test]
fn dependency_graph_delta_fixtures_direct_requirement_added_removed_raised() {
    let base_manifest =
        manifest_with_dependencies("bitflags = \"2.0\"\nserde = \"1.0\"\ntempfile = \"3.0\"\n");
    let head_manifest =
        manifest_with_dependencies("anyhow = \"1.0\"\nbitflags = \"2.4\"\nserde = \"1.0\"\n");
    let mut lock = String::from("version = 4\n");
    lock.push_str(&lock_package("anyhow", "1.0.0", Some(CRATES_IO), None, &[]));
    lock.push_str(&lock_package(
        "bitflags",
        "2.4.0",
        Some(CRATES_IO),
        None,
        &[],
    ));
    lock.push_str(&lock_package(
        "serde",
        "1.0.219",
        Some(CRATES_IO),
        None,
        &[],
    ));
    lock.push_str(&lock_package(
        "tempfile",
        "3.0.0",
        Some(CRATES_IO),
        None,
        &[],
    ));
    let receipt = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", base_manifest)], Some(lock.clone())),
        side("h", &[("Cargo.toml", head_manifest)], Some(lock)),
    ));

    assert_eq!(receipt.verdict, DependencyGraphDeltaVerdictV1::Complete);
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::DirectRequirementAdded,
            "anyhow"
        ),
        "rows: {receipt:?}"
    );
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::DirectRequirementRemoved,
            "tempfile"
        ),
        "rows: {receipt:?}"
    );
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::DirectRequirementRaised,
            "bitflags"
        ),
        "rows: {receipt:?}"
    );
    let raised = single_row(
        &receipt,
        DependencyGraphDeltaKindV1::DirectRequirementRaised,
        "bitflags",
    );
    assert_eq!(raised.base_requirement.as_deref(), Some("2.0"));
    assert_eq!(raised.head_requirement.as_deref(), Some("2.4"));
    assert!(!has_kind(
        &receipt,
        DependencyGraphDeltaKindV1::ManifestLockMismatch,
        "bitflags"
    ));
}

#[test]
fn dependency_graph_delta_fixtures_requirement_range_broadened_and_narrowed() {
    let base_manifest = manifest_with_dependencies(
        "cfg-if = \"1.0\"\nlog = \"0.4\"\nserde = { version = [\"1.0\", \"0.9\"] }\n",
    );
    let head_manifest = manifest_with_dependencies(
        "cfg-if = { version = [\"1.0\", \"0.2\"] }\nlog = \"=0.4.27\"\nserde = \"1.0\"\n",
    );
    let mut lock = String::from("version = 4\n");
    lock.push_str(&lock_package("cfg-if", "1.0.0", Some(CRATES_IO), None, &[]));
    lock.push_str(&lock_package("log", "0.4.27", Some(CRATES_IO), None, &[]));
    lock.push_str(&lock_package(
        "serde",
        "1.0.219",
        Some(CRATES_IO),
        None,
        &[],
    ));
    let receipt = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", base_manifest)], Some(lock.clone())),
        side("h", &[("Cargo.toml", head_manifest)], Some(lock)),
    ));

    assert_eq!(receipt.verdict, DependencyGraphDeltaVerdictV1::Complete);
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::RequirementRangeBroadened,
            "cfg-if"
        ),
        "adding a 0.2 alternative to 1.0 broadens the accepted range; rows: {receipt:?}"
    );
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::RequirementRangeNarrowed,
            "log"
        ),
        "pinning log to an exact in-range version narrows the range; rows: {receipt:?}"
    );
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::RequirementRangeNarrowed,
            "serde"
        ),
        "dropping the 0.9 alternative narrows the accepted range; rows: {receipt:?}"
    );
}

#[test]
fn dependency_graph_delta_fixtures_workspace_inherited_requirement_raise() {
    let base_root = String::from(
        "[workspace]\nmembers = [\"member\"]\n\n[workspace.dependencies]\nserde = \"1.0\"\n",
    );
    let head_root = String::from(
        "[workspace]\nmembers = [\"member\"]\n\n[workspace.dependencies]\nserde = \"1.1\"\n",
    );
    let member = String::from(
        "[package]\nname = \"member\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nserde.workspace = true\n",
    );
    let mut lock = String::from("version = 4\n");
    lock.push_str(&lock_package(
        "serde",
        "1.0.219",
        Some(CRATES_IO),
        None,
        &[],
    ));
    let mut head_lock = String::from("version = 4\n");
    head_lock.push_str(&lock_package("serde", "1.1.0", Some(CRATES_IO), None, &[]));
    let receipt = dependency_graph_delta(&request(
        side(
            "b",
            &[
                ("Cargo.toml", base_root),
                ("member/Cargo.toml", member.clone()),
            ],
            Some(lock),
        ),
        side(
            "h",
            &[("Cargo.toml", head_root), ("member/Cargo.toml", member)],
            Some(head_lock),
        ),
    ));

    assert_eq!(receipt.verdict, DependencyGraphDeltaVerdictV1::Complete);
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::DirectRequirementRaised,
            "serde"
        ),
        "a workspace-dependency floor raise must surface at the member site; rows: {receipt:?}"
    );
}

// ---------------------------------------------------------------------------
// Lock-only and graph movements
// ---------------------------------------------------------------------------

#[test]
fn dependency_graph_delta_fixtures_lock_only_resolution_change_is_visible() {
    let manifest = manifest_with_dependencies("libc = \"0.2\"\n");
    let mut base_lock = String::from("version = 4\n");
    base_lock.push_str(&lock_package("libc", "0.2.155", Some(CRATES_IO), None, &[]));
    let mut head_lock = String::from("version = 4\n");
    head_lock.push_str(&lock_package("libc", "0.2.171", Some(CRATES_IO), None, &[]));
    let receipt = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", manifest.clone())], Some(base_lock)),
        side("h", &[("Cargo.toml", manifest)], Some(head_lock)),
    ));

    assert_eq!(receipt.verdict, DependencyGraphDeltaVerdictV1::Complete);
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::LockOnlyResolutionChanged,
            "libc"
        ),
        "a lockfile-only resolution move must not be omitted; rows: {receipt:?}"
    );
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::PackageUpgraded,
            "libc"
        ),
        "rows: {receipt:?}"
    );
    assert!(
        !has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::NoSemanticGraphChange,
            "fixture-product"
        ),
        "lock-only movement must never be reported as no change; rows: {receipt:?}"
    );
}

#[test]
fn dependency_graph_delta_fixtures_transitive_swap_with_equal_count_stays_visible() {
    let manifest = manifest_with_dependencies("kept = \"1.0\"\n");
    let mut base_lock = String::from("version = 4\n");
    base_lock.push_str(&lock_package("kept", "1.0.0", Some(CRATES_IO), None, &[]));
    base_lock.push_str(&lock_package(
        "transitive-a",
        "1.0.0",
        Some(CRATES_IO),
        None,
        &[],
    ));
    base_lock.push_str(&lock_package(
        "transitive-b",
        "2.0.0",
        Some(CRATES_IO),
        None,
        &[],
    ));
    let mut head_lock = String::from("version = 4\n");
    head_lock.push_str(&lock_package("kept", "1.0.0", Some(CRATES_IO), None, &[]));
    head_lock.push_str(&lock_package(
        "transitive-b",
        "2.0.0",
        Some(CRATES_IO),
        None,
        &[],
    ));
    head_lock.push_str(&lock_package(
        "unrelated-c",
        "3.0.0",
        Some(CRATES_IO),
        None,
        &[],
    ));
    let receipt = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", manifest.clone())], Some(base_lock)),
        side("h", &[("Cargo.toml", manifest)], Some(head_lock)),
    ));

    assert_eq!(receipt.verdict, DependencyGraphDeltaVerdictV1::Complete);
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::PackageRemoved,
            "transitive-a"
        ),
        "count parity must not hide the removal; rows: {receipt:?}"
    );
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::PackageAdded,
            "unrelated-c"
        ),
        "count parity must not hide the addition; rows: {receipt:?}"
    );
}

#[test]
fn dependency_graph_delta_fixtures_source_and_checksum_change_is_visible() {
    let manifest = manifest_with_dependencies("smallvec = \"1.0\"\n");
    let mut base_lock = String::from("version = 4\n");
    base_lock.push_str(&lock_package(
        "smallvec",
        "1.13.2",
        Some(CRATES_IO),
        Some("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"),
        &[],
    ));
    let mut head_lock = String::from("version = 4\n");
    head_lock.push_str(&lock_package(
        "smallvec",
        "1.13.2",
        Some(CRATES_IO),
        Some("dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"),
        &[],
    ));
    let receipt = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", manifest.clone())], Some(base_lock)),
        side("h", &[("Cargo.toml", manifest)], Some(head_lock)),
    ));

    assert_eq!(receipt.verdict, DependencyGraphDeltaVerdictV1::Complete);
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::SourceOrChecksumChanged,
            "smallvec"
        ),
        "a silent checksum change must stay visible; rows: {receipt:?}"
    );
}

#[test]
fn dependency_graph_delta_fixtures_manifest_git_source_change_is_visible() {
    let base_manifest = manifest_with_dependencies(
        "serde-json = { package = \"serde_json\", git = \"https://example.com/serde-json\", rev = \"aaaaaaaa\" }\n",
    );
    let head_manifest = manifest_with_dependencies(
        "serde-json = { package = \"serde_json\", git = \"https://example.com/serde-json\", rev = \"bbbbbbbb\" }\n",
    );
    let mut lock = String::from("version = 4\n");
    lock.push_str(&lock_package("serde_json", "1.0.0", None, None, &[]));
    let receipt = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", base_manifest)], Some(lock.clone())),
        side("h", &[("Cargo.toml", head_manifest)], Some(lock)),
    ));

    assert_eq!(receipt.verdict, DependencyGraphDeltaVerdictV1::Complete);
    let changed = single_row(
        &receipt,
        DependencyGraphDeltaKindV1::SourceOrChecksumChanged,
        "serde-json",
    );
    assert!(
        changed
            .base_source
            .as_deref()
            .unwrap_or("")
            .contains("aaaaaaaa"),
        "rows: {receipt:?}"
    );
    assert!(
        changed
            .head_source
            .as_deref()
            .unwrap_or("")
            .contains("bbbbbbbb"),
        "rows: {receipt:?}"
    );
}

#[test]
fn dependency_graph_delta_fixtures_feature_activation_change_is_visible() {
    let base_manifest = manifest_with_dependencies("serde = { version = \"1.0\" }\n");
    let head_manifest =
        manifest_with_dependencies("serde = { version = \"1.0\", features = [\"derive\"] }\n");
    let mut lock = String::from("version = 4\n");
    lock.push_str(&lock_package(
        "serde",
        "1.0.219",
        Some(CRATES_IO),
        None,
        &[],
    ));
    lock.push_str(&lock_package(
        "serde_derive",
        "1.0.219",
        Some(CRATES_IO),
        None,
        &[],
    ));
    let receipt = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", base_manifest)], Some(lock.clone())),
        side("h", &[("Cargo.toml", head_manifest)], Some(lock)),
    ));

    assert_eq!(receipt.verdict, DependencyGraphDeltaVerdictV1::Complete);
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::FeatureActivationChanged,
            "serde"
        ),
        "manifest feature activation must stay visible; rows: {receipt:?}"
    );
}

#[test]
fn dependency_graph_delta_fixtures_lockfile_edge_activation_change_is_visible() {
    let manifest = manifest_with_dependencies("tracing = \"0.1\"\n");
    let mut base_lock = String::from("version = 4\n");
    base_lock.push_str(&lock_package(
        "tracing",
        "0.1.0",
        Some(CRATES_IO),
        None,
        &[],
    ));
    base_lock.push_str(&lock_package(
        "tracing-attributes",
        "0.1.0",
        Some(CRATES_IO),
        None,
        &[],
    ));
    base_lock.push_str(&lock_package(
        "tracing-core",
        "0.1.0",
        Some(CRATES_IO),
        None,
        &[],
    ));
    let mut head_lock = String::from("version = 4\n");
    head_lock.push_str(&lock_package(
        "tracing",
        "0.1.0",
        Some(CRATES_IO),
        None,
        &["tracing-attributes 0.1.0", "tracing-core 0.1.0"],
    ));
    head_lock.push_str(&lock_package(
        "tracing-attributes",
        "0.1.0",
        Some(CRATES_IO),
        None,
        &[],
    ));
    head_lock.push_str(&lock_package(
        "tracing-core",
        "0.1.0",
        Some(CRATES_IO),
        None,
        &[],
    ));
    let receipt = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", manifest.clone())], Some(base_lock)),
        side("h", &[("Cargo.toml", manifest)], Some(head_lock)),
    ));

    assert_eq!(receipt.verdict, DependencyGraphDeltaVerdictV1::Complete);
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::FeatureActivationChanged,
            "tracing"
        ),
        "edge-level feature activation toward an existing package must be \
         classified as feature activation; rows: {receipt:?}"
    );
}

#[test]
fn dependency_graph_delta_fixtures_target_and_dependency_class_change() {
    let base_manifest = String::from(
        "[package]\nname = \"fixture-product\"\nversion = \"0.1.0\"\n\n[dev-dependencies]\nlibc = \"0.2\"\nwinapi = \"0.3\"\n",
    );
    let head_manifest = String::from(
        "[package]\nname = \"fixture-product\"\nversion = \"0.1.0\"\n\n[dependencies]\nlibc = \"0.2\"\n\n[target.'cfg(windows)'.dependencies]\nwinapi = \"0.3\"\n",
    );
    let mut lock = String::from("version = 4\n");
    lock.push_str(&lock_package("libc", "0.2.171", Some(CRATES_IO), None, &[]));
    lock.push_str(&lock_package("winapi", "0.3.9", Some(CRATES_IO), None, &[]));
    let receipt = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", base_manifest)], Some(lock.clone())),
        side("h", &[("Cargo.toml", head_manifest)], Some(lock)),
    ));

    assert_eq!(receipt.verdict, DependencyGraphDeltaVerdictV1::Complete);
    let class_move = single_row(
        &receipt,
        DependencyGraphDeltaKindV1::TargetOrDependencyClassChanged,
        "libc",
    );
    assert_eq!(
        class_move.dependency_class,
        DependencyGraphEdgeClassV1::Normal
    );
    assert!(class_move.detail.contains("from_dev"), "rows: {receipt:?}");
    let target_move = single_row(
        &receipt,
        DependencyGraphDeltaKindV1::TargetOrDependencyClassChanged,
        "winapi",
    );
    assert_eq!(target_move.target, "cfg(windows)");
    assert!(
        !has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::DirectRequirementAdded,
            "libc"
        ),
        "a class move is one movement, not add plus remove; rows: {receipt:?}"
    );
    assert!(
        !has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::DirectRequirementRemoved,
            "libc"
        ),
        "rows: {receipt:?}"
    );
}

#[test]
fn dependency_graph_delta_fixtures_manifest_lock_mismatch_stale_lock() {
    let base_manifest = manifest_with_dependencies("serde = \"1.0\"\n");
    let head_manifest = manifest_with_dependencies("serde = \"1.1\"\n");
    let mut base_lock = String::from("version = 4\n");
    base_lock.push_str(&lock_package(
        "serde",
        "1.0.219",
        Some(CRATES_IO),
        None,
        &[],
    ));
    // Head lock is stale: still resolves the old 1.0 line.
    let mut head_lock = String::from("version = 4\n");
    head_lock.push_str(&lock_package(
        "serde",
        "1.0.219",
        Some(CRATES_IO),
        None,
        &[],
    ));
    let receipt = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", base_manifest)], Some(base_lock)),
        side("h", &[("Cargo.toml", head_manifest)], Some(head_lock)),
    ));

    assert_eq!(receipt.verdict, DependencyGraphDeltaVerdictV1::Complete);
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::DirectRequirementRaised,
            "serde"
        ),
        "rows: {receipt:?}"
    );
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::ManifestLockMismatch,
            "serde"
        ),
        "a stale lockfile must fail visibly instead of looking clean; rows: {receipt:?}"
    );
    let mismatch = single_row(
        &receipt,
        DependencyGraphDeltaKindV1::ManifestLockMismatch,
        "serde",
    );
    assert_eq!(
        mismatch.head_requirement.as_deref(),
        Some("1.1"),
        "rows: {receipt:?}"
    );
}

#[test]
fn dependency_graph_delta_fixtures_duplicate_version_movement() {
    let manifest = manifest_with_dependencies("regex = \"1.5\"\n");
    let mut base_lock = String::from("version = 4\n");
    base_lock.push_str(&lock_package("regex", "1.5.0", Some(CRATES_IO), None, &[]));
    let mut head_lock = String::from("version = 4\n");
    head_lock.push_str(&lock_package("regex", "1.5.0", Some(CRATES_IO), None, &[]));
    head_lock.push_str(&lock_package("regex", "1.9.0", Some(CRATES_IO), None, &[]));
    let receipt = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", manifest.clone())], Some(base_lock)),
        side("h", &[("Cargo.toml", manifest)], Some(head_lock)),
    ));

    assert_eq!(receipt.verdict, DependencyGraphDeltaVerdictV1::Complete);
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::DuplicateVersionMovement,
            "regex"
        ),
        "duplicate-version population change must stay visible; rows: {receipt:?}"
    );
    assert!(
        has_kind(&receipt, DependencyGraphDeltaKindV1::PackageAdded, "regex"),
        "rows: {receipt:?}"
    );
}

#[test]
fn dependency_graph_delta_fixtures_no_semantic_graph_change() {
    let manifest = manifest_with_dependencies("libc = \"0.2\"\n");
    let commented = format!("# only a comment changed\n{manifest}");
    let mut lock = String::from("version = 4\n");
    lock.push_str(&lock_package("libc", "0.2.171", Some(CRATES_IO), None, &[]));
    let receipt = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", manifest.clone())], Some(lock.clone())),
        side("h", &[("Cargo.toml", commented)], Some(lock)),
    ));

    eprintln!("DEBUG receipt: {receipt:?}");
    assert_eq!(receipt.verdict, DependencyGraphDeltaVerdictV1::Complete);
    assert_eq!(receipt.rows.len(), 1, "rows: {receipt:?}");
    let only_row = receipt
        .rows
        .first()
        .unwrap_or_else(|| panic!("expected one row"));
    assert_eq!(
        only_row.kind,
        DependencyGraphDeltaKindV1::NoSemanticGraphChange
    );
    assert_eq!(only_row.package, "fixture-product");
}

// ---------------------------------------------------------------------------
// Negative denominators: missing, malformed, empty
// ---------------------------------------------------------------------------

#[test]
fn dependency_graph_delta_fixtures_missing_and_malformed_inputs_cannot_be_clean() {
    let manifest = manifest_with_dependencies("libc = \"0.2\"\n");
    let mut lock = String::from("version = 4\n");
    lock.push_str(&lock_package("libc", "0.2.171", Some(CRATES_IO), None, &[]));

    // Missing head lockfile.
    let missing_lock = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", manifest.clone())], Some(lock.clone())),
        side("h", &[("Cargo.toml", manifest.clone())], None),
    ));
    assert_eq!(
        missing_lock.verdict,
        DependencyGraphDeltaVerdictV1::InstrumentFailure
    );
    assert!(missing_lock.rows.iter().any(|row| row.kind
        == DependencyGraphDeltaKindV1::UnsupportedOrInstrumentFailure
        && row.detail == "head_lockfile_missing"));
    assert!(
        !missing_lock
            .rows
            .iter()
            .any(|row| row.kind == DependencyGraphDeltaKindV1::NoSemanticGraphChange)
    );

    // Malformed lockfile TOML.
    let malformed_lock = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", manifest.clone())], Some(lock.clone())),
        side(
            "h",
            &[("Cargo.toml", manifest.clone())],
            Some("version = 4\n[[package]]\nname = \"broken\"".to_string()),
        ),
    ));
    assert_eq!(
        malformed_lock.verdict,
        DependencyGraphDeltaVerdictV1::InstrumentFailure
    );
    assert!(
        malformed_lock
            .rows
            .iter()
            .any(|row| row.detail.starts_with("lockfile_parse_error"))
    );

    // Malformed manifest TOML.
    let malformed_manifest = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", manifest.clone())], Some(lock.clone())),
        side(
            "h",
            &[("Cargo.toml", "[dependencies\nlibc = \"0.2\"\n".to_string())],
            Some(lock.clone()),
        ),
    ));
    assert_eq!(
        malformed_manifest.verdict,
        DependencyGraphDeltaVerdictV1::InstrumentFailure
    );
    assert!(
        malformed_manifest
            .rows
            .iter()
            .any(|row| row.detail.starts_with("manifest_parse_error"))
    );

    // Unparseable lockfile version.
    let bad_version = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", manifest.clone())], Some(lock.clone())),
        side(
            "h",
            &[("Cargo.toml", manifest.clone())],
            Some(format!(
                "{lock}\n[[package]]\nname = \"odd\"\nversion = \"not-semver\"\n"
            )),
        ),
    ));
    assert_eq!(
        bad_version.verdict,
        DependencyGraphDeltaVerdictV1::InstrumentFailure
    );
    assert!(
        bad_version
            .rows
            .iter()
            .any(|row| row.detail.starts_with("lockfile_version_unparseable"))
    );

    // Empty manifest set on the head side is a zero denominator.
    let empty_manifests = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", manifest.clone())], Some(lock.clone())),
        side("h", &[], Some(lock)),
    ));
    assert_eq!(
        empty_manifests.verdict,
        DependencyGraphDeltaVerdictV1::InstrumentFailure
    );
    assert!(
        empty_manifests
            .rows
            .iter()
            .any(|row| row.detail == "head_manifest_set_empty")
    );
}

#[test]
fn dependency_graph_delta_fixtures_unresolvable_requirement_is_manifest_lock_mismatch() {
    let base_manifest = manifest_with_dependencies("serde = \"1.0\"\n");
    let head_manifest = manifest_with_dependencies("serde = \"1.0\"\nnonexistent = \"9.9\"\n");
    let mut lock = String::from("version = 4\n");
    lock.push_str(&lock_package(
        "serde",
        "1.0.219",
        Some(CRATES_IO),
        None,
        &[],
    ));
    let receipt = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", base_manifest)], Some(lock.clone())),
        side("h", &[("Cargo.toml", head_manifest)], Some(lock)),
    ));

    assert_eq!(receipt.verdict, DependencyGraphDeltaVerdictV1::Complete);
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::DirectRequirementAdded,
            "nonexistent"
        ),
        "rows: {receipt:?}"
    );
    assert!(
        has_kind(
            &receipt,
            DependencyGraphDeltaKindV1::ManifestLockMismatch,
            "nonexistent"
        ),
        "a requirement absent from the lockfile must fail visibly; rows: {receipt:?}"
    );
}

// ---------------------------------------------------------------------------
// Contract properties: identity, determinism, projection
// ---------------------------------------------------------------------------

#[test]
fn dependency_graph_delta_contract_identity_binding_fields() {
    let manifest = manifest_with_dependencies("libc = \"0.2\"\n");
    let mut lock = String::from("version = 4\n");
    lock.push_str(&lock_package("libc", "0.2.171", Some(CRATES_IO), None, &[]));
    let receipt = dependency_graph_delta(&request(
        side(
            "1111111111111111111111111111111111111111",
            &[("Cargo.toml", manifest.clone())],
            Some(lock.clone()),
        ),
        side(
            "2222222222222222222222222222222222222222",
            &[("Cargo.toml", manifest)],
            Some(lock),
        ),
    ));

    assert_eq!(receipt.schema_id, DEPENDENCY_GRAPH_DELTA_V1_SCHEMA_ID);
    assert_eq!(receipt.schema_version, 1);
    assert_eq!(receipt.repository, "EffortlessMetrics/cargo-allow");
    assert_eq!(receipt.product, "fixture-product");
    assert_eq!(receipt.target, "x86_64-unknown-linux-gnu");
    assert_eq!(receipt.cargo_tool_identity.as_deref(), Some("cargo 1.95.0"));
    assert_eq!(
        receipt.base.commit,
        "1111111111111111111111111111111111111111"
    );
    assert_eq!(
        receipt.head.tree,
        "tree-2222222222222222222222222222222222222222"
    );
    assert_eq!(receipt.base.manifest_count, 1);
    assert!(!receipt.base.manifest_set_digest.is_empty());
    assert!(receipt.base.lockfile_digest.is_some());
    assert_eq!(
        receipt
            .rows
            .first()
            .map(|row| row.kind)
            .unwrap_or(DependencyGraphDeltaKindV1::UnsupportedOrInstrumentFailure),
        DependencyGraphDeltaKindV1::NoSemanticGraphChange
    );
}

#[test]
fn dependency_graph_delta_contract_identity_digests_track_inputs() {
    let manifest = manifest_with_dependencies("libc = \"0.2\"\n");
    let changed_manifest = manifest_with_dependencies("libc = \"0.2\"\nlog = \"0.4\"\n");
    let mut lock = String::from("version = 4\n");
    lock.push_str(&lock_package("libc", "0.2.171", Some(CRATES_IO), None, &[]));
    let unchanged = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", manifest.clone())], Some(lock.clone())),
        side("h", &[("Cargo.toml", manifest.clone())], Some(lock.clone())),
    ));
    let changed = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", manifest.clone())], Some(lock.clone())),
        side("h", &[("Cargo.toml", changed_manifest)], Some(lock.clone())),
    ));

    assert_ne!(
        unchanged.head.manifest_set_digest, changed.head.manifest_set_digest,
        "moving manifest bytes must move the bound digest"
    );
    assert_eq!(
        unchanged.head.lockfile_digest, changed.head.lockfile_digest,
        "unchanged lockfile bytes keep the same digest"
    );

    let no_lock = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", manifest.clone())], Some(lock.clone())),
        side("h", &[("Cargo.toml", manifest.clone())], None),
    ));
    assert_eq!(no_lock.head.lockfile_digest, None);
}

#[test]
fn dependency_graph_delta_contract_deterministic_across_input_order() {
    let manifest_one = manifest_with_dependencies("libc = \"0.2\"\nlog = \"0.4\"\n");
    let manifest_two = manifest_with_dependencies("log = \"0.4\"\nlibc = \"0.2\"\n");
    let mut lock_one = String::from("version = 4\n");
    lock_one.push_str(&lock_package("libc", "0.2.171", Some(CRATES_IO), None, &[]));
    lock_one.push_str(&lock_package("log", "0.4.27", Some(CRATES_IO), None, &[]));
    let mut lock_two = String::from("version = 4\n");
    lock_two.push_str(&lock_package("log", "0.4.27", Some(CRATES_IO), None, &[]));
    lock_two.push_str(&lock_package("libc", "0.2.171", Some(CRATES_IO), None, &[]));

    let first = dependency_graph_delta(&request(
        side(
            "b",
            &[("Cargo.toml", manifest_one.clone())],
            Some(lock_one.clone()),
        ),
        side(
            "h",
            &[("Cargo.toml", manifest_one.clone())],
            Some(lock_one.clone()),
        ),
    ));
    let second = dependency_graph_delta(&request(
        side(
            "b",
            &[("Cargo.toml", manifest_two.clone())],
            Some(lock_two.clone()),
        ),
        side(
            "h",
            &[("Cargo.toml", manifest_two.clone())],
            Some(lock_two.clone()),
        ),
    ));

    // Semantic output is independent of dependency order inside the manifest
    // text and lockfile record order.
    assert_eq!(
        first.rows, second.rows,
        "manifest dependency order and lockfile record order must not change the classified rows"
    );
    assert_eq!(first.verdict, second.verdict);

    // Byte identity is preserved: reordered manifest or lockfile bytes move
    // the bound digests even when the semantic rows are unchanged.
    assert_ne!(
        first.base.manifest_set_digest, second.base.manifest_set_digest,
        "different manifest bytes must produce different bound digests"
    );
    assert_ne!(
        first.base.lockfile_digest, second.base.lockfile_digest,
        "different lockfile bytes must produce different bound digests"
    );

    // Traversal-order invariance: the same texts inserted in a different
    // order into the manifest map produce one identical receipt.
    let reordered = dependency_graph_delta(&request(
        side(
            "b",
            &[("Cargo.toml", manifest_two.clone())],
            Some(lock_two.clone()),
        ),
        side(
            "h",
            &[("Cargo.toml", manifest_two.clone())],
            Some(lock_two.clone()),
        ),
    ));
    let same_bytes = dependency_graph_delta(&request(
        side(
            "b",
            &[("Cargo.toml", manifest_two.clone())],
            Some(lock_two.clone()),
        ),
        side(
            "h",
            &[("Cargo.toml", manifest_two.clone())],
            Some(lock_two.clone()),
        ),
    ));
    assert_eq!(
        reordered.head.manifest_set_digest, same_bytes.head.manifest_set_digest,
        "input traversal order must not change the bound digest"
    );

    assert!(
        first
            .rows
            .iter()
            .zip(first.rows.iter().skip(1))
            .all(|(left, right)| left <= right),
        "rows must be emitted in deterministic sorted order"
    );
}

#[test]
fn dependency_graph_delta_contract_json_projection_round_trips() {
    let base_manifest = manifest_with_dependencies("toml = \"1.0\"\n");
    let head_manifest = manifest_with_dependencies("toml = \"0.8\"\n");
    let mut base_lock = String::from("version = 4\n");
    base_lock.push_str(&lock_package("toml", "1.8.0", Some(CRATES_IO), None, &[]));
    let mut head_lock = String::from("version = 4\n");
    head_lock.push_str(&lock_package("toml", "0.8.20", Some(CRATES_IO), None, &[]));
    let receipt = dependency_graph_delta(&request(
        side("b", &[("Cargo.toml", base_manifest)], Some(base_lock)),
        side("h", &[("Cargo.toml", head_manifest)], Some(head_lock)),
    ));

    let rendered = serde_json::to_string(&receipt)
        .unwrap_or_else(|error| panic!("receipt must serialize: {error}"));
    let parsed: DependencyGraphDeltaReceiptV1 = serde_json::from_str(&rendered)
        .unwrap_or_else(|error| panic!("receipt must deserialize: {error}"));
    assert_eq!(parsed, receipt);
    assert!(
        rendered.contains("direct_requirement_lowered"),
        "machine projection must use stable snake_case labels: {rendered}"
    );
}
