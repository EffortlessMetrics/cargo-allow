//! Governance return-prevention guards (#3544 / #2942 step 8).
//!
//! Mirrors the #3317 obligation-authority guard pattern for governance:
//! repository-intent governance authority lives in intent-model (DTOs) and
//! intent-engine (validation) — see the MOVE-GOV window rows (#3542). These
//! guards fail if canonical governance surface returns to or grows in
//! allow-policy:
//!
//! - the governance module file set is frozen (no new files);
//! - the public symbol inventory of those modules is frozen (no new
//!   canonical types/validators; deletions are the goal and are allowed);
//! - allow-policy's manifest must never gain intent dependencies (runtime
//!   dependency law; dev-scope parity lives in cargo-allow's dev-deps).

use std::collections::BTreeSet;
use std::path::PathBuf;

const GOVERNANCE_DIRS: &[&str] = &[
    "crates/allow-policy/src/product_crates",
    "crates/allow-policy/src/product_move",
    "crates/allow-policy/src/extraction_parity",
    "crates/allow-policy/src/extraction_shims",
];

/// Frozen file set of the governance modules (37 files at #3544). New files
/// require updating this list with an explicit reviewed exception.
const FROZEN_FILES: &[&str] = &[
    "crates/allow-policy/src/extraction_parity/compare.rs",
    "crates/allow-policy/src/extraction_parity/config.rs",
    "crates/allow-policy/src/extraction_parity/cutover_receipt.rs",
    "crates/allow-policy/src/extraction_parity/mod.rs",
    "crates/allow-policy/src/extraction_parity/producer.rs",
    "crates/allow-policy/src/extraction_parity/reachability.rs",
    "crates/allow-policy/src/extraction_parity/tests.rs",
    "crates/allow-policy/src/extraction_parity/validate.rs",
    "crates/allow-policy/src/extraction_shims/config.rs",
    "crates/allow-policy/src/extraction_shims/mod.rs",
    "crates/allow-policy/src/extraction_shims/tests.rs",
    "crates/allow-policy/src/extraction_shims/validate.rs",
    "crates/allow-policy/src/product_crates/config.rs",
    "crates/allow-policy/src/product_crates/mod.rs",
    "crates/allow-policy/src/product_crates/tests.rs",
    "crates/allow-policy/src/product_crates/v2.rs",
    "crates/allow-policy/src/product_move/config.rs",
    "crates/allow-policy/src/product_move/mod.rs",
    "crates/allow-policy/src/product_move/tests.rs",
    "crates/allow-policy/src/product_move/validate.rs",
    "crates/allow-policy/src/product_packages/config.rs",
    "crates/allow-policy/src/product_packages/mod.rs",
    "crates/allow-policy/src/product_packages/tests.rs",
    "crates/allow-policy/src/product_packages/v2.rs",
    "crates/allow-policy/src/product_packages/validate.rs",
];

/// Frozen public symbol inventory (144 symbols at #3544). Additions fail;
/// deletions are the migration goal.
const FROZEN_SYMBOLS: &[&str] = &[
    "ArchitectureManifest",
    "CrateRole",
    "ForbiddenCrateDependency",
    "ProductDefinition",
    "RequiredCrateDependency",
    "SharedCrateDefinition",
    "ArchitectureManifestV2",
    "ArchitecturePackageRowV2",
    "AuthorityKind",
    "AuthorityNode",
    "CargoDependencyClass",
    "CargoDependencyEdge",
    "CargoMetadataGraphV2",
    "CargoPackageIdResolver",
    "ClosureDiagnostic",
    "ClosureResultKind",
    "CrateIdentityV2",
    "CutoverReceiptDiagnostic",
    "CutoverReceiptDiagnosticKind",
    "DenominatorReport",
    "DependencyClass",
    "DependencyEdge",
    "ExtractionCutoverReceipt",
    "ExtractionCutoverReceiptEvidence",
    "ExtractionParityCase",
    "ExtractionParityRegistry",
    "ExtractionShim",
    "ExtractionShimKind",
    "ExtractionShimRegistry",
    "ExtractionStage",
    "IdentityDiagnostic",
    "IdentityDiagnosticKind",
    "MoveDiscovery",
    "MoveEntry",
    "MoveLedgerDiagnostic",
    "MoveLedgerDiagnosticKind",
    "MoveLedgerReport",
    "OldPathCase",
    "OldPathDisposition",
    "PackagePosture",
    "PackageResolutionError",
    "PackageTopologyDiagnostic",
    "PackageTopologyDiagnosticKind",
    "PackageTopologyEntry",
    "PackageTopologyEntryV2",
    "PackageTopologyReport",
    "ParityComparison",
    "ParityComparisonResult",
    "ParityDiagnostic",
    "ParityDiagnosticKind",
    "ParityDisposition",
    "ParityObservation",
    "ParityReport",
    "PlannedCrate",
    "ProductMoveLedger",
    "ProductPackageTopology",
    "ProductPackageTopologyV2",
    "PublicationStateV2",
    "ReachabilityDiagnostic",
    "ReachabilityDiagnosticKind",
    "ReachabilityReport",
    "ReconcileDiagnostic",
    "ReconcileDiagnosticKind",
    "ReconcileReport",
    "ShimDiagnostic",
    "ShimDiagnosticKind",
    "ShimPosture",
    "ShimReport",
    "ShimStatus",
    "StageReceiptTemplate",
    "ValidatedProductMoveLedger",
    "VersionSourceV2",
    "as_str",
    "compare_observations",
    "corpus_digest",
    "extraction_shim_registry_blocks_enforced_check",
    "find_identity_by_library",
    "find_identity_by_package",
    "format_product_move_ledger_diagnostics",
    "from_manifest",
    "is_clean",
    "knows",
    "load_workspace_metadata_graph_v2",
    "parse",
    "parse_architecture_manifest",
    "parse_architecture_manifest_at",
    "parse_architecture_manifest_v2",
    "parse_architecture_manifest_v2_at",
    "parse_cargo_metadata_graph_v2",
    "parse_extraction_cutover_receipt",
    "parse_extraction_cutover_receipt_at",
    "parse_extraction_parity_registry",
    "parse_extraction_parity_registry_at",
    "parse_extraction_shim_registry",
    "parse_extraction_shim_registry_at",
    "parse_product_move_ledger",
    "parse_product_move_ledger_at",
    "parse_product_package_topology",
    "parse_product_package_topology_at",
    "parse_product_package_topology_v2",
    "parse_product_package_topology_v2_at",
    "produce_extraction_cutover_receipt",
    "product_move_ledger_blocks_enforced_check",
    "render_json",
    "render_product_move_map",
    "resolve",
    "satisfies_migration",
    "shortest_closure_path",
    "validate_cutover_reachability",
    "validate_duplicate_authority",
    "validate_extraction_cutover_receipt",
    "validate_extraction_parity_registry",
    "validate_extraction_parity_registry_at",
    "validate_extraction_shim_registry",
    "validate_extraction_shim_registry_at",
    "validate_old_path_reachability",
    "validate_product_move_ledger",
    "validate_product_move_ledger_at",
    "validate_product_package_topology",
    "validate_product_package_topology_at",
];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn governance_sources(root: &std::path::Path) -> Result<Vec<PathBuf>, String> {
    let mut sources = Vec::new();
    for dir in GOVERNANCE_DIRS {
        let dir_path = root.join(dir);
        let entries =
            std::fs::read_dir(&dir_path).map_err(|err| format!("read dir {dir}: {err}"))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                sources.push(path);
            }
        }
    }
    Ok(sources)
}

