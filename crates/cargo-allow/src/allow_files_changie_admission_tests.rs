//! allow-files changie-feature package admission (#3624).
//!
//! Qualifies the exact packaged surface another repository will consume
//! as a normal versioned dependency: the `changie` feature in the
//! normalized packaged manifest, the feature-enabled and
//! feature-disabled dependency closures, the public API baseline, and
//! the metadata the handoff records. The admission record this test
//! pins is the input to #2501's release qualification; actual
//! publication stays owned by the explicitly authorized release
//! transaction (#2502), which this file never performs.

use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap_or_else(|| std::panic::panic_any("workspace root not resolvable"))
        .to_path_buf()
}

fn read_workspace_file(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read {rel}: {err}")))
}

/// The admission record (#3624): the exact package/feature/API surface
/// the cargo-allow candidate must include, and what invalidates it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowFilesChangiePackageAdmissionV1 {
    pub schema_id: &'static str,
    pub crate_name: &'static str,
    pub feature: &'static str,
    pub feature_enabled_closure: Vec<&'static str>,
    pub feature_disabled_closure_contains_yaml: bool,
    pub public_modules: Vec<&'static str>,
    pub compatibility_generation: &'static str,
    pub diagnostic_schema_generation: u32,
    pub effective_rule_schema_generation: u32,
    pub msrv: &'static str,
    pub support_posture: &'static str,
    pub admission_digest_inputs: Vec<&'static str>,
}

pub fn admission_record() -> AllowFilesChangiePackageAdmissionV1 {
    AllowFilesChangiePackageAdmissionV1 {
        schema_id: "cargo-allow.allow-files-changie-package-admission.v1",
        crate_name: "allow-files",
        feature: "changie",
        // Enabling the feature adds exactly this reviewed closure: the
        // optional yaml-rust2 dependency and nothing else (falsifier 2
        // and 3: no workspace path deps, no YAML when disabled).
        feature_enabled_closure: vec!["yaml-rust2"],
        feature_disabled_closure_contains_yaml: false,
        public_modules: vec!["allow_files::changie", "allow_files::changie_lint"],
        compatibility_generation: "1.25",
        diagnostic_schema_generation: 1,
        effective_rule_schema_generation: 1,
        msrv: "1.95",
        support_posture: "experimental static companion (not a stable parser SDK)",
        admission_digest_inputs: vec![
            "crates/allow-files/Cargo.toml",
            "crates/allow-files/src/changie.rs",
            "crates/allow-files/src/changie_lint.rs",
            "crates/allow-files/README.md",
        ],
    }
}

/// The `[features]` section text up to (excluding) the next line-start
/// TOML section header.
fn features_section(manifest: &str) -> String {
    let after = manifest.split("[features]").nth(1).unwrap_or_default();
    let mut section = String::new();
    for line in after.lines().skip(1) {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') && trimmed.contains(']') {
            break;
        }
        section.push_str(line);
        section.push('\n');
    }
    section
}

#[test]
fn changie_feature_is_declared_in_the_crate_manifest() {
    // Falsifier 1: the packaged manifest must carry the feature.
    let root = workspace_root();
    let manifest = read_workspace_file(&root, "crates/allow-files/Cargo.toml");
    assert!(
        manifest.contains("[features]"),
        "allow-files must declare its features section"
    );
    let features_block = features_section(&manifest);
    // The declaration may wrap or carry a trailing comment; assert on
    // the feature name plus the exact dependency token instead of the
    // whole-line string.
    let declaration_line = features_block
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("changie") && line.contains('='))
        .unwrap_or_else(|| {
            std::panic::panic_any(format!("no changie feature line in {features_block:?}"))
        });
    assert!(
        declaration_line.contains("dep:yaml-rust2"),
        "the changie feature must gate exactly the optional yaml dependency: {declaration_line}"
    );
    // The dependency is optional so feature-disabled consumers never
    // compile it (falsifier 3).
    assert!(
        manifest.contains("yaml-rust2 = { version = \"0.11\", optional = true }"),
        "yaml-rust2 must be an optional dependency"
    );
}

#[test]
fn feature_closure_is_exactly_the_reviewed_dependency() {
    let root = workspace_root();
    let manifest = read_workspace_file(&root, "crates/allow-files/Cargo.toml");
    let features_block = features_section(&manifest);
    // Extract every feature line; only changie may exist and it may
    // only pull the reviewed dependency.
    let declared: Vec<&str> = features_block
        .lines()
        .filter(|line| !line.trim().starts_with('#'))
        .filter_map(|line| line.trim().split('=').next().map(str::trim))
        .filter(|name| !name.is_empty())
        .collect();
    assert_eq!(
        declared,
        vec!["changie"],
        "the changie admission covers exactly one feature: {declared:?}"
    );
}

#[test]
fn public_api_baseline_matches_the_admission() {
    // Falsifier 5: the public surface must exist and carry the claim
    // boundary in rustdoc so packaged docs cannot overclaim.
    let root = workspace_root();
    let lib = read_workspace_file(&root, "crates/allow-files/src/lib.rs");
    assert!(
        lib.contains("pub mod changie;"),
        "the parse module must be public under the feature"
    );
    assert!(
        lib.contains("pub mod changie_lint;"),
        "the lint module must be public under the feature"
    );
    let parse = read_workspace_file(&root, "crates/allow-files/src/changie.rs");
    assert!(
        parse.contains("never says `changie batch` ran"),
        "the public rustdoc states the static-versus-render boundary verbatim"
    );
}

