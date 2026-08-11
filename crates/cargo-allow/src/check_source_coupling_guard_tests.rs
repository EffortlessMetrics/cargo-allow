use super::{
    source_coupling_diagnostics_at, source_coupling_diagnostics_for_check,
    source_coupling_diagnostics_for_sources, source_coupling_fails_check,
};
use allow_match::CheckMode;
use allow_policy::product_crates::parse_architecture_manifest_v2;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

fn fixture_manifest() -> Result<allow_policy::product_crates::ArchitectureManifestV2, String> {
    parse_architecture_manifest_v2(
        r#"
schema_version = "2.0"
authority_generation = 2
manifest_id = "fixture"
controlling_issue = 3443
linked_move_ledger = "fixture-ledger"

[[crate_identity]]
logical_id = "product-a"
workspace_path = "crates/product-a"
workspace_dependency_aliases = ["product-a"]
cargo_package_name = "product-a"
rust_library_name = "product_a"
product_or_shared_owner = "product-a"
crate_role = "CargoAllowCore"

[[crate_identity]]
logical_id = "product-b"
workspace_path = "crates/product-b"
workspace_dependency_aliases = ["product-b"]
cargo_package_name = "product-b"
rust_library_name = "product_b"
product_or_shared_owner = "product-b"
crate_role = "CargoAllowCore"

[[crate_identity]]
logical_id = "shared-protocol"
workspace_path = "crates/shared-protocol"
workspace_dependency_aliases = ["shared-protocol"]
cargo_package_name = "shared-protocol"
rust_library_name = "shared_protocol"
product_or_shared_owner = "shared"
crate_role = "SharedProtocol"
"#,
    )
    .map_err(|error| format!("fixture manifest: {error}"))
}

#[test]
fn rejects_only_known_cross_product_imports() -> Result<(), String> {
    let manifest = fixture_manifest()?;
    let sources = vec![(
        PathBuf::from("crates/product-a/src/lib.rs"),
        "use product_b::private_api;\nuse product_a::own_api;\nuse shared_protocol::Wire;\nuse external::Thing;\nuse crate::local;\n".to_string(),
    )];
    let forbidden = BTreeMap::from([(
        "product-a".to_string(),
        BTreeSet::from(["product-b".to_string()]),
    )]);
    let diagnostics = source_coupling_diagnostics_for_sources(&manifest, &forbidden, &sources)
        .map_err(|error| format!("scan coupling fixture: {error}"))?;
    if diagnostics.len() != 1 {
        return Err(format!(
            "expected one cross-product diagnostic, got {diagnostics:?}"
        ));
    }
    let diagnostic = diagnostics
        .first()
        .ok_or_else(|| "missing cross-product diagnostic".to_string())?;
    if diagnostic.target_crate != "product-b"
        || diagnostic.source_owner != "product-a"
        || diagnostic.target_owner != "product-b"
        || diagnostic.line != 1
    {
        return Err(format!("unexpected diagnostic: {diagnostic:?}"));
    }
    Ok(())
}

#[test]
fn audit_mode_remains_advisory() -> Result<(), String> {
    if source_coupling_fails_check(PathBuf::from(".").as_path(), CheckMode::Audit)
        .map_err(|error| format!("audit guard: {error}"))?
    {
        return Err("audit mode unexpectedly enforced source coupling".to_string());
    }
    Ok(())
}

#[test]
fn strict_mode_checks_architecture_repositories() -> Result<(), String> {
    let diagnostics =
        source_coupling_diagnostics_for_check(PathBuf::from(".").as_path(), CheckMode::Strict)
            .map_err(|error| format!("strict guard: {error}"))?;
    if !diagnostics.is_empty() {
        return Err(format!(
            "unexpected tracked source coupling: {diagnostics:?}"
        ));
    }
    Ok(())
}

#[test]
fn consumer_trees_without_architecture_manifests_are_outside_scope() -> Result<(), String> {
    let root = PathBuf::from("target/cargo-allow/source-coupling-consumer-without-policy");
    let diagnostics = source_coupling_diagnostics_at(&root)
        .map_err(|error| format!("consumer tree guard: {error}"))?;
    if !diagnostics.is_empty() {
        return Err(format!(
            "unexpected consumer-tree diagnostics: {diagnostics:?}"
        ));
    }
    Ok(())
}

#[test]
fn tracked_worktree_source_coupling_is_clean() -> Result<(), String> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let diagnostics = super::source_coupling_diagnostics_at(&root)
        .map_err(|error| format!("scan tracked source coupling: {error}"))?;
    if !diagnostics.is_empty() {
        return Err(format!("tracked worktree source coupling: {diagnostics:?}"));
    }
    Ok(())
}
