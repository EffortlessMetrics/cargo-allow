use super::product_move_ledger_fails_check;
use allow_match::CheckMode;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn repository_product_move_ledger_passes_no_new_guard() -> Result<(), String> {
    let root = repo_root();
    if product_move_ledger_fails_check(&root, CheckMode::NoNew)
        .map_err(|error| format!("evaluate move ledger guard: {error}"))?
    {
        return Err("repository move ledger should pass no-new guard".to_string());
    }
    Ok(())
}

#[test]
fn repository_product_move_ledger_is_advisory_in_audit_mode() -> Result<(), String> {
    let root = repo_root();
    if product_move_ledger_fails_check(&root, CheckMode::Audit)
        .map_err(|error| format!("evaluate move ledger guard: {error}"))?
    {
        return Err("audit mode should not enforce move ledger".to_string());
    }
    Ok(())
}

#[test]
fn unledgered_intent_source_fails_no_new_guard() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-product-move-no-new-{}",
        std::process::id()
    ));
    if root.exists() {
        std::fs::remove_dir_all(&root).map_err(|error| format!("clean temp root: {error}"))?;
    }
    std::fs::create_dir_all(root.join("crates/intent-model/src"))
        .map_err(|error| format!("create temp source root: {error}"))?;
    std::fs::create_dir_all(root.join("policy"))
        .map_err(|error| format!("create temp policy root: {error}"))?;
    std::fs::create_dir_all(root.join("docs/architecture"))
        .map_err(|error| format!("create temp docs root: {error}"))?;
    std::fs::write(root.join("crates/intent-model/src/known.rs"), "")
        .map_err(|error| format!("write known source: {error}"))?;
    std::fs::write(root.join("crates/intent-model/src/unledgered.rs"), "")
        .map_err(|error| format!("write unledgered source: {error}"))?;

    let text = r##"
schema_id = "cargo-allow.three-product-move-ledger.v1"
schema_version = 1
ledger_id = "CARGO-ALLOW-MOVE-LEDGER-0001"
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
claim_boundary = "test inventory only"

[discovery]
recursive_roots = ["crates/intent-model/src"]
no_new_enforcement = true
token_scan_roots = []
selected_files = []
filename_tokens = []

[[entry]]
id = "TEST-KNOWN"
source_kind = "RustModule"
current_paths = ["crates/intent-model/src/known.rs"]
current_refs = []
current_identity = "known test source"
current_product = "cargo-intent"
current_crate = "intent-model"
current_consumers = ["test"]
posture = "TestOnly"
target_product = "cargo-intent"
target_crate = "intent-model"
target_module = "tests"
disposition = "MoveToIntentModel"
compatibility_strategy = "NoCompatibilityMove"
schema_producer_impact = "none"
parity_case_ids = []
cutover_stage = "ArchitectureInventory"
expected_cutover_receipt = "CUTOVER-ARCHITECTURE-INVENTORY"
old_path_reachability_disposition = "TestFixtureOnly"
active_shim_ids = []
latest_allowed_shim_stage = "ArchitectureInventory"
duplicate_authority_class = "TestFixtureOnly"
selected_public_producer_after_cutover = "repository"
package_ci_docs_impact = ["test"]
removal_issue_or_condition = "test only"
migration_owner_issue = "#2598"
risk = "Low"
rollback = "delete temp fixture"
status = "TargetRatified"
claim_boundary = "test only"
next_move = "none"
deletion_output = "none"
"##;
    let ledger = allow_policy::product_move::parse_product_move_ledger(text)
        .map_err(|error| format!("parse temp move ledger: {error}"))?;
    std::fs::write(
        root.join("docs/architecture/product-move-map.md"),
        allow_policy::product_move::render_product_move_map(&ledger),
    )
    .map_err(|error| format!("write temp projection: {error}"))?;
    std::fs::write(root.join("policy/product-move-ledger.toml"), text)
        .map_err(|error| format!("write temp ledger: {error}"))?;

    if !product_move_ledger_fails_check(&root, CheckMode::NoNew)
        .map_err(|error| format!("evaluate temp move ledger guard: {error}"))?
    {
        return Err("unledgered intent source should fail no-new guard".to_string());
    }
    if product_move_ledger_fails_check(&root, CheckMode::Audit)
        .map_err(|error| format!("evaluate temp move ledger guard in audit: {error}"))?
    {
        return Err("audit mode should not enforce unledgered intent source".to_string());
    }

    std::fs::remove_dir_all(&root).map_err(|error| format!("remove temp root: {error}"))?;
    Ok(())
}
