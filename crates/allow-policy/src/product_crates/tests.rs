use super::config::parse_architecture_manifest;
use super::dependency_graph::parse_cargo_metadata_graph;
use super::validate::{
    ArchitectureDiagnosticKind, validate_architecture_manifest, validate_dependency_law,
    workspace_members_from_manifest,
};
use std::path::PathBuf;

const REPO_MANIFEST: &str = r#"
schema_version = "1.0"
manifest_id = "CARGO-ALLOW-ARCH-0001"
controlling_issue = 2580
linked_move_ledger = "CARGO-ALLOW-MOVE-LEDGER-0001"

[[product]]
id = "cargo-allow"
binary = "cargo-allow"
owned_crates = ["cargo-allow", "allow-core"]
forbid_product_dependencies = ["cargo-intent", "cargo-proof"]

[[product]]
id = "cargo-intent"
binary = "cargo-intent"
owned_crates = ["intent-engine", "intent-model"]
forbid_product_dependencies = ["cargo-proof"]

[[product]]
id = "cargo-proof"
binary = "cargo-proof"
owned_crates = ["proof-engine", "proof-protocol"]
forbid_product_dependencies = []

[[shared_crate]]
name = "repo-protocol"
role = "SharedProtocol"
allowed_domain_dependencies = []

[[forbidden_crate_dependency]]
from = "proof-engine"
to = "intent-engine"
repair_hint = "intent-protocol"
"#;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/product-crates")
}

fn load_fixture_metadata(name: &str) -> Result<super::dependency_graph::CargoMetadataGraph, String> {
    let path = fixture_root().join(name);
    let text = std::fs::read_to_string(&path)
        .map_err(|err| format!("read fixture {}: {err}", path.display()))?;
    parse_cargo_metadata_graph(&text)
        .map_err(|err| format!("parse fixture {}: {err}", path.display()))
}

#[test]
fn parse_architecture_manifest_reads_products() -> Result<(), String> {
    let manifest = parse_architecture_manifest(
        r#"
schema_version = "1.0"
manifest_id = "CARGO-ALLOW-ARCH-0001"
controlling_issue = 2580
linked_move_ledger = "CARGO-ALLOW-MOVE-LEDGER-0001"

[[product]]
id = "cargo-allow"
binary = "cargo-allow"
owned_crates = ["cargo-allow"]
forbid_product_dependencies = ["cargo-intent"]
"#,
    )
    .map_err(|err| format!("parse architecture manifest: {err}"))?;
    assert_eq!(manifest.manifest_id, "CARGO-ALLOW-ARCH-0001");
    assert_eq!(manifest.product.len(), 1);
    assert_eq!(
        manifest.product[0].owned_crates,
        vec!["cargo-allow".to_string()]
    );
    Ok(())
}

#[test]
fn repository_architecture_manifest_covers_workspace() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let members = workspace_members_from_manifest(&root)
        .map_err(|err| format!("workspace members: {err}"))?;
    let manifest_path = root.join("policy/product-crates.toml");
    let text = std::fs::read_to_string(&manifest_path)
        .map_err(|err| format!("manifest readable: {err}"))?;
    let manifest =
        parse_architecture_manifest(&text).map_err(|err| format!("parse manifest: {err}"))?;
    let (_, diagnostics, report) = validate_architecture_manifest(manifest, &members);
    if diagnostics
        .iter()
        .any(|diag| diag.kind == ArchitectureDiagnosticKind::UnownedWorkspaceCrate)
    {
        return Err(format!("unowned workspace crates: {diagnostics:?}"));
    }
    if report.owned_crate_count < members.len() {
        return Err("owned crate count should cover workspace members".to_string());
    }
    Ok(())
}

