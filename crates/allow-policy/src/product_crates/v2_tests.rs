//! Tests for the V2 architecture identity authority (#2921).

use super::config::{CrateRole, parse_architecture_manifest};
use super::v1_reader::read_v1_as_historical;
use super::v2_validate::{
    IdentityDiagnosticKind, validate_v2_alias_map, validate_v2_identity_uniqueness,
};
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

fn parse_ok(input: &str) -> Result<ArchitectureManifestV2, String> {
    parse_architecture_manifest_v2(input).map_err(|err| err.to_string())
}

#[test]
fn v2_parser_reads_identity_fields() -> Result<(), String> {
    let manifest = parse_ok(V2_MINIMAL)?;
    if manifest.schema_version != ARCHITECTURE_MANIFEST_V2_SCHEMA_VERSION {
        return Err("schema_version mismatch".to_string());
    }
    if manifest.authority_generation != ARCHITECTURE_MANIFEST_V2_AUTHORITY_GENERATION {
        return Err("authority_generation mismatch".to_string());
    }
    if manifest.crate_identity.len() != 1 {
        return Err("expected 1 crate_identity".to_string());
    }
    let entry = manifest.crate_identity.first().ok_or("expected crate_identity entry")?;
    if entry.logical_id != "repo-protocol" {
        return Err(format!("unexpected logical_id: {}", entry.logical_id));
    }
    if entry.cargo_package_name != "effortless-repo-protocol" {
        return Err(format!(
            "unexpected cargo_package_name: {}",
            entry.cargo_package_name
        ));
    }
    if entry.rust_library_name != "repo_protocol" {
        return Err(format!(
            "unexpected rust_library_name: {}",
            entry.rust_library_name
        ));
    }
    if entry.crate_role != CrateRole::SharedProtocol {
        return Err("unexpected crate_role".to_string());
    }
    Ok(())
}

#[test]
fn v2_parser_rejects_missing_schema_version() -> Result<(), String> {
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
    if parse_ok(input).is_ok() {
        return Err("missing schema_version should fail".to_string());
    }
    Ok(())
}

#[test]
fn v2_parser_rejects_v1_schema_version() -> Result<(), String> {
    let input = V2_MINIMAL.replace("\"2.0\"", "\"1.0\"");
    match parse_ok(&input) {
        Ok(_) => Err("schema_version 1.0 should fail".to_string()),
        Err(msg) if msg.contains("expected `2.0`") => Ok(()),
        Err(msg) => Err(format!("unexpected error: {msg}")),
    }
}

#[test]
fn v2_parser_rejects_missing_authority_generation() -> Result<(), String> {
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
    if parse_ok(input).is_ok() {
        return Err("missing authority_generation should fail".to_string());
    }
    Ok(())
}

#[test]
fn v2_parser_rejects_wrong_authority_generation() -> Result<(), String> {
    let input = V2_MINIMAL.replace("authority_generation = 2", "authority_generation = 1");
    if parse_ok(&input).is_ok() {
        return Err("authority_generation 1 should fail".to_string());
    }
    Ok(())
}

#[test]
fn v2_parser_rejects_unknown_field() -> Result<(), String> {
    let input = format!("{}\nunknown_field = true\n", V2_MINIMAL.trim_end());
    if parse_ok(&input).is_ok() {
        return Err("unknown field should fail".to_string());
    }
    Ok(())
}

#[test]
fn v2_logical_id_differs_from_package_and_library() -> Result<(), String> {
    let manifest = parse_ok(V2_MINIMAL)?;
    let entry = manifest
        .crate_identity
        .first()
        .ok_or("expected crate_identity entry")?;
    if entry.logical_id == entry.cargo_package_name {
        return Err("logical_id should differ from cargo_package_name".to_string());
    }
    if entry.logical_id == entry.rust_library_name {
        return Err("logical_id should differ from rust_library_name".to_string());
    }
    if entry.cargo_package_name == entry.rust_library_name {
        return Err("cargo_package_name should differ from rust_library_name".to_string());
    }
    Ok(())
}

#[test]
fn v2_uniqueness_passes_for_distinct_entries() -> Result<(), String> {
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
    let manifest = parse_ok(input)?;
    let diags = validate_v2_identity_uniqueness(&manifest);
    if !diags.is_empty() {
        return Err(format!(
            "distinct entries should have no uniqueness errors: {diags:?}"
        ));
    }
    Ok(())
}

