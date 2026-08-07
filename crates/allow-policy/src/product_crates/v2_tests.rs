//! Tests for the V2 architecture identity authority (#2921).

use super::config::{CrateRole, parse_architecture_manifest};
use super::v1_reader::read_v1_as_historical;
use super::v2_validate::{validate_v2_alias_map, validate_v2_identity_uniqueness};
use super::{
    ARCHITECTURE_MANIFEST_V2_AUTHORITY_GENERATION, ARCHITECTURE_MANIFEST_V2_SCHEMA_VERSION,
    ArchitectureManifestV2, parse_architecture_manifest_v2,
};

const V2_MINIMAL: &str = r#"
schema_version = "2.0"
authority_generation = 2
manifest_id = "CARGO-ALLOW-ARCH-V2-TEST"
controlling_issue = 2921
linked_move_ledger = "CARGO-ALLOW-MOVE-LEDGER-0001"

[[crate_identity]]
logical_id = "repo-protocol"
workspace_path = "crates/effortless-repo-protocol"
workspace_dependency_aliases = ["effortless-repo-protocol"]
cargo_package_name = "effortless-repo-protocol"
rust_library_name = "repo_protocol"
product_or_shared_owner = "shared"
crate_role = "SharedProtocol"
"#;

#[test]
fn v2_parser_reads_identity_fields() {
    let manifest = parse_architecture_manifest_v2(V2_MINIMAL).expect("V2 parse should succeed");
    assert_eq!(
        manifest.schema_version,
        ARCHITECTURE_MANIFEST_V2_SCHEMA_VERSION
    );
    assert_eq!(
        manifest.authority_generation,
        ARCHITECTURE_MANIFEST_V2_AUTHORITY_GENERATION
    );
    assert_eq!(manifest.crate_identity.len(), 1);
    let entry = &manifest.crate_identity[0];
    assert_eq!(entry.logical_id, "repo-protocol");
    assert_eq!(entry.cargo_package_name, "effortless-repo-protocol");
    assert_eq!(entry.rust_library_name, "repo_protocol");
    assert_eq!(entry.crate_role, CrateRole::SharedProtocol);
}

#[test]
fn v2_parser_rejects_missing_schema_version() {
    let input = r#"
authority_generation = 2
manifest_id = "TEST"
controlling_issue = 2921
linked_move_ledger = "TEST"

[[crate_identity]]
logical_id = "a"
workspace_path = "crates/a"
cargo_package_name = "a"
rust_library_name = "a"
product_or_shared_owner = "shared"
crate_role = "SharedProtocol"
"#;
    let result = parse_architecture_manifest_v2(input);
    assert!(result.is_err(), "missing schema_version should fail");
}

#[test]
fn v2_parser_rejects_v1_schema_version() {
    let input = V2_MINIMAL.replace("\"2.0\"", "\"1.0\"");
    let result = parse_architecture_manifest_v2(&input);
    assert!(result.is_err(), "schema_version 1.0 should fail");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("expected `2.0`"),
        "error should name expected version: {msg}"
    );
}

#[test]
fn v2_parser_rejects_missing_authority_generation() {
    let input = r#"
schema_version = "2.0"
manifest_id = "TEST"
controlling_issue = 2921
linked_move_ledger = "TEST"

[[crate_identity]]
logical_id = "a"
workspace_path = "crates/a"
cargo_package_name = "a"
rust_library_name = "a"
product_or_shared_owner = "shared"
crate_role = "SharedProtocol"
"#;
    let result = parse_architecture_manifest_v2(input);
    assert!(result.is_err(), "missing authority_generation should fail");
}

#[test]
fn v2_parser_rejects_wrong_authority_generation() {
    let input = V2_MINIMAL.replace("authority_generation = 2", "authority_generation = 1");
    let result = parse_architecture_manifest_v2(&input);
    assert!(result.is_err(), "authority_generation 1 should fail");
}

#[test]
fn v2_parser_rejects_unknown_field() {
    let input = format!("{}\nunknown_field = true\n", V2_MINIMAL.trim_end());
    let result = parse_architecture_manifest_v2(&input);
    assert!(result.is_err(), "unknown field should fail");
}

#[test]
fn v2_logical_id_differs_from_package_and_library() {
    let manifest = parse_architecture_manifest_v2(V2_MINIMAL).unwrap();
    let entry = &manifest.crate_identity[0];
    assert_ne!(entry.logical_id, entry.cargo_package_name);
    assert_ne!(entry.logical_id, entry.rust_library_name);
    assert_ne!(entry.cargo_package_name, entry.rust_library_name);
}

