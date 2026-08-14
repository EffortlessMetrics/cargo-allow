use super::config::parse_architecture_manifest;
use super::cross_check::validate_architecture_denominators;
use super::dependency_graph::parse_cargo_metadata_graph;
use super::validate::{
    ArchitectureDiagnosticKind, validate_architecture_manifest, validate_dependency_law,
};
use super::workspace::workspace_members_from_manifest;
use crate::product_move::parse_product_move_ledger;
use crate::product_packages::parse_product_package_topology;
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

[[required_crate_dependency]]
from = "proof-engine"
to = "intent-protocol"
rationale_issue = 2936
"#;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/product-crates")
}

fn load_fixture_metadata(
    name: &str,
) -> Result<super::dependency_graph::CargoMetadataGraph, String> {
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
    let manifest = parse_architecture_manifest(REPO_MANIFEST)
        .map_err(|err| format!("parse manifest: {err}"))?;
    let graph = load_fixture_metadata("forbidden-cargo-allow-to-intent-engine.json")?;
    let diagnostics = validate_dependency_law(&manifest, &graph);
    let forbidden = diagnostics
        .iter()
        .find(|diag| diag.kind == ArchitectureDiagnosticKind::ForbiddenProductDependency)
        .ok_or_else(|| format!("expected forbidden product dependency: {diagnostics:?}"))?;
    if forbidden.dependency_path != vec!["cargo-allow".to_string(), "intent-engine".to_string()] {
        return Err(format!(
            "unexpected dependency path: {:?}",
            forbidden.dependency_path
        ));
    }
    if !forbidden.message.contains("cargo-intent") {
        return Err(format!("missing product context: {}", forbidden.message));
    }
    Ok(())
}

#[test]
fn forbidden_proof_engine_to_intent_engine_recommends_intent_protocol() -> Result<(), String> {
    let manifest = parse_architecture_manifest(REPO_MANIFEST)
        .map_err(|err| format!("parse manifest: {err}"))?;
    let graph = load_fixture_metadata("forbidden-proof-engine-to-intent-engine.json")?;
    let diagnostics = validate_dependency_law(&manifest, &graph);
    let forbidden = diagnostics
        .iter()
        .find(|diag| diag.kind == ArchitectureDiagnosticKind::ForbiddenCrateDependency)
        .ok_or_else(|| format!("expected forbidden crate dependency: {diagnostics:?}"))?;
    if forbidden.dependency_path != vec!["proof-engine".to_string(), "intent-engine".to_string()] {
        return Err(format!(
            "unexpected dependency path: {:?}",
            forbidden.dependency_path
        ));
    }
    if !forbidden.message.contains("intent-protocol") {
        return Err(format!("missing repair hint: {}", forbidden.message));
    }
    Ok(())
}

#[test]
fn missing_required_obligation_input_edge_is_reported() -> Result<(), String> {
    let manifest = parse_architecture_manifest(REPO_MANIFEST)
        .map_err(|err| format!("parse manifest: {err}"))?;
    let graph = load_fixture_metadata("missing-required-proof-engine-to-intent-protocol.json")?;
    let diagnostics = validate_dependency_law(&manifest, &graph);
    let missing = diagnostics
        .iter()
        .find(|diag| diag.kind == ArchitectureDiagnosticKind::MissingRequiredCrateDependency)
        .ok_or_else(|| format!("expected missing required dependency: {diagnostics:?}"))?;
    if missing.dependency_path != vec!["proof-engine".to_string(), "intent-protocol".to_string()] {
        return Err(format!(
            "unexpected dependency path: {:?}",
            missing.dependency_path
        ));
    }
    if !missing.message.contains("#2936") {
        return Err(format!("missing rationale issue: {}", missing.message));
    }
    Ok(())
}

#[test]
fn shared_protocol_domain_leak_detects_product_dependency() -> Result<(), String> {
    let manifest = parse_architecture_manifest(REPO_MANIFEST)
        .map_err(|err| format!("parse manifest: {err}"))?;
    let graph = load_fixture_metadata("shared-protocol-domain-leak.json")?;
    let diagnostics = validate_dependency_law(&manifest, &graph);
    let leak = diagnostics
        .iter()
        .find(|diag| diag.kind == ArchitectureDiagnosticKind::SharedProtocolDomainLeak)
        .ok_or_else(|| format!("expected shared protocol domain leak: {diagnostics:?}"))?;
    if leak.dependency_path != vec!["repo-protocol".to_string(), "intent-model".to_string()] {
        return Err(format!(
            "unexpected dependency path: {:?}",
            leak.dependency_path
        ));
    }
    Ok(())
}