#[test]
fn v2_uniqueness_detects_duplicate_logical_id() -> Result<(), String> {
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
    let manifest = parse_ok(input)?;
    let diags = validate_v2_identity_uniqueness(&manifest);
    if !diags
        .iter()
        .any(|d| d.kind == IdentityDiagnosticKind::DuplicateLogicalId)
    {
        return Err("should detect duplicate logical_id".to_string());
    }
    Ok(())
}

#[test]
fn v2_uniqueness_detects_duplicate_workspace_path() -> Result<(), String> {
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
    let manifest = parse_ok(input)?;
    let diags = validate_v2_identity_uniqueness(&manifest);
    if !diags
        .iter()
        .any(|d| d.kind == IdentityDiagnosticKind::DuplicateWorkspacePath)
    {
        return Err("should detect duplicate workspace_path".to_string());
    }
    Ok(())
}

#[test]
fn v2_alias_map_detects_ambiguous_alias() -> Result<(), String> {
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
    let manifest = parse_ok(input)?;
    let diags = validate_v2_alias_map(&manifest);
    if !diags
        .iter()
        .any(|d| d.kind == IdentityDiagnosticKind::AmbiguousAlias)
    {
        return Err("should detect ambiguous alias".to_string());
    }
    Ok(())
}

#[test]
fn v2_alias_map_accepts_deliberate_enumerated_aliases() -> Result<(), String> {
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
    let manifest = parse_ok(input)?;
    let diags = validate_v2_alias_map(&manifest);
    if !diags.is_empty() {
        return Err(format!(
            "multiple aliases to one package should be fine: {diags:?}"
        ));
    }
    Ok(())
}

#[test]
fn v1_historical_reader_projects_to_v2() -> Result<(), String> {
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
    let v1 = parse_architecture_manifest(v1_input).map_err(|e| e.to_string())?;
    let projection = read_v1_as_historical(&v1).map_err(|e| e.to_string())?;
    if projection.manifest.schema_version != ARCHITECTURE_MANIFEST_V2_SCHEMA_VERSION {
        return Err("projected schema_version mismatch".to_string());
    }
    if projection.manifest.authority_generation != ARCHITECTURE_MANIFEST_V2_AUTHORITY_GENERATION {
        return Err("projected authority_generation mismatch".to_string());
    }
    // 2 product crates + 1 shared = 3 identities
    if projection.manifest.crate_identity.len() != 3 {
        return Err(format!(
            "expected 3 crate identities, got {}",
            projection.manifest.crate_identity.len()
        ));
    }
    // Every entry should produce a migration diagnostic
    if projection.diagnostics.len() != 3 {
        return Err(format!(
            "expected 3 migration diagnostics, got {}",
            projection.diagnostics.len()
        ));
    }
    // Library name derived by dash-to-underscore
    let allow_core = projection
        .manifest
        .crate_identity
        .iter()
        .find(|e| e.logical_id == "allow-core")
        .ok_or("missing allow-core identity")?;
    if allow_core.rust_library_name != "allow_core" {
        return Err(format!(
            "expected allow_core library name, got {}",
            allow_core.rust_library_name
        ));
    }
    Ok(())
}

#[test]
fn v1_historical_reader_cannot_produce_current_result() -> Result<(), String> {
    let v1_input = r#"
schema_version = "1.0"
manifest_id = "TEST"
controlling_issue = 2580
linked_move_ledger = "TEST"

[[product]]
id = "cargo-allow"
owned_crates = ["a"]
"#;
    let v1 = parse_architecture_manifest(v1_input).map_err(|e| e.to_string())?;
    let projection = read_v1_as_historical(&v1).map_err(|e| e.to_string())?;
    if projection.diagnostics.is_empty() {
        return Err("V1 historical reader must always emit migration diagnostics".to_string());
    }
    Ok(())
}

#[test]
fn v2_manifest_type_supports_full_workspace_without_rename() -> Result<(), String> {
    let input = r#"
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

[[crate_identity]]
logical_id = "proof-engine"
workspace_path = "crates/proof-engine"
cargo_package_name = "proof-engine"
rust_library_name = "proof_engine"
product_or_shared_owner = "cargo-proof"
crate_role = "CargoProof"
"#;
    let manifest: ArchitectureManifestV2 = parse_ok(input)?;
    if manifest.crate_identity.len() != 2 {
        return Err(format!(
            "expected 2 identities, got {}",
            manifest.crate_identity.len()
        ));
    }
    let diags = validate_v2_identity_uniqueness(&manifest);
    if !diags.is_empty() {
        return Err(format!("single entries should validate: {diags:?}"));
    }
    Ok(())
}