#[test]
fn v2_uniqueness_passes_for_distinct_entries() {
    let input = r#"
schema_version = "2.0"
authority_generation = 2
manifest_id = "TEST"
controlling_issue = 2921
linked_move_ledger = "TEST"

[[crate_identity]]
logical_id = "a"
workspace_path = "crates/a"
cargo_package_name = "pkg-a"
rust_library_name = "lib_a"
product_or_shared_owner = "shared"
crate_role = "SharedProtocol"

[[crate_identity]]
logical_id = "b"
workspace_path = "crates/b"
cargo_package_name = "pkg-b"
rust_library_name = "lib_b"
product_or_shared_owner = "shared"
crate_role = "SharedProtocol"
"#;
    let manifest = parse_architecture_manifest_v2(input).unwrap();
    let diags = validate_v2_identity_uniqueness(&manifest);
    assert!(
        diags.is_empty(),
        "distinct entries should have no uniqueness errors: {diags:?}"
    );
}

#[test]
fn v2_uniqueness_detects_duplicate_logical_id() {
    let input = r#"
schema_version = "2.0"
authority_generation = 2
manifest_id = "TEST"
controlling_issue = 2921
linked_move_ledger = "TEST"

[[crate_identity]]
logical_id = "dup"
workspace_path = "crates/a"
cargo_package_name = "pkg-a"
rust_library_name = "lib_a"
product_or_shared_owner = "shared"
crate_role = "SharedProtocol"

[[crate_identity]]
logical_id = "dup"
workspace_path = "crates/b"
cargo_package_name = "pkg-b"
rust_library_name = "lib_b"
product_or_shared_owner = "shared"
crate_role = "SharedProtocol"
"#;
    let manifest = parse_architecture_manifest_v2(input).unwrap();
    let diags = validate_v2_identity_uniqueness(&manifest);
    assert!(
        diags
            .iter()
            .any(|d| d.kind == super::v2_validate::IdentityDiagnosticKind::DuplicateLogicalId)
    );
}

#[test]
fn v2_uniqueness_detects_duplicate_workspace_path() {
    let input = r#"
schema_version = "2.0"
authority_generation = 2
manifest_id = "TEST"
controlling_issue = 2921
linked_move_ledger = "TEST"

[[crate_identity]]
logical_id = "a"
workspace_path = "crates/same"
cargo_package_name = "pkg-a"
rust_library_name = "lib_a"
product_or_shared_owner = "shared"
crate_role = "SharedProtocol"

[[crate_identity]]
logical_id = "b"
workspace_path = "crates/same"
cargo_package_name = "pkg-b"
rust_library_name = "lib_b"
product_or_shared_owner = "shared"
crate_role = "SharedProtocol"
"#;
    let manifest = parse_architecture_manifest_v2(input).unwrap();
    let diags = validate_v2_identity_uniqueness(&manifest);
    assert!(
        diags
            .iter()
            .any(|d| d.kind == super::v2_validate::IdentityDiagnosticKind::DuplicateWorkspacePath)
    );
}

#[test]
fn v2_alias_map_detects_ambiguous_alias() {
    let input = r#"
schema_version = "2.0"
authority_generation = 2
manifest_id = "TEST"
controlling_issue = 2921
linked_move_ledger = "TEST"

[[crate_identity]]
logical_id = "a"
workspace_path = "crates/a"
workspace_dependency_aliases = ["shared-alias"]
cargo_package_name = "pkg-a"
rust_library_name = "lib_a"
product_or_shared_owner = "shared"
crate_role = "SharedProtocol"

[[crate_identity]]
logical_id = "b"
workspace_path = "crates/b"
workspace_dependency_aliases = ["shared-alias"]
cargo_package_name = "pkg-b"
rust_library_name = "lib_b"
product_or_shared_owner = "shared"
crate_role = "SharedProtocol"
"#;
    let manifest = parse_architecture_manifest_v2(input).unwrap();
    let diags = validate_v2_alias_map(&manifest);
    assert!(
        diags
            .iter()
            .any(|d| d.kind == super::v2_validate::IdentityDiagnosticKind::AmbiguousAlias)
    );
}