#[test]
fn dev_dependency_bypass_remains_visible() -> Result<(), String> {
    let manifest = parse_architecture_manifest(REPO_MANIFEST)
        .map_err(|err| format!("parse manifest: {err}"))?;
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

#[test]
fn repository_architecture_denominators_align_with_topology_and_ledger() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let members = workspace_members_from_manifest(&root)
        .map_err(|err| format!("workspace members: {err}"))?;
    let manifest_path = root.join("policy/product-crates.toml");
    let text = std::fs::read_to_string(&manifest_path)
        .map_err(|err| format!("manifest readable: {err}"))?;
    let manifest =
        parse_architecture_manifest(&text).map_err(|err| format!("parse manifest: {err}"))?;
    let (diagnostics, report) =
        super::cross_check::validate_architecture_denominators_at(&root, &manifest, &members)
            .map_err(|err| format!("validate denominators: {err}"))?;
    if diagnostics.iter().any(|diag| {
        matches!(
            diag.kind,
            ArchitectureDiagnosticKind::ManifestTopologyLinkMismatch
                | ArchitectureDiagnosticKind::ManifestMoveLedgerLinkMismatch
                | ArchitectureDiagnosticKind::PackageTopologyFamilyMismatch
                | ArchitectureDiagnosticKind::ArchitectureCrateMissingFromTopology
                | ArchitectureDiagnosticKind::PackageTopologyCrateMissingFromArchitecture
                | ArchitectureDiagnosticKind::PlannedCrateNowPresent
                | ArchitectureDiagnosticKind::MoveLedgerUnknownTargetCrate
        )
    }) {
        return Err(format!("denominator drift: {diagnostics:?}"));
    }
    if report.architecture_crate_count < report.workspace_member_count {
        return Err("architecture crate inventory should cover workspace members".to_string());
    }
    if report.topology_package_count < report.workspace_member_count {
        return Err("package topology should classify every workspace member".to_string());
    }
    Ok(())
}

#[test]
fn planned_crate_now_present_is_reported() -> Result<(), String> {
    let manifest = parse_architecture_manifest(
        r#"
schema_version = "1.0"
manifest_id = "ARCH-TEST"
controlling_issue = 2580
linked_move_ledger = "LEDGER-TEST"

[[product]]
id = "cargo-allow"
owned_crates = ["cargo-allow"]

[[planned_crate]]
name = "cargo-allow"
owner_product = "cargo-allow"
role = "CargoAllowCore"
stage_issue = 2599
"#,
    )
    .map_err(|err| format!("parse manifest: {err}"))?;
    let topology = parse_product_package_topology(
        r#"
schema_version = "1.0"
topology_id = "TOPO-TEST"
controlling_issue = 2604
linked_architecture_manifest = "ARCH-TEST"

[[package]]
package = "cargo-allow"
product_family = "cargo-allow"
posture = "CargoAllowSupported"
publish = true
candidate_inclusion = true
release_order = 1
"#,
    )
    .map_err(|err| format!("parse topology: {err}"))?;
    let ledger = parse_product_move_ledger(
        r#"
schema_id = "cargo-allow.three-product-move-ledger.v1"
schema_version = 1
ledger_id = "LEDGER-TEST"
controlling_issue = 2598
owner_issue = 2598
topology_issue = 2612
architecture_issue = 2580
package_issue = 2604
parity_issue = 2606
shim_issue = 2607
linked_plan = "plans/three-product-crate-extraction.md"
linked_adr = "CARGO-ALLOW-ADR-0002"
projection = "docs/architecture/product-move-map.md"
plan = "plans/three-product-crate-extraction.md"
claim_boundary = "test"

[discovery]
recursive_roots = []
token_scan_roots = []
selected_files = []
filename_tokens = []
"#,
    )
    .map_err(|err| format!("parse ledger: {err}"))?;
    let members = vec!["crates/cargo-allow".to_string()];
    let (diagnostics, _) =
        validate_architecture_denominators(&manifest, &topology, &ledger, &members);
    let planned = diagnostics
        .iter()
        .find(|diag| diag.kind == ArchitectureDiagnosticKind::PlannedCrateNowPresent)
        .ok_or_else(|| format!("expected planned crate now present: {diagnostics:?}"))?;
    if !planned.crate_names.contains(&"cargo-allow".to_string()) {
        return Err(format!("unexpected planned crate diagnostic: {planned:?}"));
    }
    Ok(())
}