#[test]
fn governance_module_file_set_is_frozen() -> Result<(), String> {
    let root = workspace_root();
    let sources = governance_sources(&root)?;
    let frozen: BTreeSet<&str> = FROZEN_FILES.iter().copied().collect();
    let mut additions = Vec::new();
    for path in &sources {
        let rel = path
            .strip_prefix(&root)
            .map_err(|err| err.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        if !frozen.contains(rel.as_str()) {
            additions.push(rel);
        }
    }
    if !additions.is_empty() {
        return Err(format!(
            "new file(s) in allow-policy governance modules: {additions:?} — canonical              governance belongs in intent-model/intent-engine (#2942); extend the window              and this freeze only with an explicit reviewed exception (#3544)"
        ));
    }
    Ok(())
}

#[test]
fn governance_symbol_inventory_does_not_grow() -> Result<(), String> {
    let root = workspace_root();
    let sources = governance_sources(&root)?;
    let frozen: BTreeSet<&str> = FROZEN_SYMBOLS.iter().copied().collect();
    let mut observed: BTreeSet<String> = BTreeSet::new();
    for path in &sources {
        let text = std::fs::read_to_string(path)
            .map_err(|err| format!("read {}: {err}", path.display()))?;
        for line in text.lines() {
            let trimmed = line.trim_start();
            for keyword in ["fn", "struct", "enum", "trait"] {
                let Some(rest) = trimmed.strip_prefix(&format!("pub {keyword} ")) else {
                    continue;
                };
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    observed.insert(name);
                }
            }
        }
    }
    let additions: Vec<&str> = observed
        .iter()
        .map(String::as_str)
        .filter(|name| !frozen.contains(name))
        .collect();
    if !additions.is_empty() {
        return Err(format!(
            "new public governance symbols in allow-policy: {additions:?} — canonical              governance types/validators live in intent-model/intent-engine (#2942/#3544)"
        ));
    }
    Ok(())
}

#[test]
fn allow_policy_manifest_has_no_intent_dependencies() -> Result<(), String> {
    let root = workspace_root();
    let manifest = std::fs::read_to_string(root.join("crates/allow-policy/Cargo.toml"))
        .map_err(|err| format!("read allow-policy manifest: {err}"))?;
    let table: toml::Table =
        toml::from_str(&manifest).map_err(|err| format!("parse allow-policy manifest: {err}"))?;
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(deps) = table.get(section).and_then(|v| v.as_table()) else {
            continue;
        };
        let intent_deps: Vec<&str> = deps
            .keys()
            .map(String::as_str)
            .filter(|name| name.starts_with("intent-"))
            .collect();
        if !intent_deps.is_empty() {
            return Err(format!(
                "allow-policy must not depend on intent crates (found {intent_deps:?} in                  {section}); governance parity runs at dev scope in cargo-allow and the                  runtime path uses the bounded governance projection (#3548)"
            ));
        }
    }
    Ok(())
}