#[test]
fn forbidden_cargo_allow_to_intent_engine_reports_exact_path() -> Result<(), String> {
    let manifest =
        parse_architecture_manifest(REPO_MANIFEST).map_err(|err| format!("parse manifest: {err}"))?;
    let graph = load_fixture_metadata("forbidden-cargo-allow-to-intent-engine.json")?;
    let diagnostics = validate_dependency_law(&manifest, &graph);
    let forbidden = diagnostics
        .iter()
        .find(|diag| diag.kind == ArchitectureDiagnosticKind::ForbiddenProductDependency)
        .ok_or_else(|| format!("expected forbidden product dependency: {diagnostics:?}"))?;
    if forbidden.dependency_path
        != vec!["cargo-allow".to_string(), "intent-engine".to_string()]
    {
        return Err(format!("unexpected dependency path: {:?}", forbidden.dependency_path));
    }
    if !forbidden.message.contains("cargo-intent") {
        return Err(format!("missing product context: {}", forbidden.message));
    }
    Ok(())
}

#[test]
fn forbidden_proof_engine_to_intent_engine_recommends_intent_protocol() -> Result<(), String> {
    let manifest =
        parse_architecture_manifest(REPO_MANIFEST).map_err(|err| format!("parse manifest: {err}"))?;
    let graph = load_fixture_metadata("forbidden-proof-engine-to-intent-engine.json")?;
    let diagnostics = validate_dependency_law(&manifest, &graph);
    let forbidden = diagnostics
        .iter()
        .find(|diag| diag.kind == ArchitectureDiagnosticKind::ForbiddenCrateDependency)
        .ok_or_else(|| format!("expected forbidden crate dependency: {diagnostics:?}"))?;
    if forbidden.dependency_path
        != vec!["proof-engine".to_string(), "intent-engine".to_string()]
    {
        return Err(format!("unexpected dependency path: {:?}", forbidden.dependency_path));
    }
    if !forbidden.message.contains("intent-protocol") {
        return Err(format!("missing repair hint: {}", forbidden.message));
    }
    Ok(())
}

#[test]
fn shared_protocol_domain_leak_detects_product_dependency() -> Result<(), String> {
    let manifest =
        parse_architecture_manifest(REPO_MANIFEST).map_err(|err| format!("parse manifest: {err}"))?;
    let graph = load_fixture_metadata("shared-protocol-domain-leak.json")?;
    let diagnostics = validate_dependency_law(&manifest, &graph);
    let leak = diagnostics
        .iter()
        .find(|diag| diag.kind == ArchitectureDiagnosticKind::SharedProtocolDomainLeak)
        .ok_or_else(|| format!("expected shared protocol domain leak: {diagnostics:?}"))?;
    if leak.dependency_path != vec!["repo-protocol".to_string(), "intent-model".to_string()] {
        return Err(format!("unexpected dependency path: {:?}", leak.dependency_path));
    }
    Ok(())
}

#[test]
fn dev_dependency_bypass_remains_visible() -> Result<(), String> {
    let manifest =
        parse_architecture_manifest(REPO_MANIFEST).map_err(|err| format!("parse manifest: {err}"))?;
    let graph = parse_cargo_metadata_graph(
        r#"{
          "packages": [
            {
              "name": "cargo-allow",
              "dependencies": [
                { "name": "intent-engine", "kind": "dev" }
              ]
            },
            { "name": "intent-engine", "dependencies": [] }
          ]
        }"#,
    )
    .map_err(|err| format!("parse metadata: {err}"))?;
    let diagnostics = validate_dependency_law(&manifest, &graph);
    let forbidden = diagnostics
        .iter()
        .find(|diag| diag.kind == ArchitectureDiagnosticKind::ForbiddenProductDependency)
        .ok_or_else(|| format!("expected dev dependency visibility: {diagnostics:?}"))?;
    if forbidden.dependency_class != Some(super::dependency_graph::DependencyClass::Dev) {
        return Err(format!(
            "expected dev dependency class, got {:?}",
            forbidden.dependency_class
        ));
    }
    Ok(())
}
