use super::config::{MoveEntry, ProductMoveLedger, parse_product_move_ledger};
use super::validate::{
    MoveLedgerDiagnosticKind, render_product_move_map, validate_product_move_ledger,
    validate_product_move_ledger_at,
};
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn current_ledger() -> Result<ProductMoveLedger, String> {
    parse_product_move_ledger(include_str!("../../../../policy/product-move-ledger.toml"))
        .map_err(|error| format!("parse current move ledger: {error}"))
}

fn first_entry_mut(ledger: &mut ProductMoveLedger) -> Result<&mut MoveEntry, String> {
    ledger
        .entry
        .first_mut()
        .ok_or_else(|| "move ledger should have a first entry".to_string())
}

#[test]
fn parse_product_move_ledger_requires_exact_schema() -> Result<(), String> {
    let current = include_str!("../../../../policy/product-move-ledger.toml").replace("\r\n", "\n");
    let missing = current.replacen(
        "schema_id = \"cargo-allow.three-product-move-ledger.v1\"\n",
        "",
        1,
    );
    if parse_product_move_ledger(&missing).is_ok() {
        return Err("missing schema_id unexpectedly parsed".to_string());
    }

    let unsupported = current.replacen("schema_version = 1", "schema_version = 2", 1);
    if parse_product_move_ledger(&unsupported).is_ok() {
        return Err("unsupported schema_version unexpectedly parsed".to_string());
    }

    let unknown = current.replacen(
        "schema_version = 1\n",
        "schema_version = 1\nunknown_top_level = true\n",
        1,
    );
    if parse_product_move_ledger(&unknown).is_ok() {
        return Err("unknown top-level field unexpectedly parsed".to_string());
    }

    let missing_version = current.replacen("schema_version = 1\n", "", 1);
    if parse_product_move_ledger(&missing_version).is_ok() {
        return Err("missing schema_version unexpectedly parsed".to_string());
    }

    let unknown_discovery = current.replacen(
        "[discovery]\n",
        "[discovery]\nunknown_discovery_field = true\n",
        1,
    );
    if parse_product_move_ledger(&unknown_discovery).is_ok() {
        return Err("unknown discovery field unexpectedly parsed".to_string());
    }

    let unknown_entry =
        current.replacen("[[entry]]\n", "[[entry]]\nunknown_entry_field = true\n", 1);
    if parse_product_move_ledger(&unknown_entry).is_ok() {
        return Err("unknown entry field unexpectedly parsed".to_string());
    }

    let missing_required_entry =
        current.replacen("id = \"MOVE-INTENT-MODEL-REQUIREMENTS\"\n", "", 1);
    if parse_product_move_ledger(&missing_required_entry).is_ok() {
        return Err("missing required entry field unexpectedly parsed".to_string());
    }

    Ok(())
}

#[test]
fn structural_validation_does_not_use_process_cwd() -> Result<(), String> {
    let mut ledger = current_ledger()?;
    first_entry_mut(&mut ledger)?.current_paths =
        vec!["path/not/available/from/arbitrary/cwd.rs".to_string()];
    let validated = validate_product_move_ledger(ledger);
    if !validated.valid {
        return Err("structural validation should not perform filesystem checks".to_string());
    }
    Ok(())
}

#[test]
fn explicit_root_validation_rejects_missing_and_escaping_paths() -> Result<(), String> {
    let root = repo_root();

    let current_text = include_str!("../../../../policy/product-move-ledger.toml");
    let original_path = "current_paths = [\"crates/allow-policy/src/spec_system/requirement.rs\"]";
    let missing_text = current_text.replacen(
        original_path,
        "current_paths = [\"missing/current/source.rs\"]",
        1,
    );
    if missing_text == current_text {
        return Err("missing-path fixture replacement did not apply".to_string());
    }
    let missing_path = root.join("target/cargo-allow/test-missing-product-move-ledger.toml");
    std::fs::create_dir_all(
        missing_path
            .parent()
            .ok_or_else(|| "missing ledger parent".to_string())?,
    )
    .map_err(|error| format!("create ledger parent: {error}"))?;
    std::fs::write(&missing_path, missing_text)
        .map_err(|error| format!("write missing-path ledger: {error}"))?;
    let (_, diagnostics, _) = validate_product_move_ledger_at(&root, &missing_path)
        .map_err(|error| format!("validate missing-path ledger: {error}"))?;
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == MoveLedgerDiagnosticKind::MissingCurrentPath)
    );

    let escaping_text =
        current_text.replacen(original_path, "current_paths = [\"../outside.rs\"]", 1);
    if escaping_text == current_text {
        return Err("escaping-path fixture replacement did not apply".to_string());
    }
    let escaping_path = root.join("target/cargo-allow/test-escaping-product-move-ledger.toml");
    std::fs::write(&escaping_path, escaping_text)
        .map_err(|error| format!("write escaping ledger: {error}"))?;
    let (_, diagnostics, _) = validate_product_move_ledger_at(&root, &escaping_path)
        .map_err(|error| format!("validate escaping ledger: {error}"))?;
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == MoveLedgerDiagnosticKind::EscapingCurrentPath)
    );

    let _ = std::fs::remove_file(missing_path);
    let _ = std::fs::remove_file(escaping_path);
    Ok(())
}