#[test]
fn package_topology_family_mismatch_is_reported() -> Result<(), String> {
    let manifest = parse_architecture_manifest(
        r#"
schema_version = "1.0"
manifest_id = "ARCH-TEST"
controlling_issue = 2580
linked_move_ledger = "LEDGER-TEST"

[[product]]
id = "cargo-intent"
owned_crates = ["intent-model"]
"#,
    )
    .map_err(|err| format!("parse manifest: {err}"))?;
    let topology = parse_product_package_topology(
        r#"
schema_version = "1.0"
topology_id = "TOPO-TEST"
controlling_issue = 2604
linked_architecture_manifest = "ARCH-TEST"

[[package]]
package = "intent-model"
product_family = "cargo-allow"
posture = "CargoAllowSupported"
publish = false
candidate_inclusion = false
release_order = 1
"#,
    )
    .map_err(|err| format!("parse topology: {err}"))?;
    let ledger = parse_product_move_ledger(
        r#"
schema_id = "cargo-allow.three-product-move-ledger.v1"
schema_version = 1
ledger_id = "LEDGER-TEST"
controlling_issue = 2598
owner_issue = 2598
topology_issue = 2612
architecture_issue = 2580
package_issue = 2604
parity_issue = 2606
shim_issue = 2607
linked_plan = "plans/three-product-crate-extraction.md"
linked_adr = "CARGO-ALLOW-ADR-0002"
projection = "docs/architecture/product-move-map.md"
plan = "plans/three-product-crate-extraction.md"
claim_boundary = "test"

[discovery]
recursive_roots = []
token_scan_roots = []
selected_files = []
filename_tokens = []
"#,
    )
    .map_err(|err| format!("parse ledger: {err}"))?;
    let members = vec!["crates/intent-model".to_string()];
    let (diagnostics, _) =
        validate_architecture_denominators(&manifest, &topology, &ledger, &members);
    let mismatch = diagnostics
        .iter()
        .find(|diag| diag.kind == ArchitectureDiagnosticKind::PackageTopologyFamilyMismatch)
        .ok_or_else(|| format!("expected family mismatch: {diagnostics:?}"))?;
    if !mismatch.message.contains("cargo-intent") || !mismatch.message.contains("cargo-allow") {
        return Err(format!("unexpected mismatch message: {}", mismatch.message));
    }
    Ok(())
}

#[test]
fn move_ledger_unknown_target_crate_is_reported() -> Result<(), String> {
    let manifest = parse_architecture_manifest(
        r#"
schema_version = "1.0"
manifest_id = "ARCH-TEST"
controlling_issue = 2580
linked_move_ledger = "LEDGER-TEST"

[[product]]
id = "cargo-allow"
owned_crates = ["cargo-allow"]
"#,
    )
    .map_err(|err| format!("parse manifest: {err}"))?;
    let topology = parse_product_package_topology(
        r#"
schema_version = "1.0"
topology_id = "TOPO-TEST"
controlling_issue = 2604
linked_architecture_manifest = "ARCH-TEST"

[[package]]
package = "cargo-allow"
product_family = "cargo-allow"
posture = "CargoAllowSupported"
publish = true
candidate_inclusion = true
release_order = 1
"#,
    )
    .map_err(|err| format!("parse topology: {err}"))?;
    let ledger = parse_product_move_ledger(
        r#"
schema_id = "cargo-allow.three-product-move-ledger.v1"
schema_version = 1
ledger_id = "LEDGER-TEST"
controlling_issue = 2598
owner_issue = 2598
topology_issue = 2612
architecture_issue = 2580
package_issue = 2604
parity_issue = 2606
shim_issue = 2607
linked_plan = "plans/three-product-crate-extraction.md"
linked_adr = "CARGO-ALLOW-ADR-0002"
projection = "docs/architecture/product-move-map.md"
plan = "plans/three-product-crate-extraction.md"
claim_boundary = "test"

[discovery]
recursive_roots = []
token_scan_roots = []
selected_files = []
filename_tokens = []

[[entry]]
id = "MOVE-TEST-001"
source_kind = "RustModule"
current_paths = ["crates/example/src/lib.rs"]
current_refs = []
current_identity = "crates/example/src/lib.rs"
current_product = "cargo-allow"
current_crate = "cargo-allow"
current_consumers = []
posture = "PrivateImplementation"
target_product = "cargo-intent"
target_crate = "intent-engine"
target_module = "intent_engine"
disposition = "MoveToIntentEngine"
compatibility_strategy = "ParallelParityThenDelete"
schema_producer_impact = "None"
parity_case_ids = []
cutover_stage = "IntentEngine"
expected_cutover_receipt = "none"
old_path_reachability_disposition = "OldPathStillReachable"
active_shim_ids = []
latest_allowed_shim_stage = "IntentEngine"
duplicate_authority_class = "None"
selected_public_producer_after_cutover = "intent-engine"
package_ci_docs_impact = []
removal_issue_or_condition = "issue-2586"
migration_owner_issue = "2586"
risk = "Low"
rollback = "Revert"
status = "Current"
claim_boundary = "test"
next_move = "test"
deletion_output = "test"
"#,
    )
    .map_err(|err| format!("parse ledger: {err}"))?;
    let members = vec!["crates/cargo-allow".to_string()];
    let (diagnostics, _) =
        validate_architecture_denominators(&manifest, &topology, &ledger, &members);
    let unknown = diagnostics
        .iter()
        .find(|diag| diag.kind == ArchitectureDiagnosticKind::MoveLedgerUnknownTargetCrate)
        .ok_or_else(|| format!("expected unknown target crate: {diagnostics:?}"))?;
    if !unknown.crate_names.contains(&"intent-engine".to_string()) {
        return Err(format!("unexpected unknown target diagnostic: {unknown:?}"));
    }
    Ok(())
}