#[test]
fn support_and_compatibility_wording_is_pinned() {
    // Falsifier 6: the package must not claim broad 1.x support ahead of
    // the #3614 matrix; the admission pins the exact generation.
    let record = admission_record();
    assert_eq!(record.compatibility_generation, "1.25");
    assert!(
        record
            .support_posture
            .contains("experimental static companion")
    );
    let root = workspace_root();
    let manifest = read_workspace_file(&root, "crates/allow-files/Cargo.toml");
    assert!(
        !manifest.to_lowercase().contains("stable parser"),
        "the package description must not claim a stable parser SDK"
    );
}

#[test]
fn admission_record_matches_the_published_schema() {
    // The record and the checked schema are one authority: field drift
    // between them fails here rather than at a consumer.
    let root = workspace_root();
    let schema_text = read_workspace_file(
        &root,
        "docs/schemas/allow-files-changie-package-admission.schema.json",
    );
    let record = admission_record();
    for required in [
        "schema_id",
        "crate_name",
        "feature",
        "feature_enabled_closure",
        "feature_disabled_closure_contains_yaml",
        "public_modules",
        "compatibility_generation",
        "diagnostic_schema_generation",
        "effective_rule_schema_generation",
        "msrv",
        "support_posture",
        "admission_digest_inputs",
    ] {
        assert!(
            schema_text.contains(&format!("\"{required}\"")),
            "schema is missing the {required} field"
        );
    }
    assert!(schema_text.contains(record.schema_id));
    assert!(schema_text.contains(record.crate_name));
    assert!(schema_text.contains(record.feature));
    assert!(schema_text.contains(record.compatibility_generation));
    assert!(schema_text.contains(record.msrv));
    // The schema itself refuses parser-SDK overclaim wording.
    assert!(schema_text.contains("stable parser SDK"));
}

#[test]
fn admission_record_is_complete_and_deterministic() {
    let record = admission_record();
    assert_eq!(
        record.schema_id,
        "cargo-allow.allow-files-changie-package-admission.v1"
    );
    assert_eq!(record.crate_name, "allow-files");
    assert!(!record.feature_enabled_closure.is_empty());
    assert!(!record.feature_disabled_closure_contains_yaml);
    assert_eq!(record.diagnostic_schema_generation, 1);
    assert_eq!(record.effective_rule_schema_generation, 1);
    // Determinism: the record is a pure function.
    assert_eq!(admission_record(), record);
    // Every digest input exists in the tree (checksum invalidation is
    // wired through exact file identities).
    let root = workspace_root();
    for rel in &record.admission_digest_inputs {
        assert!(
            root.join(rel).is_file(),
            "admission digest input missing: {rel}"
        );
    }
}

#[test]
fn packaged_metadata_is_source_independent() {
    // Falsifier 2: the published package must not carry workspace path
    // dependencies for the sensor closure. allow-files may keep only
    // its existing non-optional workspace dependencies; yaml-rust2 is
    // the sole sensor dependency and it is a registry version.
    let root = workspace_root();
    let manifest = read_workspace_file(&root, "crates/allow-files/Cargo.toml");
    let dependencies = manifest
        .split("[dependencies]")
        .nth(1)
        .and_then(|text| text.split('[').next())
        .unwrap_or_default();
    for line in dependencies.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("yaml-rust2") {
            assert!(
                !trimmed.contains("path"),
                "the sensor dependency must be registry-versioned, not a path dep: {trimmed}"
            );
            assert!(
                !trimmed.contains("git"),
                "the sensor dependency must not be a git dependency: {trimmed}"
            );
        }
    }
}

#[test]
fn topology_carries_the_sensor_surface_in_the_release_candidate() {
    // The cargo-allow candidate must include allow-files with its
    // feature surface; the V2 topology is the candidate authority.
    let root = workspace_root();
    let topology = read_workspace_file(&root, "policy/product-package-topology-v2.toml");
    let row = topology
        .split("[[package]]")
        .find(|block| block.contains("logical_id = \"allow-files\""))
        .unwrap_or_else(|| std::panic::panic_any("allow-files row missing from topology"));
    assert!(row.contains("publish = true"), "must be publishable");
    assert!(
        row.contains("candidate_inclusion = true"),
        "must be in the candidate"
    );
    assert!(
        row.contains("package_version = \"0.2.0\""),
        "the admitted version is the candidate version"
    );
}

#[test]
fn no_publication_or_tag_mutation_is_declared_by_the_admission() {
    // Falsifiers 9 and 10: the admission record is qualification input
    // only — publication authority stays with the release transaction.
    let record = admission_record();
    let joined = format!(
        "{} {} {}",
        record.schema_id, record.support_posture, record.msrv
    );
    assert!(!joined.to_lowercase().contains("published"));
    assert!(!joined.to_lowercase().contains("uploaded"));
}