#[test]
fn structural_validation_rejects_duplicate_unclassified_and_unbounded_entries() -> Result<(), String>
{
    let mut duplicate = current_ledger()?;
    let duplicate_id = duplicate
        .entry
        .first()
        .ok_or_else(|| "missing first entry".to_string())?
        .id
        .clone();
    let second = duplicate
        .entry
        .get_mut(1)
        .ok_or_else(|| "missing second entry".to_string())?;
    second.id = duplicate_id;
    if validate_product_move_ledger(duplicate).valid {
        return Err("duplicate move IDs unexpectedly validated".to_string());
    }

    let mut unclassified = current_ledger()?;
    first_entry_mut(&mut unclassified)?.target_crate = "intent-source".to_string();
    if validate_product_move_ledger(unclassified).valid {
        return Err("unclassified target crate unexpectedly validated".to_string());
    }

    let mut unbounded = current_ledger()?;
    let first = first_entry_mut(&mut unbounded)?;
    first.duplicate_authority_class = "BoundedParityOnly".to_string();
    first.parity_case_ids.clear();
    if validate_product_move_ledger(unbounded).valid {
        return Err("unbounded duplicate authority unexpectedly validated".to_string());
    }

    Ok(())
}

#[test]
fn repository_move_ledger_is_complete_and_projection_is_current() -> Result<(), String> {
    let root = repo_root();
    let ledger_path = root.join("policy/product-move-ledger.toml");
    let (validated, diagnostics, report) = validate_product_move_ledger_at(&root, &ledger_path)
        .map_err(|error| format!("validate repository move ledger: {error}"))?;
    if !validated.valid {
        return Err(format!("move ledger diagnostics: {diagnostics:?}"));
    }
    assert_eq!(report.entry_count, 101);
    assert_eq!(report.target_ratified_count, 99);
    assert_eq!(report.decision_required_count, 1);

    let projection = std::fs::read_to_string(root.join(&validated.ledger.projection))
        .map_err(|error| format!("read move-map projection: {error}"))?;
    assert_eq!(
        projection.replace("\r\n", "\n"),
        render_product_move_map(&validated.ledger)
    );
    Ok(())
}

#[test]
fn explicit_root_validation_detects_unledgered_selected_source() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-product-move-unledgered-{}",
        std::process::id()
    ));
    if root.exists() {
        std::fs::remove_dir_all(&root).map_err(|error| format!("clean temp root: {error}"))?;
    }
    std::fs::create_dir_all(root.join("crates/cargo-allow/src"))
        .map_err(|error| format!("create temp source root: {error}"))?;
    std::fs::create_dir_all(root.join("policy"))
        .map_err(|error| format!("create temp policy root: {error}"))?;
    std::fs::create_dir_all(root.join("docs/architecture"))
        .map_err(|error| format!("create temp docs root: {error}"))?;
    std::fs::write(root.join("crates/cargo-allow/src/known.rs"), "")
        .map_err(|error| format!("write known source: {error}"))?;
    std::fs::write(root.join("crates/cargo-allow/src/spec_system_extra.rs"), "")
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
recursive_roots = []
token_scan_roots = ["crates/cargo-allow/src"]
selected_files = []
filename_tokens = ["spec_system"]

[[entry]]
id = "TEST-KNOWN"
source_kind = "RustModule"
current_paths = ["crates/cargo-allow/src/known.rs"]
current_refs = []
current_identity = "known test source"
current_product = "cargo-allow"
current_crate = "cargo-allow"
current_consumers = ["test"]
posture = "TestOnly"
target_product = "cargo-allow"
target_crate = "cargo-allow"
target_module = "tests"
disposition = "RemainCargoAllowCore"
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
    let ledger = parse_product_move_ledger(text)
        .map_err(|error| format!("parse temp move ledger: {error}"))?;
    std::fs::write(
        root.join("docs/architecture/product-move-map.md"),
        render_product_move_map(&ledger),
    )
    .map_err(|error| format!("write temp projection: {error}"))?;
    let ledger_path = root.join("policy/product-move-ledger.toml");
    std::fs::write(&ledger_path, text).map_err(|error| format!("write temp ledger: {error}"))?;
    let (_, diagnostics, _) = validate_product_move_ledger_at(&root, &ledger_path)
        .map_err(|error| format!("validate temp ledger: {error}"))?;
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.kind == MoveLedgerDiagnosticKind::UnledgeredSelectedSource
    }));

    std::fs::remove_dir_all(&root).map_err(|error| format!("remove temp root: {error}"))?;
    Ok(())
}

#[test]
#[ignore = "manual: regenerate docs/architecture/product-move-map.md"]
fn regenerate_product_move_map_projection() -> Result<(), String> {
    let root = repo_root();
    let ledger_path = root.join("policy/product-move-ledger.toml");
    let text = std::fs::read_to_string(&ledger_path)
        .map_err(|error| format!("read move ledger: {error}"))?;
    let ledger =
        parse_product_move_ledger(&text).map_err(|error| format!("parse move ledger: {error}"))?;
    let projection = render_product_move_map(&ledger);
    std::fs::write(root.join(&ledger.projection), projection)
        .map_err(|error| format!("write move map projection: {error}"))?;
    Ok(())
}
