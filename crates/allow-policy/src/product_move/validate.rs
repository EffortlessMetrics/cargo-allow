use super::config::{MoveDiscovery, MoveEntry, ProductMoveLedger, parse_product_move_ledger_at};
use allow_core::{CargoAllowError, CargoAllowResult, normalize_path};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

pub const PRODUCT_MOVE_LEDGER_RELATIVE_PATH: &str = "policy/product-move-ledger.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveLedgerDiagnosticKind {
    DuplicateId,
    EmptyEntrySet,
    MetadataInvalid,
    ClosedValueInvalid,
    RequiredFieldMissing,
    InvalidIssueSet,
    EscapingCurrentPath,
    MissingCurrentPath,
    UnboundedDuplicateAuthority,
    UnboundedShim,
    UnledgeredSelectedSource,
    ProjectionDrift,
}

impl MoveLedgerDiagnosticKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateId => "duplicate_id",
            Self::EmptyEntrySet => "empty_entry_set",
            Self::MetadataInvalid => "metadata_invalid",
            Self::ClosedValueInvalid => "closed_value_invalid",
            Self::RequiredFieldMissing => "required_field_missing",
            Self::InvalidIssueSet => "invalid_issue_set",
            Self::EscapingCurrentPath => "escaping_current_path",
            Self::MissingCurrentPath => "missing_current_path",
            Self::UnboundedDuplicateAuthority => "unbounded_duplicate_authority",
            Self::UnboundedShim => "unbounded_shim",
            Self::UnledgeredSelectedSource => "unledgered_selected_source",
            Self::ProjectionDrift => "projection_drift",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveLedgerDiagnostic {
    pub kind: MoveLedgerDiagnosticKind,
    pub message: String,
    pub entry_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedProductMoveLedger {
    pub ledger: ProductMoveLedger,
    pub valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MoveLedgerReport {
    pub entry_count: usize,
    pub target_ratified_count: usize,
    pub decision_required_count: usize,
    pub old_path_reachable_count: usize,
    pub disposition_counts: BTreeMap<String, usize>,
    pub status_counts: BTreeMap<String, usize>,
    /// Ledger parity_case_ids references with no registered parity case
    /// (#3576). Pre-existing drift: 84 references use a planned
    /// PARITY-UPPERCASE naming scheme the registry never adopted. Counted
    /// (not diagnosed) until the naming decision lands.
    pub dangling_parity_case_reference_count: usize,
}

pub fn validate_product_move_ledger(ledger: ProductMoveLedger) -> ValidatedProductMoveLedger {
    let diagnostics = collect_structural_diagnostics(&ledger);
    ValidatedProductMoveLedger {
        ledger,
        valid: diagnostics.is_empty(),
    }
}

pub fn validate_product_move_ledger_at(
    root: &Path,
    ledger_path: &Path,
) -> CargoAllowResult<(
    ValidatedProductMoveLedger,
    Vec<MoveLedgerDiagnostic>,
    MoveLedgerReport,
)> {
    let text = std::fs::read_to_string(ledger_path).map_err(|error| {
        CargoAllowError::new(format!(
            "product move ledger unreadable at {}: {error}",
            ledger_path.display()
        ))
    })?;
    let ledger = parse_product_move_ledger_at(Some(ledger_path), &text)?;
    let mut diagnostics = collect_structural_diagnostics(&ledger);
    diagnostics.extend(collect_root_diagnostics(root, &ledger));
    let report = summarize_report(&ledger);
    let validated = ValidatedProductMoveLedger {
        ledger,
        valid: diagnostics.is_empty(),
    };
    Ok((validated, diagnostics, report))
}

pub fn product_move_ledger_blocks_enforced_check(root: &Path) -> CargoAllowResult<bool> {
    let ledger_path = root.join(PRODUCT_MOVE_LEDGER_RELATIVE_PATH);
    if !ledger_path.is_file() {
        return Ok(false);
    }
    let (validated, diagnostics, _) = validate_product_move_ledger_at(root, &ledger_path)?;
    if !validated.ledger.discovery.no_new_enforcement {
        return Ok(false);
    }
    Ok(!diagnostics.is_empty())
}

pub fn format_product_move_ledger_diagnostics(diagnostics: &[MoveLedgerDiagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| {
            let entry_ids = if diagnostic.entry_ids.is_empty() {
                String::new()
            } else {
                format!(" [{}]", diagnostic.entry_ids.join(", "))
            };
            format!(
                "{}: {}{}",
                diagnostic.kind.as_str(),
                diagnostic.message,
                entry_ids
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn normalize_projection_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

pub fn render_product_move_map(ledger: &ProductMoveLedger) -> String {
    let mut entries = ledger.entry.iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| left.id.cmp(&right.id));
    let report = summarize_report(ledger);

    let mut out = String::new();
    out.push_str("# Three-Product Move Map\n\n");
    out.push_str("> Generated from `policy/product-move-ledger.toml`. Edit the ledger,\n");
    out.push_str("> then update this projection through the checked renderer. Do not maintain a\n");
    out.push_str("> second ownership spreadsheet.\n\n");
    out.push_str("## Denominator\n\n");
    out.push_str(&format!(
        "- Ledger schema: `{}` generation `{}`\n",
        ledger.schema_id, ledger.schema_version
    ));
    out.push_str(&format!("- Entries: **{}**\n", report.entry_count));
    out.push_str(&format!(
        "- Topology authority: Issue **#{}**\n",
        ledger.topology_issue
    ));
    out.push_str(&format!(
        "- Move/deletion owner: Issue **#{}**\n",
        ledger.owner_issue
    ));
    out.push_str(
        "- Current posture: inventory and target ratification only; no implementation moved.\n\n",
    );
    out.push_str("### Status counts\n\n");
    for (status, count) in &report.status_counts {
        out.push_str(&format!("- `{status}`: **{count}**\n"));
    }
    out.push_str("\n### Disposition counts\n\n");
    for (disposition, count) in &report.disposition_counts {
        out.push_str(&format!("- `{disposition}`: **{count}**\n"));
    }
    out.push_str("\n## Executable next frontier\n\n");
    out.push_str("1. **#2580** — `ProductCrateArchitectureV1` from this ledger and #2612.\n");
    out.push_str(
        "2. **#2604** — `ProductPackageTopologyV1` without changing the ten-crate candidate.\n",
    );
    out.push_str("3. **#2607** — register only shims required by the first source moves.\n");
    out.push_str("4. **#2606** — parity, stage, reachability, and cutover receipt contracts.\n");
    out.push_str(
        "5. **#2582** — first real move: minimal `repo-protocol` envelope with parity evidence.\n\n",
    );
    out.push_str("## Entries\n\n");
    for entry in entries {
        out.push_str(&format!("### `{}`\n\n", entry.id));
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
        out.push_str(&format!(
            "- Removal: {}\n",
            entry.removal_issue_or_condition
        ));
        out.push_str(&format!("- Next: {}\n", entry.next_move));
        out.push_str(&format!("- Deletion output: {}\n\n", entry.deletion_output));
    }
    out.push_str("## Transition rules\n\n");
    out.push_str(
        "- A bounded duplicate names parity cases, a cutover receipt, a latest shim stage,\n",
    );
    out.push_str("  an owner, and a deletion condition.\n");
    out.push_str(
        "- `OldPathStillReachable` is an inventory fact, not approval to retain a second\n",
    );
    out.push_str("  evaluator after the selected cutover.\n");
    out.push_str(
        "- Repository-authored intent sources may remain at their paths while producer and\n",
    );
    out.push_str("  semantic ownership move.\n");
    out.push_str(
        "- Cargo-allow provider payloads stay cargo-allow-owned and travel through neutral\n",
    );
    out.push_str("  envelopes; no initial `cargo-allow-protocol` crate exists.\n");
    out.push_str(
        "- Physical repository extraction still requires #2558, #2605, #2559, and a later\n",
    );
    out.push_str("  explicit authorization.\n\n");
    out.push_str("## Claim boundary\n\n");
    out.push_str(&ledger.claim_boundary);
    out.push('\n');
    out
}

fn collect_structural_diagnostics(ledger: &ProductMoveLedger) -> Vec<MoveLedgerDiagnostic> {
    let mut diagnostics = Vec::new();
    if ledger.entry.is_empty() {
        diagnostics.push(diagnostic(
            MoveLedgerDiagnosticKind::EmptyEntrySet,
            "product move ledger has no entries",
            Vec::new(),
        ));
    }
    if ledger.controlling_issue != 2598
        || ledger.owner_issue != 2598
        || ledger.topology_issue != 2612
        || ledger.architecture_issue != 2580
        || ledger.package_issue != 2604
        || ledger.parity_issue != 2606
        || ledger.shim_issue != 2607
        || ledger.ledger_id != "CARGO-ALLOW-MOVE-LEDGER-0001"
        || ledger.linked_plan != "plans/three-product-crate-extraction.md"
        || ledger.linked_adr != "CARGO-ALLOW-ADR-0002"
        || ledger.projection != "docs/architecture/product-move-map.md"
        || ledger.plan != "plans/three-product-crate-extraction.md"
        || ledger.claim_boundary.trim().is_empty()
    {
        diagnostics.push(diagnostic(
            MoveLedgerDiagnosticKind::MetadataInvalid,
            "product move ledger metadata does not match the retained architecture denominator",
            Vec::new(),
        ));
    }

    let source_kinds = closed(&[
        "RustModule",
        "RustSurface",
        "CommandSurface",
        "TestFixture",
        "RepositoryAsset",
        "Schema",
        "PackageSurface",
        "IssueSet",
    ]);
    let postures = closed(&[
        "PrivateImplementation",
        "ExperimentalPublic",
        "SupportedPublic",
        "HistoricalCompatibility",
        "TestOnly",
        "PublicImplementation",
        "CanonicalAuthoredSource",
        "SupportedDocumentation",
        "CIConfiguration",
        "PlanningAuthority",
    ]);
    let products = closed(&["cargo-allow", "cargo-intent", "cargo-proof", "shared"]);
    let target_crates = closed(&[
        "allow-core",
        "allow-policy",
        "allow-inventory",
        "allow-files",
        "allow-rust",
        "allow-match",
        "allow-report",
        "allow-diff",
        "allow-policy-legacy",
        "cargo-allow",
        "repo-protocol",
        "repo-snapshot",
        "repo-edit",
        "rust-source-index",
        "effortless-repo-protocol",
        "effortless-repo-snapshot",
        "effortless-repo-edit",
        "effortless-rust-source-index",
        "intent-model",
        "intent-protocol",
        "intent-engine",
        "intent-edit",
        "cargo-intent",
        "proof-protocol",
        "proof-provider-api",
        "proof-engine",
        "proof-adapter-command",
        "proof-adapter-cargo-allow",
        "proof-adapter-ripr",
        "proof-adapter-hawk",
        "cargo-proof",
    ]);
    let dispositions = closed(&[
        "MoveToSharedProtocol",
        "MoveToSharedSnapshot",
        "MoveToRustSourceIndex",
        "MoveToIntentModel",
        "MoveToIntentProtocol",
        "MoveToIntentEngine",
        "MoveToIntentEdit",
        "MoveToCargoIntentApp",
        "MoveToProofProtocol",
        "MoveToProofProviderApi",
        "MoveToProofAdapterCommand",
        "MoveToProofEngine",
        "MoveToCargoProofApp",
        "MoveToProofAdapter",
        "RemainCargoAllowCore",
        "RemainCargoIntent",
        "RemainProviderOwned",
        "CompatibilityAdapter",
        "HistoricalReaderOnly",
        "GeneratedProjection",
        "DeleteAfterParity",
        "DeleteImmediatelyAsDead",
        "RepositoryDecisionRequired",
    ]);
    let strategies = closed(&[
        "ParallelParityThenDelete",
        "HistoricalReadOnlyAdapter",
        "CompatibilityFacadeThenDelete",
        "FixtureParityThenRetire",
        "ProcessDelegationThenDelete",
        "ReExportThenDelete",
        "AdaptThenDelete",
        "OneWayProcessDelegation",
        "SchemaAdapterThenDelete",
        "NoCompatibilityMove",
        "ReadInPlaceNewProducer",
        "PackageAssetMigration",
        "DocsSplitThenDeprecate",
        "CILaneSplit",
        "PackageTopologyBeforeMove",
        "IssueOwnershipUpdate",
        "MigrationControlThenDelete",
    ]);
    let stages = closed(&[
        "ArchitectureInventory",
        "RepoProtocol",
        "RepoSnapshot",
        "RepoEdit",
        "IntentModel",
        "IntentProtocol",
        "IntentEngine",
        "CargoIntentFrontDoor",
        "CargoAllowCompatibilityCutover",
        "EmbeddedIntentDeletion",
        "RustSourceIndex",
        "IntentEdit",
        "ProofProtocol",
        "ProofProviderApi",
        "ProofEngineAndCli",
        "ProviderAdapters",
        "IndependentPackaging",
    ]);
    let reachability = closed(&[
        "OldPathStillReachable",
        "Deleted",
        "CompileUnreachable",
        "FeatureUnreachableInSupportedCandidate",
        "CompatibilityProjectionOnly",
        "HistoricalReaderOnly",
        "TestFixtureOnly",
        "ExplicitlyDeferredWithinBound",
    ]);
    let duplicate_classes = closed(&[
        "None",
        "BoundedParityOnly",
        "CompatibilityProjection",
        "HistoricalReader",
        "TestFixtureOnly",
        "GeneratedView",
    ]);
    let statuses = closed(&[
        "Inventoried",
        "TargetRatified",
        "NewOwnerImplemented",
        "ParityOutstanding",
        "ParityAccepted",
        "CutoverOutstanding",
        "CutoverCurrent",
        "OldPathStillReachable",
        "CompatibilityOnly",
        "HistoricalOnly",
        "Deletable",
        "Deleted",
        "BlockedByUnreviewedDifference",
        "RepositoryDecisionRequired",
    ]);
    let risks = closed(&["Low", "Medium", "High", "Critical"]);

    let mut seen = BTreeSet::new();
    for entry in &ledger.entry {
        if !seen.insert(entry.id.as_str()) {
            diagnostics.push(diagnostic(
                MoveLedgerDiagnosticKind::DuplicateId,
                format!("duplicate move ledger entry id `{}`", entry.id),
                vec![entry.id.clone()],
            ));
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
            diagnostics.push(diagnostic(
                MoveLedgerDiagnosticKind::ClosedValueInvalid,
                format!("closed move-ledger value invalid for `{}`", entry.id),
                vec![entry.id.clone()],
            ));
        }
        if required_field_missing(entry) {
            diagnostics.push(diagnostic(
                MoveLedgerDiagnosticKind::RequiredFieldMissing,
                format!("required move-ledger field missing for `{}`", entry.id),
                vec![entry.id.clone()],
            ));
        }
        if entry.source_kind == "IssueSet" {
            if entry.current_refs.is_empty() || !entry.current_paths.is_empty() {
                diagnostics.push(diagnostic(
                    MoveLedgerDiagnosticKind::InvalidIssueSet,
                    format!("issue-set entry shape invalid for `{}`", entry.id),
                    vec![entry.id.clone()],
                ));
            }
        } else if entry.current_paths.is_empty() {
            diagnostics.push(diagnostic(
                MoveLedgerDiagnosticKind::RequiredFieldMissing,
                format!("current_paths missing for `{}`", entry.id),
                vec![entry.id.clone()],
            ));
        }
        if entry.duplicate_authority_class != "None"
            && entry.duplicate_authority_class != "TestFixtureOnly"
            && (entry.parity_case_ids.is_empty()
                || entry.expected_cutover_receipt.trim().is_empty()
                || entry.removal_issue_or_condition.trim().is_empty())
        {
            diagnostics.push(diagnostic(
                MoveLedgerDiagnosticKind::UnboundedDuplicateAuthority,
                format!("duplicate authority is unbounded for `{}`", entry.id),
                vec![entry.id.clone()],
            ));
        }
        if !entry.active_shim_ids.is_empty() && entry.latest_allowed_shim_stage.trim().is_empty() {
            diagnostics.push(diagnostic(
                MoveLedgerDiagnosticKind::UnboundedShim,
                format!("active shim has no latest stage for `{}`", entry.id),
                vec![entry.id.clone()],
            ));
        }
    }

    diagnostics
}

fn collect_root_diagnostics(root: &Path, ledger: &ProductMoveLedger) -> Vec<MoveLedgerDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut registered_paths = BTreeSet::new();
    for entry in &ledger.entry {
        for current_path in &entry.current_paths {
            registered_paths.insert(current_path.clone());
            if !is_safe_repo_relative(current_path) {
                diagnostics.push(diagnostic(
                    MoveLedgerDiagnosticKind::EscapingCurrentPath,
                    format!(
                        "current identity escapes repository for `{}`: {current_path}",
                        entry.id
                    ),
                    vec![entry.id.clone()],
                ));
                continue;
            }
            if !root.join(current_path).exists() {
                diagnostics.push(diagnostic(
                    MoveLedgerDiagnosticKind::MissingCurrentPath,
                    format!(
                        "current identity path missing for `{}`: {current_path}",
                        entry.id
                    ),
                    vec![entry.id.clone()],
                ));
            }
        }
    }

    match discover_selected_paths(root, &ledger.discovery) {
        Ok(discovered) => {
            let unledgered = discovered
                .difference(&registered_paths)
                .cloned()
                .collect::<Vec<_>>();
            if !unledgered.is_empty() {
                diagnostics.push(diagnostic(
                    MoveLedgerDiagnosticKind::UnledgeredSelectedSource,
                    format!("selected sources missing move-ledger disposition: {unledgered:?}"),
                    Vec::new(),
                ));
            }
        }
        Err(message) => diagnostics.push(diagnostic(
            MoveLedgerDiagnosticKind::MissingCurrentPath,
            message,
            Vec::new(),
        )),
    }

    if is_safe_repo_relative(&ledger.projection) {
        let projection_path = root.join(&ledger.projection);
        match std::fs::read_to_string(&projection_path) {
            Ok(current)
                if normalize_projection_text(&current) == render_product_move_map(ledger) => {}
            Ok(_) => diagnostics.push(diagnostic(
                MoveLedgerDiagnosticKind::ProjectionDrift,
                format!("product move map is stale: {}", projection_path.display()),
                Vec::new(),
            )),
            Err(error) => diagnostics.push(diagnostic(
                MoveLedgerDiagnosticKind::ProjectionDrift,
                format!(
                    "product move map unreadable at {}: {error}",
                    projection_path.display()
                ),
                Vec::new(),
            )),
        }
    } else {
        diagnostics.push(diagnostic(
            MoveLedgerDiagnosticKind::EscapingCurrentPath,
            format!("projection path escapes repository: {}", ledger.projection),
            Vec::new(),
        ));
    }

    diagnostics
}

fn required_field_missing(entry: &MoveEntry) -> bool {
    entry.id.trim().is_empty()
        || entry.current_identity.trim().is_empty()
        || entry.current_product.trim().is_empty()
        || entry.current_crate.trim().is_empty()
        || entry.current_consumers.is_empty()
        || entry.target_product.trim().is_empty()
        || entry.target_crate.trim().is_empty()
        || entry.target_module.trim().is_empty()
        || entry.schema_producer_impact.trim().is_empty()
        || entry.expected_cutover_receipt.trim().is_empty()
        || entry
            .selected_public_producer_after_cutover
            .trim()
            .is_empty()
        || entry.package_ci_docs_impact.is_empty()
        || entry.removal_issue_or_condition.trim().is_empty()
        || entry.migration_owner_issue.trim().is_empty()
        || entry.rollback.trim().is_empty()
        || entry.claim_boundary.trim().is_empty()
        || entry.next_move.trim().is_empty()
        || entry.deletion_output.trim().is_empty()
}

fn is_safe_repo_relative(value: &str) -> bool {
    let path = Path::new(value);
    !value.trim().is_empty()
        && !path.is_absolute()
        && !path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
}

fn collect_files(root: &Path, relative: &Path, out: &mut BTreeSet<String>) -> Result<(), String> {
    if !is_safe_repo_relative(&relative.to_string_lossy()) {
        return Err(format!(
            "move-ledger discovery root escapes repository: {}",
            relative.display()
        ));
    }
    let absolute = root.join(relative);
    if absolute.is_symlink() {
        return Err(format!(
            "move-ledger discovery root is a symlink: {}",
            absolute.display()
        ));
    }
    if absolute.is_file() {
        out.insert(normalize_path(relative));
        return Ok(());
    }
    let entries = std::fs::read_dir(&absolute).map_err(|error| {
        format!(
            "move-ledger discovery failed for {}: {error}",
            absolute.display()
        )
    })?;
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("move-ledger discovery entry failed: {error}"))?;
        let child = relative.join(entry.file_name());
        let child_path = entry.path();
        if child_path.is_symlink() {
            continue;
        }
        if child_path.is_dir() {
            collect_files(root, &child, out)?;
        } else if child_path.is_file() {
            out.insert(normalize_path(&child));
        }
    }
    Ok(())
}

fn discover_selected_paths(
    root: &Path,
    discovery: &MoveDiscovery,
) -> Result<BTreeSet<String>, String> {
    let mut selected = BTreeSet::new();
    for recursive_root in &discovery.recursive_roots {
        collect_files(root, Path::new(recursive_root), &mut selected)?;
    }
    for selected_file in &discovery.selected_files {
        if !is_safe_repo_relative(selected_file) {
            return Err(format!(
                "selected move-ledger file escapes repository: {selected_file}"
            ));
        }
        selected.insert(selected_file.clone());
    }
    for scan_root in &discovery.token_scan_roots {
        let mut candidates = BTreeSet::new();
        collect_files(root, Path::new(scan_root), &mut candidates)?;
        for candidate in candidates {
            let Some(file_name) = Path::new(&candidate)
                .file_name()
                .and_then(|value| value.to_str())
            else {
                continue;
            };
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

fn summarize_report(ledger: &ProductMoveLedger) -> MoveLedgerReport {
    let mut report = MoveLedgerReport {
        entry_count: ledger.entry.len(),
        ..MoveLedgerReport::default()
    };
    for entry in &ledger.entry {
        if entry.status == "TargetRatified" {
            report.target_ratified_count += 1;
        }
        if entry.status == "RepositoryDecisionRequired" {
            report.decision_required_count += 1;
        }
        if entry.old_path_reachability_disposition == "OldPathStillReachable" {
            report.old_path_reachable_count += 1;
        }
        *report
            .disposition_counts
            .entry(entry.disposition.clone())
            .or_default() += 1;
        *report
            .status_counts
            .entry(entry.status.clone())
            .or_default() += 1;
    }
    report.dangling_parity_case_reference_count = count_dangling_parity_case_references(ledger);
    report
}

/// Count parity_case_ids references with no registered parity case
/// (#3576). Registered ids come from extraction-parity.toml; the ledger's
/// planned PARITY-UPPERCASE ids that were never registered count as
/// dangling.
fn count_dangling_parity_case_references(ledger: &ProductMoveLedger) -> usize {
    let registry_text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../policy/extraction-parity.toml"),
    )
    .unwrap_or_default();
    let registered: BTreeSet<String> = toml::from_str::<toml::Table>(&registry_text)
        .ok()
        .and_then(|table| {
            table
                .get("case")
                .and_then(|value| value.as_array())
                .map(|cases| {
                    cases
                        .iter()
                        .filter_map(|case| {
                            case.get("id")
                                .and_then(|id| id.as_str())
                                .map(str::to_string)
                        })
                        .collect()
                })
        })
        .unwrap_or_default();
    ledger
        .entry
        .iter()
        .flat_map(|entry| entry.parity_case_ids.iter())
        .filter(|case_id| !registered.contains(*case_id))
        .count()
}

fn closed(values: &[&'static str]) -> BTreeSet<&'static str> {
    values.iter().copied().collect()
}

fn diagnostic(
    kind: MoveLedgerDiagnosticKind,
    message: impl Into<String>,
    entry_ids: Vec<String>,
) -> MoveLedgerDiagnostic {
    MoveLedgerDiagnostic {
        kind,
        message: message.into(),
        entry_ids,
    }
}