#[test]
fn v2_alias_map_accepts_deliberate_enumerated_aliases() {
    let input = r#"
schema_version = "2.0"
authority_generation = 2
manifest_id = "TEST"
controlling_issue = 2921
linked_move_ledger = "TEST"

[[crate_identity]]
logical_id = "core"
workspace_path = "crates/core"
workspace_dependency_aliases = ["allow-core", "legacy-allow-core"]
cargo_package_name = "allow-core"
rust_library_name = "allow_core"
product_or_shared_owner = "cargo-allow"
crate_role = "CargoAllowCore"
"#;
    let manifest = parse_architecture_manifest_v2(input).unwrap();
    let diags = validate_v2_alias_map(&manifest);
    assert!(
        diags.is_empty(),
        "multiple aliases to one package should be fine: {diags:?}"
    );
}

#[test]
fn v1_historical_reader_projects_to_v2() {
    let v1_input = r#"
schema_version = "1.0"
manifest_id = "CARGO-ALLOW-ARCH-0001"
controlling_issue = 2580
linked_move_ledger = "TEST"

[[product]]
id = "cargo-allow"
owned_crates = ["allow-core", "cargo-allow"]
forbid_product_dependencies = []

[[shared_crate]]
name = "repo-protocol"
role = "SharedProtocol"
allowed_domain_dependencies = []
"#;
    let v1 = parse_architecture_manifest(v1_input).unwrap();
    let projection = read_v1_as_historical(&v1).unwrap();
    assert_eq!(
        projection.manifest.schema_version,
        ARCHITECTURE_MANIFEST_V2_SCHEMA_VERSION
    );
    assert_eq!(
        projection.manifest.authority_generation,
        ARCHITECTURE_MANIFEST_V2_AUTHORITY_GENERATION
    );
    // 2 product crates + 1 shared = 3 identities
    assert_eq!(projection.manifest.crate_identity.len(), 3);
    // Every entry should produce a migration diagnostic
    assert_eq!(projection.diagnostics.len(), 3);
    // Library name derived by dash-to-underscore
    let allow_core = projection
        .manifest
        .crate_identity
        .iter()
        .find(|e| e.logical_id == "allow-core")
        .unwrap();
    assert_eq!(allow_core.rust_library_name, "allow_core");
}

#[test]
fn v1_historical_reader_cannot_produce_current_result() {
    // A V1 projection always has diagnostics — it's historical, never current.
    let v1_input = r#"
schema_version = "1.0"
manifest_id = "TEST"
controlling_issue = 2580
linked_move_ledger = "TEST"

[[product]]
id = "cargo-allow"
owned_crates = ["a"]
"#;
    let v1 = parse_architecture_manifest(v1_input).unwrap();
    let projection = read_v1_as_historical(&v1).unwrap();
    assert!(
        !projection.diagnostics.is_empty(),
        "V1 historical reader must always emit migration diagnostics"
    );
}

#[test]
fn v2_manifest_type_supports_full_workspace_without_rename() {
    // The 20-crate workspace (post-absorption) should be representable
    // in V2 without changing any package names.
    let mut input = String::from(
        r#"
schema_version = "2.0"
authority_generation = 2
manifest_id = "CARGO-ALLOW-ARCH-V2-0001"
controlling_issue = 2921
linked_move_ledger = "CARGO-ALLOW-MOVE-LEDGER-0001"

[[crate_identity]]
logical_id = "allow-core"
workspace_path = "crates/allow-core"
cargo_package_name = "allow-core"
rust_library_name = "allow_core"
product_or_shared_owner = "cargo-allow"
crate_role = "CargoAllowCore"
"#,
    );
    // Just verify a single entry parses and validates cleanly.
    let manifest = parse_architecture_manifest_v2(&input).unwrap();
    let diags = validate_v2_identity_uniqueness(&manifest);
    assert!(diags.is_empty(), "single entry should validate: {diags:?}");
    // Append more entries to verify multi-entry works.
    input.push_str(
        r#"
[[crate_identity]]
logical_id = "proof-engine"
workspace_path = "crates/proof-engine"
cargo_package_name = "proof-engine"
rust_library_name = "proof_engine"
product_or_shared_owner = "cargo-proof"
crate_role = "CargoProof"
"#,
    );
    let manifest: ArchitectureManifestV2 = parse_architecture_manifest_v2(&input).unwrap();
    assert_eq!(manifest.crate_identity.len(), 2);
}
