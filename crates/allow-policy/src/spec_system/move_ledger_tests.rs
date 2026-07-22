use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveLedger {
    schema_id: String,
    schema_version: u32,
    owner_issue: u32,
    topology_issue: u32,
    architecture_issue: u32,
    package_issue: u32,
    parity_issue: u32,
    shim_issue: u32,
    projection: String,
    plan: String,
    claim_boundary: String,
    discovery: Discovery,
    entry: Vec<MoveEntry>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Discovery {
    recursive_roots: Vec<String>,
    token_scan_roots: Vec<String>,
    selected_files: Vec<String>,
    filename_tokens: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveEntry {
    id: String,
    source_kind: String,
    current_paths: Vec<String>,
    current_refs: Vec<String>,
    current_identity: String,
    current_product: String,
    current_crate: String,
    current_consumers: Vec<String>,
    posture: String,
    target_product: String,
    target_crate: String,
    target_module: String,
    disposition: String,
    compatibility_strategy: String,
    schema_producer_impact: String,
    parity_case_ids: Vec<String>,
    cutover_stage: String,
    expected_cutover_receipt: String,
    old_path_reachability_disposition: String,
    active_shim_ids: Vec<String>,
    latest_allowed_shim_stage: String,
    duplicate_authority_class: String,
    selected_public_producer_after_cutover: String,
    package_ci_docs_impact: Vec<String>,
    removal_issue_or_condition: String,
    migration_owner_issue: String,
    risk: String,
    rollback: String,
    status: String,
    claim_boundary: String,
    next_move: String,
    deletion_output: String,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn parse_ledger(text: &str) -> Result<MoveLedger, String> {
    toml::from_str(text).map_err(|error| format!("move ledger should parse as TOML: {error}"))
}

fn closed(values: &[&'static str]) -> BTreeSet<&'static str> {
    values.iter().copied().collect()
}

fn validate_ledger(root: &Path, ledger: &MoveLedger) -> Result<(), String> {
    if ledger.schema_id != "cargo-allow.three-product-move-ledger.v1"
        || ledger.schema_version != 1
        || ledger.owner_issue != 2598
        || ledger.topology_issue != 2612
        || ledger.architecture_issue != 2580
        || ledger.package_issue != 2604
        || ledger.parity_issue != 2606
        || ledger.shim_issue != 2607
    {
        return Err("move_ledger_metadata_invalid".to_string());
    }
    if ledger.projection != "docs/architecture/product-move-map.md"
        || ledger.plan != "plans/three-product-crate-extraction.md"
        || ledger.claim_boundary.trim().is_empty()
        || ledger.entry.is_empty()
    {
        return Err("move_ledger_denominator_invalid".to_string());
    }

    let target_crates = closed(&[
        "allow-core", "allow-policy", "allow-inventory", "allow-files", "allow-rust",
        "allow-match", "allow-report", "allow-diff", "allow-policy-legacy", "cargo-allow",
        "repo-protocol", "repo-snapshot", "repo-edit", "rust-source-index",
        "intent-model", "intent-protocol", "intent-engine", "intent-edit", "cargo-intent",
        "proof-protocol", "proof-provider-api", "proof-engine", "proof-adapter-command",
        "proof-adapter-cargo-allow", "proof-adapter-ripr", "proof-adapter-hawk", "cargo-proof",
    ]);
    let source_kinds = closed(&[
        "RustModule", "RustSurface", "CommandSurface", "TestFixture", "RepositoryAsset",
        "Schema", "PackageSurface", "IssueSet",
    ]);
    let postures = closed(&[
        "PrivateImplementation", "ExperimentalPublic", "SupportedPublic",
        "HistoricalCompatibility", "TestOnly", "PublicImplementation",
        "CanonicalAuthoredSource", "SupportedDocumentation", "CIConfiguration",
        "PlanningAuthority",
    ]);
    let products = closed(&["cargo-allow", "cargo-intent", "cargo-proof", "shared"]);
    let dispositions = closed(&[
        "MoveToSharedProtocol", "MoveToSharedSnapshot", "MoveToRustSourceIndex",
        "MoveToIntentModel", "MoveToIntentProtocol", "MoveToIntentEngine",
        "MoveToIntentEdit", "MoveToCargoIntentApp", "MoveToProofProtocol",
        "MoveToProofProviderApi", "MoveToProofEngine", "MoveToProofAdapter",
        "RemainCargoAllowCore", "RemainProviderOwned", "CompatibilityAdapter",
        "HistoricalReaderOnly", "GeneratedProjection", "DeleteAfterParity",
        "DeleteImmediatelyAsDead", "RepositoryDecisionRequired",
    ]);
    let strategies = closed(&[
        "ParallelParityThenDelete", "HistoricalReadOnlyAdapter",
        "CompatibilityFacadeThenDelete", "FixtureParityThenRetire",
        "ProcessDelegationThenDelete", "ReExportThenDelete", "AdaptThenDelete",
        "OneWayProcessDelegation", "SchemaAdapterThenDelete", "NoCompatibilityMove",
        "ReadInPlaceNewProducer", "PackageAssetMigration", "DocsSplitThenDeprecate",
        "CILaneSplit", "PackageTopologyBeforeMove", "IssueOwnershipUpdate",
        "MigrationControlThenDelete",
    ]);
    let stages = closed(&[
        "ArchitectureInventory", "RepoProtocol", "RepoSnapshot", "IntentModel",
        "IntentProtocol", "IntentEngine", "CargoIntentFrontDoor",
        "CargoAllowCompatibilityCutover", "EmbeddedIntentDeletion", "RustSourceIndex",
        "IntentEdit", "ProofProtocol", "ProofProviderApi", "ProofEngineAndCli",
        "ProviderAdapters", "IndependentPackaging",
    ]);
    let reachability = closed(&[
        "OldPathStillReachable", "Deleted", "CompileUnreachable",
        "FeatureUnreachableInSupportedCandidate", "CompatibilityProjectionOnly",
        "HistoricalReaderOnly", "TestFixtureOnly", "ExplicitlyDeferredWithinBound",
    ]);
    let duplicate_classes = closed(&[
        "None", "BoundedParityOnly", "CompatibilityProjection", "HistoricalReader",
        "TestFixtureOnly", "GeneratedView",
    ]);
    let statuses = closed(&[
        "Inventoried", "TargetRatified", "NewOwnerImplemented", "ParityOutstanding",
        "ParityAccepted", "CutoverOutstanding", "CutoverCurrent",
        "OldPathStillReachable", "CompatibilityOnly", "HistoricalOnly", "Deletable",
        "Deleted", "BlockedByUnreviewedDifference", "RepositoryDecisionRequired",
    ]);
    let risks = closed(&["Low", "Medium", "High", "Critical"]);

    let mut ids = BTreeSet::new();
    let mut registered_paths = BTreeSet::new();

    for entry in &ledger.entry {
        if !ids.insert(entry.id.as_str()) {
            return Err(format!("move_ledger_duplicate_id:{}", entry.id));
        }
        if !source_kinds.contains(entry.source_kind.as_str())
            || !postures.contains(entry.posture.as_str())
            || !products.contains(entry.target_product.as_str())
            || !target_crates.contains(entry.target_crate.as_str())
            || !dispositions.contains(entry.disposition.as_str())
            || !strategies.contains(entry.compatibility_strategy.as_str())
            || !stages.contains(entry.cutover_stage.as_str())
            || !stages.contains(entry.latest_allowed_shim_stage.as_str())
            || !reachability.contains(entry.old_path_reachability_disposition.as_str())
            || !duplicate_classes.contains(entry.duplicate_authority_class.as_str())
            || !statuses.contains(entry.status.as_str())
            || !risks.contains(entry.risk.as_str())
        {
            return Err(format!("move_ledger_closed_value_invalid:{}", entry.id));
        }
        if entry.current_identity.trim().is_empty()
            || entry.current_product.trim().is_empty()
            || entry.current_crate.trim().is_empty()
            || entry.current_consumers.is_empty()
            || entry.target_module.trim().is_empty()
            || entry.schema_producer_impact.trim().is_empty()
            || entry.expected_cutover_receipt.trim().is_empty()
            || entry.selected_public_producer_after_cutover.trim().is_empty()
            || entry.package_ci_docs_impact.is_empty()
            || entry.removal_issue_or_condition.trim().is_empty()
            || entry.migration_owner_issue.trim().is_empty()
            || entry.rollback.trim().is_empty()
            || entry.claim_boundary.trim().is_empty()
            || entry.next_move.trim().is_empty()
            || entry.deletion_output.trim().is_empty()
        {
            return Err(format!("move_ledger_required_field_missing:{}", entry.id));
        }
        if entry.source_kind == "IssueSet" {
            if entry.current_refs.is_empty() || !entry.current_paths.is_empty() {
                return Err(format!("move_ledger_issue_set_invalid:{}", entry.id));
            }
        } else if entry.current_paths.is_empty() {
            return Err(format!("move_ledger_current_paths_missing:{}", entry.id));
        }
        for current_path in &entry.current_paths {
            registered_paths.insert(current_path.clone());
            if !root.join(current_path).exists() {
                return Err(format!(
                    "move_ledger_current_source_missing:{}:{}",
                    entry.id, current_path
                ));
            }
        }
        if entry.duplicate_authority_class != "None"
            && entry.duplicate_authority_class != "TestFixtureOnly"
            && (entry.parity_case_ids.is_empty()
                || entry.removal_issue_or_condition.trim().is_empty()
                || entry.expected_cutover_receipt.trim().is_empty())
        {
            return Err(format!("move_ledger_unbounded_duplicate:{}", entry.id));
        }
        if !entry.active_shim_ids.is_empty()
            && entry.latest_allowed_shim_stage.trim().is_empty()
        {
            return Err(format!("move_ledger_unbounded_shim:{}", entry.id));
        }
    }

    let discovered = discover_selected_paths(root, &ledger.discovery)?;
    let unledgered = discovered
        .difference(&registered_paths)
        .cloned()
        .collect::<Vec<_>>();
    if !unledgered.is_empty() {
        return Err(format!("move_ledger_unledgered_selected:{unledgered:?}"));
    }

    Ok(())
}

fn collect_files(root: &Path, relative: &Path, out: &mut BTreeSet<String>) -> Result<(), String> {
    let absolute = root.join(relative);
    if absolute.is_file() {
        out.insert(relative.to_string_lossy().replace('\\', "/"));
        return Ok(());
    }
    let entries = std::fs::read_dir(&absolute)
        .map_err(|error| format!("move ledger discovery failed for {}: {error}", absolute.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("move ledger discovery entry failed: {error}"))?;
        let child = relative.join(entry.file_name());
        if entry.path().is_dir() {
            collect_files(root, &child, out)?;
        } else if entry.path().is_file() {
            out.insert(child.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn discover_selected_paths(root: &Path, discovery: &Discovery) -> Result<BTreeSet<String>, String> {
    let mut selected = BTreeSet::new();
    for recursive_root in &discovery.recursive_roots {
        collect_files(root, Path::new(recursive_root), &mut selected)?;
    }
    for selected_file in &discovery.selected_files {
        selected.insert(selected_file.clone());
    }
    for scan_root in &discovery.token_scan_roots {
        let mut candidates = BTreeSet::new();
        collect_files(root, Path::new(scan_root), &mut candidates)?;
        for candidate in candidates {
            let file_name = Path::new(&candidate)
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            if discovery
                .filename_tokens
                .iter()
                .any(|token| file_name.contains(token))
            {
                selected.insert(candidate);
            }
        }
    }
    Ok(selected)
}

fn render_projection(ledger: &MoveLedger) -> String {
    let mut entries = ledger.entry.iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| left.id.cmp(&right.id));
    let mut statuses = BTreeMap::<&str, usize>::new();
    let mut dispositions = BTreeMap::<&str, usize>::new();
    for entry in &entries {
        *statuses.entry(entry.status.as_str()).or_default() += 1;
        *dispositions.entry(entry.disposition.as_str()).or_default() += 1;
    }

    let mut out = String::new();
    out.push_str("# Three-Product Move Map\n\n");
    out.push_str("> Generated from `.allow/artifacts/product-move-ledger.toml`. Edit the ledger,\n");
    out.push_str("> then update this projection through the checked renderer. Do not maintain a\n");
    out.push_str("> second ownership spreadsheet.\n\n");
    out.push_str("## Denominator\n\n");
    out.push_str(&format!("- Ledger schema: `{}` generation `{}`\n", ledger.schema_id, ledger.schema_version));
    out.push_str(&format!("- Entries: **{}**\n", entries.len()));
    out.push_str(&format!("- Topology authority: Issue **#{}**\n", ledger.topology_issue));
    out.push_str(&format!("- Move/deletion owner: Issue **#{}**\n", ledger.owner_issue));
    out.push_str("- Current posture: inventory and target ratification only; no implementation moved.\n\n");
    out.push_str("### Status counts\n\n");
    for (status, count) in statuses {
        out.push_str(&format!("- `{status}`: **{count}**\n"));
    }
    out.push_str("\n### Disposition counts\n\n");
    for (disposition, count) in dispositions {
        out.push_str(&format!("- `{disposition}`: **{count}**\n"));
    }
    out.push_str("\n## Executable next frontier\n\n");
    out.push_str("1. **#2580** — `ProductCrateArchitectureV1` from this ledger and #2612.\n");
    out.push_str("2. **#2604** — `ProductPackageTopologyV1` without changing the ten-crate candidate.\n");
    out.push_str("3. **#2607** — register only shims required by the first source moves.\n");
    out.push_str("4. **#2606** — parity, stage, reachability, and cutover receipt contracts.\n");
    out.push_str("5. **#2582** — first real move: minimal `repo-protocol` envelope with parity evidence.\n\n");
    out.push_str("## Entries\n\n");
    for entry in entries {
        out.push_str(&format!("### `{}`\n", entry.id));
        out.push_str(&format!("- Current: {}\n", entry.current_identity));
        out.push_str(&format!(
            "- Target: `{} / {}::{}`\n",
            entry.target_product, entry.target_crate, entry.target_module
        ));
        out.push_str(&format!("- Disposition: `{}`\n", entry.disposition));
        out.push_str(&format!(
            "- Stage/status: `{}` / `{}`\n",
            entry.cutover_stage, entry.status
        ));
        out.push_str(&format!(
            "- Old path: `{}`\n",
            entry.old_path_reachability_disposition
        ));
        out.push_str(&format!("- Next: {}\n", entry.next_move));
        out.push_str(&format!("- Deletion output: {}\n\n", entry.deletion_output));
    }
    out.push_str("## Transition rules\n\n");
    out.push_str("- A bounded duplicate names parity cases, a cutover receipt, a latest shim stage,\n");
    out.push_str("  an owner, and a deletion condition.\n");
    out.push_str("- `OldPathStillReachable` is an inventory fact, not approval to retain a second\n");
    out.push_str("  evaluator after the selected cutover.\n");
    out.push_str("- Repository-authored intent sources may remain at their paths while producer and\n");
    out.push_str("  semantic ownership move.\n");
    out.push_str("- Cargo-allow provider payloads stay cargo-allow-owned and travel through neutral\n");
    out.push_str("  envelopes; no initial `cargo-allow-protocol` crate exists.\n");
    out.push_str("- Physical repository extraction still requires #2558, #2605, #2559, and a later\n");
    out.push_str("  explicit authorization.\n\n");
    out.push_str("## Claim boundary\n\n");
    out.push_str(&ledger.claim_boundary);
    out.push('\n');
    out
}

#[test]
fn three_product_move_ledger_is_complete_and_projection_is_current() -> Result<(), String> {
    let root = repo_root();
    let text = std::fs::read_to_string(root.join(".allow/artifacts/product-move-ledger.toml"))
        .map_err(|error| format!("move ledger should be readable: {error}"))?;
    let ledger = parse_ledger(&text)?;
    validate_ledger(&root, &ledger)?;
    if ledger.entry.len() != 37 {
        return Err(format!("expected 37 move-ledger entries, got {}", ledger.entry.len()));
    }
    let projection = std::fs::read_to_string(root.join(&ledger.projection))
        .map_err(|error| format!("move map should be readable: {error}"))?;
    if projection != render_projection(&ledger) {
        return Err("move_ledger_projection_drift".to_string());
    }
    Ok(())
}

#[test]
fn move_ledger_rejects_unclassified_target_crate() -> Result<(), String> {
    let root = repo_root();
    let text = std::fs::read_to_string(root.join(".allow/artifacts/product-move-ledger.toml"))
        .map_err(|error| error.to_string())?;
    let mut ledger = parse_ledger(&text)?;
    ledger.entry[0].target_crate = "intent-source".to_string();
    let error = match validate_ledger(&root, &ledger) {
        Ok(()) => return Err("unclassified target unexpectedly passed".to_string()),
        Err(error) => error,
    };
    assert!(error.contains("closed_value_invalid"));
    Ok(())
}

#[test]
fn move_ledger_rejects_missing_current_source() -> Result<(), String> {
    let root = repo_root();
    let text = std::fs::read_to_string(root.join(".allow/artifacts/product-move-ledger.toml"))
        .map_err(|error| error.to_string())?;
    let mut ledger = parse_ledger(&text)?;
    ledger.entry[0].current_paths = vec!["crates/allow-policy/src/spec_system/missing.rs".to_string()];
    let error = match validate_ledger(&root, &ledger) {
        Ok(()) => return Err("missing current source unexpectedly passed".to_string()),
        Err(error) => error,
    };
    assert!(error.contains("current_source_missing"));
    Ok(())
}

#[test]
fn move_ledger_detects_unledgered_selected_source() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-move-ledger-unledgered-{}",
        std::process::id()
    ));
    if root.exists() {
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
    }
    std::fs::create_dir_all(root.join("crates/cargo-allow/src"))
        .map_err(|error| error.to_string())?;
    std::fs::write(root.join("crates/cargo-allow/src/spec_system_extra.rs"), "")
        .map_err(|error| error.to_string())?;
    let discovery = Discovery {
        recursive_roots: Vec::new(),
        token_scan_roots: vec!["crates/cargo-allow/src".to_string()],
        selected_files: Vec::new(),
        filename_tokens: vec!["spec_system".to_string()],
    };
    let selected = discover_selected_paths(&root, &discovery)?;
    std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
    assert!(selected.contains("crates/cargo-allow/src/spec_system_extra.rs"));
    Ok(())
}

#[test]
fn move_ledger_rejects_unbounded_transitional_duplicate() -> Result<(), String> {
    let root = repo_root();
    let text = std::fs::read_to_string(root.join(".allow/artifacts/product-move-ledger.toml"))
        .map_err(|error| error.to_string())?;
    let mut ledger = parse_ledger(&text)?;
    ledger.entry[0].duplicate_authority_class = "BoundedParityOnly".to_string();
    ledger.entry[0].parity_case_ids.clear();
    let error = match validate_ledger(&root, &ledger) {
        Ok(()) => return Err("unbounded duplicate unexpectedly passed".to_string()),
        Err(error) => error,
    };
    assert!(error.contains("unbounded_duplicate"));
    Ok(())
}
