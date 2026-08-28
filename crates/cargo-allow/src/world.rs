use allow_core::{
    AllowConfig, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, Finding,
    SOURCE_FILE_READ_MAX_BYTES, normalize_path, source_tree_path_is_ignored,
};
use allow_inventory::{
    Inventory, InventoryCompleteness, InventoryOptions, InventorySource, inventory,
    resolve_source_tree_root,
};
use allow_policy::federation::{
    FederationEvaluation, PrecedenceTier, evaluate_source_exception_policy,
};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use effortless_repo_snapshot::{
    StagedEntryKind, StagedPathRead, StagedRepositorySnapshot, read_staged_path,
    staged_repository_snapshot,
};

thread_local! {
    static SCAN_CACHE: RefCell<allow_rust::ScanCache> = RefCell::new(allow_rust::ScanCache::new());
}

const TOOL_OWNED_CACHE_GLOB: &str = "target/cargo-allow/cache/**";

fn inventory_options_with_tool_cache_ignore(mut options: InventoryOptions) -> InventoryOptions {
    if !options
        .ignored
        .iter()
        .any(|glob| glob == TOOL_OWNED_CACHE_GLOB)
    {
        options.ignored.push(TOOL_OWNED_CACHE_GLOB.to_string());
    }
    options
}

fn scan_rust_files_with_cache_mode(
    root: &Path,
    files: &[PathBuf],
    persistent_cache: bool,
) -> CargoAllowResult<allow_rust::RustScanResult> {
    SCAN_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if persistent_cache {
            match allow_rust::RootBoundScanCacheStore::open(
                root,
                allow_rust::scan_cache_generation(),
            ) {
                Ok(mut store) => {
                    let result = allow_rust::scan_rust_files_cached_with_root_bound_store(
                        root, files, &mut cache, &mut store,
                    );
                    if let Err(disposition) = store.flush_with_disposition() {
                        eprintln!(
                            "warning: persistent Rust scan cache disabled for this run ({})",
                            disposition.as_str()
                        );
                    }
                    return result;
                }
                Err(disposition) => {
                    eprintln!(
                        "warning: persistent Rust scan cache unavailable ({})",
                        disposition.as_str()
                    );
                }
            }
        }
        allow_rust::scan_rust_files_cached(root, files, &mut cache)
    })
}

use crate::{
    EvidenceValidationMode, InventoryFacts, canonical_companion_findings, current_dir,
    evidence_inventory::{
        current_evidence_source_tree_files, validate_evidence_references_for_source_tree,
    },
    extend_unique_findings, load_policy_at_path, parse_kind_filter,
};

type StagedRustInputs = (Vec<(PathBuf, String)>, Vec<(PathBuf, String)>);

/// The exact staged source-exception evaluation result. The identity is kept
/// separate from [`InventoryFacts`] so the ordinary worktree report contract
/// does not acquire staged-only ownership semantics by accident.
pub(crate) struct StagedWorld {
    pub(crate) root: PathBuf,
    pub(crate) cfg: AllowConfig,
    pub(crate) findings: Vec<Finding>,
    pub(crate) inventory_facts: InventoryFacts,
    pub(crate) federation: FederationEvaluation,
    pub(crate) source_identity: String,
    pub(crate) evidence_source_tree_files: BTreeSet<String>,
    pub(crate) product_move_ledger_present: bool,
}

/// Evaluate source-exception findings against one exact Git-index candidate.
///
/// The source bytes and inventory come from the exact staged-index adapter;
/// this path never falls back to dirty worktree bytes. Worktree-derived
/// companion sensors are rejected until they have an equivalent staged source
/// adapter, rather than being silently mixed into an exact candidate result.
pub(crate) fn load_staged_world(
    explicit_root: Option<&Path>,
    config: Option<&Path>,
    kind_filter: Option<&str>,
) -> CargoAllowResult<StagedWorld> {
    let cwd = current_dir()?;
    let root = resolve_source_tree_root(explicit_root, cwd)?;
    let snapshot = crate::command_support::snapshot_result(staged_repository_snapshot(&root))?;
    let (policy_path, federation) = evaluate_source_exception_policy(&root, config)?;
    reject_unsupported_staged_federation(&snapshot, &federation)?;
    let policy_relative = normalize_to_repo_relative(&root, &policy_path);
    let policy_text = read_staged_text(&snapshot, &policy_relative).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!(
                "exact staged source-exception check could not read policy candidate {}: {error}",
                policy_relative.display()
            ),
        )
    })?;
    let policy_digest = allow_core::sha256_v1_bytes(policy_text.as_bytes());
    let cfg = allow_policy::parse_policy_with_reportable_evidence_at(&policy_path, &policy_text)?;
    reject_unsupported_staged_companion_sensors(&cfg)?;
    let options = inventory_options_with_tool_cache_ignore(InventoryOptions {
        ignored: cfg.workspace.ignored.clone(),
        generated: cfg.workspace.generated.clone(),
        include_untracked: false,
    });
    let source_identity = snapshot.identity.semantic_hash.clone();
    let inventory = staged_inventory(&snapshot, &options);
    let inventory_facts =
        InventoryFacts::scanned_inventory(&inventory).with_policy_digest(policy_digest);
    let evidence_source_tree_files = inventory
        .files
        .iter()
        .map(normalize_path)
        .collect::<BTreeSet<_>>();
    let product_move_ledger_present = evidence_source_tree_files
        .contains(allow_policy::product_move::PRODUCT_MOVE_LEDGER_RELATIVE_PATH);
    let (manifests, sources) = staged_rust_inputs(&snapshot, &inventory)?;
    let packages = allow_rust::source_package_contexts_from_sources(manifests);
    let mut findings = Vec::new();
    let mut rust_files_with_parse_errors = 0usize;
    for (path, text) in sources {
        // Match the ordinary filesystem scanner's BOM normalization. The
        // staged source view owns the bytes, but it must not create a second
        // syntax interpretation for Windows-edited UTF-8 sources.
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let scan = allow_rust::scan_rust_source_with_completeness(&path, text);
        if scan.has_parse_error {
            rust_files_with_parse_errors += 1;
        }
        let mut source_findings = scan.findings;
        allow_rust::apply_source_package_context(&path, &packages, &mut source_findings);
        findings.extend(source_findings);
    }
    findings.extend(allow_files::scan_files_with_options(
        &inventory.files,
        &allow_files::FileScanOptions {
            generated: options.generated.clone(),
            file_families: cfg.workspace.file_families.clone(),
            content_aware_generated: false,
        },
    ));
    let staged_companion_findings =
        crate::companion::staged_companion_findings(&cfg, &inventory.files)?;
    extend_unique_findings(&mut findings, staged_companion_findings);
    if let Some(kind) = kind_filter {
        let parsed = parse_kind_filter(kind)?;
        findings.retain(|finding| parsed.matches_finding(finding));
    }
    if let Some(provenance) = federation.active_provenance.clone() {
        for finding in &mut findings {
            finding.ledger = Some(provenance.clone());
        }
    }
    let final_snapshot =
        crate::command_support::snapshot_result(staged_repository_snapshot(&root))?;
    if final_snapshot.identity.semantic_hash != source_identity {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Scan,
            "Git index changed while scanning the exact staged source-exception candidate",
        ));
    }
    Ok(StagedWorld {
        root,
        cfg,
        findings,
        inventory_facts: inventory_facts
            .with_rust_files_with_parse_errors(rust_files_with_parse_errors),
        federation,
        source_identity,
        evidence_source_tree_files,
        product_move_ledger_present,
    })
}

fn staged_inventory(snapshot: &StagedRepositorySnapshot, options: &InventoryOptions) -> Inventory {
    let mut files = snapshot
        .entries
        .iter()
        .filter(|entry| entry.stage == 0)
        .filter(|entry| {
            matches!(
                entry.kind,
                StagedEntryKind::RegularFile | StagedEntryKind::ExecutableFile
            )
        })
        .filter_map(|entry| entry.path.clone())
        .filter(|path| !source_tree_path_is_ignored(path, &options.ignored))
        .collect::<Vec<_>>();
    files.sort();
    files.dedup();
    let completeness =
        if snapshot.completeness == effortless_repo_snapshot::StagedSnapshotCompleteness::Partial {
            InventoryCompleteness::Partial
        } else if !options.ignored.is_empty() || !options.generated.is_empty() {
            InventoryCompleteness::Scoped
        } else {
            InventoryCompleteness::Complete
        };
    Inventory {
        files,
        source: InventorySource::GitIndexStagedCandidate,
        completeness,
        empty_git_tracked: snapshot.entries.is_empty(),
        deleted_tracked: Vec::new(),
        inaccessible_paths: Vec::new(),
        git_error: None,
        skipped_paths: Vec::new(),
        submodule_paths: Vec::new(),
    }
}

fn staged_rust_inputs(
    snapshot: &StagedRepositorySnapshot,
    inventory: &Inventory,
) -> CargoAllowResult<StagedRustInputs> {
    let mut manifests = Vec::new();
    let mut sources = Vec::new();
    for path in &inventory.files {
        if path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml") {
            manifests.push((path.clone(), read_staged_text(snapshot, path)?));
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            sources.push((path.clone(), read_staged_text(snapshot, path)?));
        }
    }
    Ok((manifests, sources))
}

fn read_staged_text(snapshot: &StagedRepositorySnapshot, path: &Path) -> CargoAllowResult<String> {
    let bytes = match crate::command_support::snapshot_result(read_staged_path(snapshot, path))? {
        StagedPathRead::Regular(bytes) => bytes,
        StagedPathRead::Missing => {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Inventory,
                format!(
                    "staged source file {} is absent from the candidate",
                    path.display()
                ),
            ));
        }
        StagedPathRead::Unsupported { kind, .. } => {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Inventory,
                format!(
                    "staged source file {} has unsupported entry kind {kind:?}",
                    path.display()
                ),
            ));
        }
    };
    if (bytes.len() as u64) > SOURCE_FILE_READ_MAX_BYTES {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Scan,
            format!(
                "staged source file {} exceeds the {}-byte source-read limit",
                path.display(),
                SOURCE_FILE_READ_MAX_BYTES
            ),
        ));
    }
    String::from_utf8(bytes).map_err(|source| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Scan,
            format!("staged source file {} is not valid UTF-8", path.display()),
        )
        .with_cause(&source)
    })
}

fn reject_unsupported_staged_companion_sensors(cfg: &AllowConfig) -> CargoAllowResult<()> {
    let unsupported = cfg
        .allow
        .iter()
        .filter(|entry| {
            matches!(
                entry.kind,
                allow_core::FindingKind::GeneratedCode | allow_core::FindingKind::PolicyException
            )
        })
        .map(|entry| entry.family.as_deref().unwrap_or("<unspecified>"))
        .filter(|family| !crate::companion::staged_companion_family_supported(family))
        .collect::<BTreeSet<_>>();
    if !unsupported.is_empty() {
        return Err(CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::Unsupported,
            format!(
                "exact staged source-exception evaluation does not yet support companion families {}; run the tracked-worktree check or use the staged compatibility profile",
                unsupported
                    .into_iter()
                    .map(|family| format!("`{family}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }
    Ok(())
}

fn reject_unsupported_staged_federation(
    snapshot: &StagedRepositorySnapshot,
    federation: &FederationEvaluation,
) -> CargoAllowResult<()> {
    let staged_federation_metadata = snapshot.entries.iter().any(|entry| {
        entry.stage == 0
            && entry.path.as_deref().is_some_and(|path| {
                path.components()
                    .next()
                    .is_some_and(|component| component.as_os_str() == ".allow")
            })
    });
    if staged_federation_metadata
        || !federation.ledger_contributors.is_empty()
        || !federation.divergences.is_empty()
    {
        return Err(CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::Unsupported,
            "exact staged source-exception evaluation does not yet support federated policy inputs; run the tracked-worktree check or stage through the federation-aware adapter",
        ));
    }
    Ok(())
}

type WorldLoadResult = CargoAllowResult<(
    PathBuf,
    AllowConfig,
    Vec<Finding>,
    InventoryFacts,
    FederationEvaluation,
)>;

type ScopedWorldLoadResult = CargoAllowResult<(
    PathBuf,
    AllowConfig,
    Vec<Finding>,
    InventoryFacts,
    FederationEvaluation,
    Option<allow_rust::RustFileScanOutcome>,
)>;

pub(crate) fn load_world(
    explicit_root: Option<&Path>,
    config: Option<&Path>,
    require_config: bool,
    kind_filter: Option<&str>,
    include_untracked: bool,
) -> WorldLoadResult {
    load_world_with_evidence_mode(
        explicit_root,
        config,
        require_config,
        kind_filter,
        include_untracked,
        EvidenceValidationMode::Abort,
    )
}

pub(crate) fn load_world_with_evidence_mode(
    explicit_root: Option<&Path>,
    config: Option<&Path>,
    require_config: bool,
    kind_filter: Option<&str>,
    include_untracked: bool,
    evidence_validation: EvidenceValidationMode,
) -> WorldLoadResult {
    load_world_with_evidence_mode_and_cache(
        explicit_root,
        config,
        require_config,
        kind_filter,
        include_untracked,
        evidence_validation,
        true,
    )
}

pub(crate) fn load_world_with_evidence_mode_and_cache(
    explicit_root: Option<&Path>,
    config: Option<&Path>,
    require_config: bool,
    kind_filter: Option<&str>,
    include_untracked: bool,
    evidence_validation: EvidenceValidationMode,
    persistent_cache: bool,
) -> WorldLoadResult {
    let cwd = current_dir()?;
    let root = resolve_source_tree_root(explicit_root, cwd)?;
    let (policy_path, federation) = match evaluate_source_exception_policy(&root, config) {
        Ok(value) => value,
        Err(_err) if !require_config => {
            return load_world_without_policy_and_cache(
                &root,
                kind_filter,
                include_untracked,
                evidence_validation,
                empty_federation_evaluation(PrecedenceTier::DiscoveryFallback),
                persistent_cache,
            );
        }
        Err(err) => return Err(err),
    };
    let (cfg, policy_digest) =
        crate::policy_config::load_policy_at_path_with_digest(policy_path, evidence_validation)?;
    let opts = inventory_options_with_tool_cache_ignore(InventoryOptions {
        ignored: cfg.workspace.ignored.clone(),
        generated: cfg.workspace.generated.clone(),
        include_untracked,
    });
    let inventory = inventory(&root, &opts)?;
    let inventory_facts =
        InventoryFacts::scanned_inventory(&inventory).with_policy_digest(policy_digest);
    let files = inventory.files;
    if evidence_validation.aborts_on_broken_local_evidence() {
        let evidence_source_tree_files =
            current_evidence_source_tree_files(&root, include_untracked);
        validate_evidence_references_for_source_tree(
            &root,
            &cfg,
            evidence_source_tree_files.as_ref(),
        )?;
    }
    let mut findings = Vec::new();
    // Durable scan-fact store (#2571): advisory on every failure path. The
    // store lives under target/ (never scanned) and is keyed by content
    // digest, so a stale or corrupt store can only cost a cold re-scan.
    let rust_scan = scan_rust_files_with_cache_mode(&root, &files, persistent_cache)?;
    let rust_files_skipped = rust_scan.files_skipped;
    let rust_files_with_parse_errors = rust_scan.files_with_parse_errors;
    findings.extend(rust_scan.findings);
    findings.extend(allow_files::scan_files_with_options(
        &files,
        &allow_files::FileScanOptions {
            generated: opts.generated.clone(),
            file_families: cfg.workspace.file_families.clone(),
            content_aware_generated: false,
        },
    ));
    let companion_findings = canonical_companion_findings(&root, &cfg, &files)?;
    extend_unique_findings(&mut findings, companion_findings);
    if let Some(kind) = kind_filter {
        let parsed = parse_kind_filter(kind)?;
        findings.retain(|f| parsed.matches_finding(f));
    }
    if let Some(provenance) = federation.active_provenance.clone() {
        for finding in &mut findings {
            finding.ledger = Some(provenance.clone());
        }
    }
    Ok((
        root,
        cfg,
        findings,
        inventory_facts
            .with_rust_files_considered(rust_scan.files_considered)
            .with_rust_files_skipped(rust_files_skipped)
            .with_rust_files_with_parse_errors(rust_files_with_parse_errors),
        federation,
    ))
}

/// Load the full policy but scan only the single file at `target_path` instead
/// of the entire source tree. Used by `why` (advisory, read-only) so a
/// one-finding question does not parse every file in the repository.
///
/// The matching layer decides whether this scoped finding can be evaluated
/// locally. Inventory is still collected for the target so the result never
/// conflates an untracked/ignored file with a missing finding. This remains
/// advisory and must not be reused by mutating commands.
pub(crate) fn load_world_for_path(
    explicit_root: Option<&Path>,
    config: Option<&Path>,
    require_config: bool,
    kind_filter: Option<&str>,
    include_untracked: bool,
    target_path: &Path,
) -> ScopedWorldLoadResult {
    let cwd = current_dir()?;
    let root = resolve_source_tree_root(explicit_root, cwd)?;
    let (policy_path, federation) = match evaluate_source_exception_policy(&root, config) {
        Ok(value) => value,
        Err(_err) if !require_config => {
            let (root, cfg, findings, facts, federation) = load_world_without_policy(
                &root,
                kind_filter,
                include_untracked,
                EvidenceValidationMode::ReportOnly,
                empty_federation_evaluation(PrecedenceTier::DiscoveryFallback),
            )?;
            return Ok((root, cfg, findings, facts, federation, None));
        }
        Err(err) => return Err(err),
    };
    let cfg = load_policy_at_path(policy_path, EvidenceValidationMode::ReportOnly)?;
    let inventory = inventory(
        &root,
        &inventory_options_with_tool_cache_ignore(InventoryOptions {
            ignored: cfg.workspace.ignored.clone(),
            generated: cfg.workspace.generated.clone(),
            include_untracked,
        }),
    )?;
    // Normalize the target path to repo-relative for the scan.
    let files = vec![normalize_to_repo_relative(&root, target_path)];
    let target = files.first().cloned().ok_or_else(|| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            "target source path was not prepared for scanning",
        )
    })?;
    if !inventory.files.iter().any(|path| path == &target) {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Inventory,
            format!(
                "target {} is not present in the source inventory; use --include-untracked if it is intentionally untracked",
                target_path.display()
            ),
        ));
    }
    let mut findings = Vec::new();
    let rust_scan = allow_rust::scan_rust_files(&root, &files)?;
    let target_scan = rust_scan.status_for(&target).cloned();
    // The scoped file list carries no Cargo.toml, so package-context
    // discovery inside the scanner finds no manifests and leaves
    // crate_name unset — while the full scan (check/add) sets it from the
    // inventory's manifests and finding_identity_key covers crate_name.
    // Apply the inventory's package contexts so the scoped and full scans
    // produce identical finding identities; otherwise every
    // why --plan → add --from-plan round trip for an in-package Rust
    // finding rejects with "finding identity changed" (#3581).
    let manifests = inventory
        .files
        .iter()
        .filter(|path| path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml"))
        .filter_map(|rel| {
            allow_core::read_text_file_capped(&root.join(rel))
                .ok()
                .map(|text| (rel.clone(), text))
        });
    let packages = allow_rust::source_package_contexts_from_sources(manifests);
    let mut rust_findings = rust_scan.findings;
    allow_rust::apply_source_package_context(&target, &packages, &mut rust_findings);
    findings.extend(rust_findings);
    findings.extend(allow_files::scan_files_with_options(
        &files,
        &allow_files::FileScanOptions {
            generated: cfg.workspace.generated.clone(),
            file_families: cfg.workspace.file_families.clone(),
            content_aware_generated: false,
        },
    ));
    let companion_findings = canonical_companion_findings(&root, &cfg, &files)?;
    extend_unique_findings(&mut findings, companion_findings);
    if let Some(kind) = kind_filter {
        let parsed = parse_kind_filter(kind)?;
        findings.retain(|f| parsed.matches_finding(f));
    }
    if let Some(provenance) = federation.active_provenance.clone() {
        for finding in &mut findings {
            finding.ledger = Some(provenance.clone());
        }
    }
    let inventory_facts = InventoryFacts::scanned_inventory(&inventory)
        .with_rust_files_considered(rust_scan.files_considered)
        .with_rust_files_skipped(rust_scan.files_skipped)
        .with_rust_files_with_parse_errors(rust_scan.files_with_parse_errors);
    Ok((
        root,
        cfg,
        findings,
        inventory_facts,
        federation,
        target_scan,
    ))
}

/// Explain why the target finding cannot safely use the one-file evaluator.
/// Policy locality comes from the matching layer; companion and federation
/// sources are world concerns and are kept here so `why` does not grow an
/// ad-hoc list of global semantics.
pub(crate) fn scoped_locality_reasons(
    cfg: &AllowConfig,
    finding: &Finding,
    federation: &FederationEvaluation,
) -> Vec<String> {
    let mut reasons = allow_match::scoped_locality_reasons(cfg, finding);

    if let Some(family) = finding.family.as_deref()
        && allow_core::is_repository_wide_family(family)
    {
        reasons.push(format!(
            "companion finding family `{family}` is derived from repository-wide context"
        ));
    }

    if !federation.divergences.is_empty() && finding.ledger.is_some() {
        reasons.push("federation mirror divergences affect the active finding context".to_string());
    }

    reasons.sort();
    reasons.dedup();
    reasons
}

/// Normalize an arbitrary path (absolute or repo-relative) to a repo-relative
/// PathBuf suitable for the scanner's file list.
pub(crate) fn normalize_to_repo_relative(root: &Path, path: &Path) -> PathBuf {
    // On Windows, resolve_source_tree_root returns a canonicalized path with
    // the \\?\ verbatim prefix, but the user-supplied --path is typically
    // non-verbatim. strip_prefix compares Component-by-Component and the
    // prefix types don't match, so it silently fails. Strip the verbatim
    // prefix from root first, then compare lexically (#2505).
    let root_stripped = crate::policy_config::strip_verbatim_prefix(root);
    let joined_path;
    let path_stripped = if path.is_absolute() {
        crate::policy_config::strip_verbatim_prefix(path)
    } else {
        joined_path = root.join(path);
        crate::policy_config::strip_verbatim_prefix(&joined_path)
    };
    if path_stripped.is_absolute() {
        path_stripped
            .strip_prefix(&root_stripped)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| {
                // Windows may spell the same temporary directory once with
                // an 8.3 short name and once with its long name. Canonicalize
                // both existing paths before falling back to string matching
                // so inventory membership does not depend on that spelling.
                if let (Ok(canonical_root), Ok(canonical_path)) =
                    (root_stripped.canonicalize(), path_stripped.canonicalize())
                {
                    let canonical_root =
                        crate::policy_config::strip_verbatim_prefix(&canonical_root);
                    let canonical_path =
                        crate::policy_config::strip_verbatim_prefix(&canonical_path);
                    if let Ok(relative) = canonical_path.strip_prefix(&canonical_root) {
                        return relative.to_path_buf();
                    }
                }
                // If strip_prefix still fails (e.g. path is under root but
                // canonicalization differs), try a string-based comparison.
                let path_str = path_stripped.to_string_lossy();
                let root_str = root_stripped.to_string_lossy();
                if let Some(rel) = path_str.strip_prefix(&*root_str) {
                    PathBuf::from(rel.trim_start_matches(['/', '\\']))
                } else {
                    path.to_path_buf()
                }
            })
    } else {
        path.to_path_buf()
    }
}

fn load_world_without_policy(
    root: &Path,
    kind_filter: Option<&str>,
    include_untracked: bool,
    evidence_validation: EvidenceValidationMode,
    federation: FederationEvaluation,
) -> CargoAllowResult<(
    PathBuf,
    AllowConfig,
    Vec<Finding>,
    InventoryFacts,
    FederationEvaluation,
)> {
    load_world_without_policy_and_cache(
        root,
        kind_filter,
        include_untracked,
        evidence_validation,
        federation,
        true,
    )
}

fn load_world_without_policy_and_cache(
    root: &Path,
    kind_filter: Option<&str>,
    include_untracked: bool,
    evidence_validation: EvidenceValidationMode,
    federation: FederationEvaluation,
    persistent_cache: bool,
) -> CargoAllowResult<(
    PathBuf,
    AllowConfig,
    Vec<Finding>,
    InventoryFacts,
    FederationEvaluation,
)> {
    let cfg = AllowConfig::empty();
    let opts = inventory_options_with_tool_cache_ignore(InventoryOptions {
        ignored: cfg.workspace.ignored.clone(),
        generated: cfg.workspace.generated.clone(),
        include_untracked,
    });
    let inventory = inventory(root, &opts)?;
    let inventory_facts = InventoryFacts::scanned_inventory(&inventory);
    let files = inventory.files;
    let mut findings = Vec::new();
    let rust_scan = scan_rust_files_with_cache_mode(root, &files, persistent_cache)?;
    let inventory_facts = inventory_facts
        .with_rust_files_considered(rust_scan.files_considered)
        .with_rust_files_skipped(rust_scan.files_skipped)
        .with_rust_files_with_parse_errors(rust_scan.files_with_parse_errors);
    findings.extend(rust_scan.findings);
    findings.extend(allow_files::scan_files_with_options(
        &files,
        &allow_files::FileScanOptions {
            generated: opts.generated.clone(),
            file_families: cfg.workspace.file_families.clone(),
            content_aware_generated: false,
        },
    ));
    let companion_findings = canonical_companion_findings(root, &cfg, &files)?;
    extend_unique_findings(&mut findings, companion_findings);
    if let Some(kind) = kind_filter {
        let parsed = parse_kind_filter(kind)?;
        findings.retain(|f| parsed.matches_finding(f));
    }
    // evidence_validation is intentionally unused here: this is the no-policy
    // fallback path where cfg = AllowConfig::empty(), so there are zero allow
    // entries to validate evidence against. The parameter exists for API
    // symmetry with load_world_with_evidence_mode and will be wired in if
    // a future caller needs evidence validation without a full policy (#2831).
    let _ = evidence_validation;
    Ok((
        root.to_path_buf(),
        cfg,
        findings,
        inventory_facts,
        federation,
    ))
}

fn empty_federation_evaluation(precedence: PrecedenceTier) -> FederationEvaluation {
    FederationEvaluation {
        federation_version: allow_policy::federation::FEDERATION_VERSION,
        precedence_applied: precedence,
        active_provenance: None,
        ledger_contributors: Vec::new(),
        divergences: Vec::new(),
    }
}

pub(crate) fn default_federation_evaluation() -> FederationEvaluation {
    empty_federation_evaluation(PrecedenceTier::DiscoveryFallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{AllowEntry, FindingKind, Lifecycle, Selector};
    use allow_policy::render_policy;
    use std::fs;
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn persistent_cache_off_no_policy_preserves_result_without_creating_store() -> Result<(), String>
    {
        let root = fixture_dir();
        fs::create_dir_all(root.join("src")).map_err(|err| format!("src dir: {err}"))?;
        fs::write(root.join("src/lib.rs"), "fn disabled() {}\n")
            .map_err(|err| format!("source: {err}"))?;
        git(root.as_path(), &["init"]);
        let enabled = load_world_without_policy_and_cache(
            &root,
            None,
            true,
            EvidenceValidationMode::ReportOnly,
            empty_federation_evaluation(PrecedenceTier::DiscoveryFallback),
            true,
        )
        .map_err(|err| format!("enabled world: {err}"))?;
        let cache_dir = allow_rust::ScanCacheStore::default_dir(&root);
        let cache_file = cache_dir.join("scan-cache.v2.bin");
        let sentinel = b"persistent-cache-off-sentinel";
        fs::write(&cache_file, sentinel).map_err(|err| format!("seed cache: {err}"))?;
        let lock_file = cache_dir.join("scan-cache.v2.lock");
        if lock_file.exists() {
            fs::remove_file(&lock_file).map_err(|err| format!("remove lock: {err}"))?;
        }
        for entry in fs::read_dir(&cache_dir).map_err(|err| format!("list cache: {err}"))? {
            let entry = entry.map_err(|err| format!("read cache entry: {err}"))?;
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with("scan-cache.v2.bin.tmp-")
            {
                fs::remove_file(entry.path()).map_err(|err| format!("remove temp: {err}"))?;
            }
        }
        let disabled = load_world_without_policy_and_cache(
            &root,
            None,
            true,
            EvidenceValidationMode::ReportOnly,
            empty_federation_evaluation(PrecedenceTier::DiscoveryFallback),
            false,
        )
        .map_err(|err| format!("disabled world: {err}"))?;
        if enabled.2 != disabled.2 || enabled.3 != disabled.3 {
            return Err("disabled and enabled no-policy results differ".to_string());
        }
        if fs::read(&cache_file).map_err(|err| format!("read cache: {err}"))? != sentinel {
            return Err("disabled mode modified the cache".to_string());
        }
        if lock_file.exists()
            || fs::read_dir(&cache_dir)
                .map_err(|err| format!("list cache: {err}"))?
                .filter_map(Result::ok)
                .any(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
        {
            return Err("disabled mode touched lock/temp cache files".to_string());
        }
        fs::remove_dir_all(root).map_err(|err| format!("remove fixture dir: {err}"))?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn persistent_cache_admission_failure_falls_back_without_outside_write() -> Result<(), String> {
        use std::os::unix::fs::symlink;

        let root = fixture_dir();
        fs::create_dir_all(root.join("src")).map_err(|err| format!("src dir: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "fn fallback(value: Result<(), ()>) { value.unwrap(); }\n",
        )
        .map_err(|err| format!("source: {err}"))?;
        let outside = root.join("outside-target");
        fs::create_dir_all(&outside).map_err(|err| format!("outside dir: {err}"))?;
        symlink(&outside, root.join("target")).map_err(|err| format!("target alias: {err}"))?;
        let files = vec![PathBuf::from("src/lib.rs")];

        let persistent = scan_rust_files_with_cache_mode(&root, &files, true)
            .map_err(|err| format!("persistent fallback scan: {err}"))?;
        let ordinary = scan_rust_files_with_cache_mode(&root, &files, false)
            .map_err(|err| format!("ordinary scan: {err}"))?;
        if persistent.findings != ordinary.findings
            || persistent.file_statuses != ordinary.file_statuses
            || persistent.files_considered != ordinary.files_considered
            || persistent.files_skipped != ordinary.files_skipped
            || persistent.files_with_parse_errors != ordinary.files_with_parse_errors
        {
            return Err("cache admission failure changed scan semantics".to_string());
        }
        if outside.join("cargo-allow/cache/scan-cache.v2.bin").exists() {
            return Err("cache admission failure wrote through in-root alias".to_string());
        }
        fs::remove_file(root.join("target")).map_err(|err| format!("remove alias: {err}"))?;
        fs::remove_dir_all(root).map_err(|err| format!("remove fixture: {err}"))?;
        Ok(())
    }

    #[test]
    fn persistent_cache_off_policy_preserves_result_without_creating_store() -> Result<(), String> {
        let root = fixture_dir();
        fs::create_dir_all(root.join("src")).map_err(|err| format!("src dir: {err}"))?;
        fs::create_dir_all(root.join("policy")).map_err(|err| format!("policy dir: {err}"))?;
        fs::write(root.join("src/lib.rs"), "fn disabled() {}\n")
            .map_err(|err| format!("source: {err}"))?;
        fs::write(
            root.join("policy/allow.toml"),
            "schema_version = 1\n\n[workspace]\nignored = []\ngenerated = []\n",
        )
        .map_err(|err| format!("policy: {err}"))?;
        git(root.as_path(), &["init"]);
        let enabled = load_world_with_evidence_mode_and_cache(
            Some(&root),
            Some(Path::new("policy/allow.toml")),
            true,
            None,
            true,
            EvidenceValidationMode::ReportOnly,
            true,
        )
        .map_err(|err| format!("enabled world: {err}"))?;
        let cache_dir = allow_rust::ScanCacheStore::default_dir(&root);
        let cache_file = cache_dir.join("scan-cache.v2.bin");
        let sentinel = b"persistent-cache-off-sentinel";
        fs::write(&cache_file, sentinel).map_err(|err| format!("seed cache: {err}"))?;
        let lock_file = cache_dir.join("scan-cache.v2.lock");
        if lock_file.exists() {
            fs::remove_file(&lock_file).map_err(|err| format!("remove lock: {err}"))?;
        }
        for entry in fs::read_dir(&cache_dir).map_err(|err| format!("list cache: {err}"))? {
            let entry = entry.map_err(|err| format!("read cache entry: {err}"))?;
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with("scan-cache.v2.bin.tmp-")
            {
                fs::remove_file(entry.path()).map_err(|err| format!("remove temp: {err}"))?;
            }
        }
        let disabled = load_world_with_evidence_mode_and_cache(
            Some(&root),
            Some(Path::new("policy/allow.toml")),
            true,
            None,
            true,
            EvidenceValidationMode::ReportOnly,
            false,
        )
        .map_err(|err| format!("disabled world: {err}"))?;
        if enabled.2 != disabled.2 || enabled.3 != disabled.3 {
            return Err("disabled and enabled policy results differ".to_string());
        }
        if fs::read(&cache_file).map_err(|err| format!("read cache: {err}"))? != sentinel {
            return Err("disabled mode modified the cache".to_string());
        }
        if lock_file.exists()
            || fs::read_dir(&cache_dir)
                .map_err(|err| format!("list cache: {err}"))?
                .filter_map(Result::ok)
                .any(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
        {
            return Err("disabled mode touched lock/temp cache files".to_string());
        }
        fs::remove_dir_all(root).map_err(|err| format!("remove fixture dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn no_policy_world_cache_exclusion_has_cold_warm_parity() -> Result<(), String> {
        let root = fixture_dir();
        fs::create_dir_all(root.join("src")).map_err(|err| format!("src dir: {err}"))?;
        fs::write(root.join("src/lib.rs"), "fn cached() {}\n")
            .map_err(|err| format!("source: {err}"))?;
        git(root.as_path(), &["init"]);
        let cache_dir = root.join("target/cargo-allow/cache");
        let cache_file = cache_dir.join("scan-cache.v2.bin");
        if cache_file.exists() {
            return Err("cache unexpectedly existed before cold scan".to_string());
        }
        let cold = load_world_without_policy(
            &root,
            None,
            true,
            EvidenceValidationMode::ReportOnly,
            empty_federation_evaluation(PrecedenceTier::DiscoveryFallback),
        )
        .map_err(|err| format!("cold world: {err}"))?;
        if !cache_file.exists()
            || cold.3.completeness != InventoryCompleteness::Scoped
            || cold.3.files_scanned != Some(1)
            || format!("{:?}", cold.2).contains("target/cargo-allow/cache")
        {
            return Err(
                "cold scan did not produce the expected scoped source-only result".to_string(),
            );
        }
        let warm = load_world_without_policy(
            &root,
            None,
            true,
            EvidenceValidationMode::ReportOnly,
            empty_federation_evaluation(PrecedenceTier::DiscoveryFallback),
        )
        .map_err(|err| format!("warm world: {err}"))?;
        if warm.3.completeness != InventoryCompleteness::Scoped
            || warm.3.files_scanned != Some(1)
            || format!("{:?}", warm.2).contains("target/cargo-allow/cache")
            || cold.2 != warm.2
            || cold.3.completeness != warm.3.completeness
            || cold.3.files_scanned != warm.3.files_scanned
            || cold.3.rust_files_considered != warm.3.rust_files_considered
            || cold.3.rust_files_skipped != warm.3.rust_files_skipped
            || cold.3.rust_files_with_parse_errors != 0
            || warm.3.rust_files_with_parse_errors != 0
            || cold.3.rust_files_considered != 1
            || warm.3.rust_files_considered != 1
            || cold.3.rust_files_skipped != 0
            || warm.3.rust_files_skipped != 0
        {
            return Err("cold and warm source-only results differ".to_string());
        }
        fs::remove_dir_all(root).map_err(|err| format!("remove fixture: {err}"))?;
        Ok(())
    }

    #[test]
    fn load_world_abort_rejects_untracked_local_evidence_by_default() {
        let root = fixture_dir();
        write_policy_with_untracked_evidence(&root);

        let err = load_world(
            Some(&root),
            Some(Path::new("policy/allow.toml")),
            true,
            None,
            false,
        )
        .expect_err("default source-tree inventory should reject untracked local evidence");

        assert!(
            err.to_string()
                .contains("not in the default source-tree inventory"),
            "diagnostic should explain source-tree evidence boundary: {err}"
        );
        fs::remove_dir_all(root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
    }

    #[test]
    fn load_world_abort_include_untracked_accepts_untracked_local_evidence() {
        let root = fixture_dir();
        write_policy_with_untracked_evidence(&root);

        let result = load_world(
            Some(&root),
            Some(Path::new("policy/allow.toml")),
            true,
            None,
            true,
        );

        result.unwrap_or_else(|err| {
            std::panic::panic_any(format!(
                "include-untracked inventory should accept untracked local evidence: {err}"
            ))
        });
        fs::remove_dir_all(root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
    }

    #[test]
    fn load_world_abort_rejects_untracked_local_link_by_default() {
        let root = fixture_dir();
        write_policy_with_untracked_link(&root);

        let err = load_world(
            Some(&root),
            Some(Path::new("policy/allow.toml")),
            true,
            None,
            false,
        )
        .expect_err("default source-tree inventory should reject untracked local links");

        assert!(
            err.to_string()
                .contains("not in the default source-tree inventory"),
            "diagnostic should explain source-tree link boundary: {err}"
        );
        assert!(
            err.to_string().contains("allow-0001 link"),
            "diagnostic should identify the broken traceability link: {err}"
        );
        fs::remove_dir_all(root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
    }

    #[test]
    fn load_world_abort_include_untracked_accepts_untracked_local_link() {
        let root = fixture_dir();
        write_policy_with_untracked_link(&root);

        let result = load_world(
            Some(&root),
            Some(Path::new("policy/allow.toml")),
            true,
            None,
            true,
        );

        result.unwrap_or_else(|err| {
            std::panic::panic_any(format!(
                "include-untracked inventory should accept untracked local links: {err}"
            ))
        });
        fs::remove_dir_all(root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
    }

    #[test]
    fn staged_world_rejects_worktree_derived_companion_families() {
        let root = fixture_dir();
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(AllowEntry {
            id: "allow-0001".to_string(),
            kind: FindingKind::PolicyException,
            family: Some("github_workflow".to_string()),
            path: Some(PathBuf::from(".github/workflows/ci.yml")),
            glob: None,
            owner: "ci".to_string(),
            classification: "github_workflow".to_string(),
            reason: "Retained workflow fixture.".to_string(),
            evidence: vec!["legacy-policy:test".to_string()],
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle {
                created: Some("2026-05-26".to_string()),
                review_after: Some("2026-11-01".to_string()),
                expires: None,
            },
            selector: Selector {
                ast_kind: Some("github_workflow".to_string()),
                symbol: Some(".github/workflows/ci.yml".to_string()),
                glob: Some(".github/workflows/ci.yml".to_string()),
                ..Selector::default()
            },
            last_seen: None,
        });
        fs::create_dir_all(root.join("policy"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
        fs::write(root.join("policy/allow.toml"), render_policy(&cfg))
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy write: {err}")));
        fs::write(root.join("candidate.rs"), "fn candidate() {}\n")
            .unwrap_or_else(|err| std::panic::panic_any(format!("source write: {err}")));
        git(root.as_path(), &["init"]);
        git(
            root.as_path(),
            &["config", "user.email", "cargo-allow@example.invalid"],
        );
        git(root.as_path(), &["config", "user.name", "cargo-allow test"]);
        git(root.as_path(), &["add", "--all"]);
        git(root.as_path(), &["commit", "-m", "staged companion policy"]);

        let result = load_staged_world(Some(&root), Some(Path::new("policy/allow.toml")), None);
        let error = result
            .err()
            .unwrap_or_else(|| std::panic::panic_any("unsupported family should fail closed"));
        assert_eq!(error.kind(), allow_core::CargoAllowErrorKind::Unsupported);
        assert!(error.to_string().contains("github_workflow"));
        fs::remove_dir_all(root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
    }

    #[test]
    fn staged_text_reading_rejects_missing_and_non_utf8_candidates() {
        let root = fixture_dir();
        fs::create_dir_all(root.join("policy"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
        fs::write(
            root.join("policy/allow.toml"),
            "schema_version = 1\n\n[workspace]\nignored = []\ngenerated = []\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy write: {err}")));
        git(root.as_path(), &["init"]);
        git(
            root.as_path(),
            &["config", "user.email", "cargo-allow@example.invalid"],
        );
        git(root.as_path(), &["config", "user.name", "cargo-allow test"]);
        git(root.as_path(), &["add", "--all"]);
        git(root.as_path(), &["commit", "-m", "staged text base"]);
        fs::write(root.join("invalid.rs"), [0xff_u8, 0xfe_u8])
            .unwrap_or_else(|err| std::panic::panic_any(format!("invalid source write: {err}")));
        fs::write(
            root.join("oversized.rs"),
            vec![b'x'; (SOURCE_FILE_READ_MAX_BYTES as usize) + 1],
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("oversized source write: {err}")));
        git(root.as_path(), &["add", "--", "invalid.rs", "oversized.rs"]);

        let snapshot = crate::command_support::snapshot_result(staged_repository_snapshot(&root))
            .unwrap_or_else(|err| std::panic::panic_any(format!("staged snapshot: {err}")));
        let mut partial_snapshot = snapshot.clone();
        partial_snapshot.completeness =
            effortless_repo_snapshot::StagedSnapshotCompleteness::Partial;
        let partial_inventory = staged_inventory(&partial_snapshot, &InventoryOptions::default());
        assert_eq!(
            partial_inventory.completeness,
            InventoryCompleteness::Partial
        );
        let mut federation_snapshot = snapshot.clone();
        if let Some(entry) = federation_snapshot.entries.first_mut() {
            entry.path = Some(PathBuf::from(".allow/config.toml"));
        }
        let federation_error = reject_unsupported_staged_federation(
            &federation_snapshot,
            &default_federation_evaluation(),
        )
        .err()
        .unwrap_or_else(|| std::panic::panic_any("federation metadata should fail closed"));
        assert_eq!(
            federation_error.kind(),
            allow_core::CargoAllowErrorKind::Unsupported
        );
        let missing = read_staged_text(&snapshot, Path::new("missing.rs"))
            .err()
            .unwrap_or_else(|| std::panic::panic_any("missing staged source should fail"));
        assert_eq!(missing.kind(), allow_core::CargoAllowErrorKind::Inventory);
        let invalid = read_staged_text(&snapshot, Path::new("invalid.rs"))
            .err()
            .unwrap_or_else(|| std::panic::panic_any("invalid UTF-8 source should fail"));
        assert_eq!(invalid.kind(), allow_core::CargoAllowErrorKind::Scan);
        assert!(invalid.to_string().contains("not valid UTF-8"));
        let oversized = read_staged_text(&snapshot, Path::new("oversized.rs"))
            .err()
            .unwrap_or_else(|| std::panic::panic_any("oversized source should fail"));
        assert_eq!(oversized.kind(), allow_core::CargoAllowErrorKind::Scan);
        assert!(oversized.to_string().contains("exceeds"));
        fs::remove_dir_all(root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
    }

    fn write_policy_with_untracked_evidence(root: &Path) {
        fs::create_dir_all(root.join("policy"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
        fs::create_dir_all(root.join("docs"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("docs dir: {err}")));
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(allow_entry_with_untracked_evidence());
        fs::write(root.join("policy/allow.toml"), render_policy(&cfg))
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy write: {err}")));

        git(root, &["init"]);
        git(
            root,
            &["config", "user.email", "cargo-allow@example.invalid"],
        );
        git(root, &["config", "user.name", "cargo-allow test"]);
        git(root, &["add", "policy/allow.toml"]);
        git(root, &["commit", "-m", "base policy"]);

        fs::write(root.join("docs/evidence.md"), "review notes")
            .unwrap_or_else(|err| std::panic::panic_any(format!("evidence write: {err}")));
    }

    fn write_policy_with_untracked_link(root: &Path) {
        fs::create_dir_all(root.join("policy"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
        fs::create_dir_all(root.join("docs"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("docs dir: {err}")));
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(allow_entry_with_untracked_link());
        fs::write(root.join("policy/allow.toml"), render_policy(&cfg))
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy write: {err}")));

        git(root, &["init"]);
        git(
            root,
            &["config", "user.email", "cargo-allow@example.invalid"],
        );
        git(root, &["config", "user.name", "cargo-allow test"]);
        git(root, &["add", "policy/allow.toml"]);
        git(root, &["commit", "-m", "base policy"]);

        fs::write(root.join("docs/rationale.md"), "review notes")
            .unwrap_or_else(|err| std::panic::panic_any(format!("link write: {err}")));
    }

    fn allow_entry_with_untracked_evidence() -> AllowEntry {
        AllowEntry {
            id: "allow-0001".to_string(),
            kind: FindingKind::NonRustFile,
            family: None,
            path: Some(PathBuf::from("docs/source.md")),
            glob: None,
            owner: "docs".to_string(),
            classification: "reviewed_exception".to_string(),
            reason: "Fixture exception for source-tree evidence validation.".to_string(),
            evidence: vec!["doc:docs/evidence.md".to_string()],
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle {
                created: Some("2026-05-31".to_string()),
                review_after: Some("2026-08-31".to_string()),
                expires: None,
            },
            selector: Selector {
                ast_kind: Some("tracked_file".to_string()),
                ..Selector::default()
            },
            last_seen: None,
        }
    }

    fn allow_entry_with_untracked_link() -> AllowEntry {
        let mut entry = allow_entry_with_untracked_evidence();
        entry.evidence = vec!["test:allow_entry_with_untracked_link".to_string()];
        entry.links = vec!["doc:docs/rationale.md".to_string()];
        entry
    }

    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
        if !output.status.success() {
            std::panic::panic_any(format!(
                "git {args:?} failed: stdout=`{}` stderr=`{}`",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    fn fixture_dir() -> PathBuf {
        let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "cargo-allow-world-{}-{stamp}-{id}",
            std::process::id()
        ));
        if dir.exists() {
            fs::remove_dir_all(&dir)
                .unwrap_or_else(|err| std::panic::panic_any(format!("reset fixture dir: {err}")));
        }
        fs::create_dir_all(&dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture dir: {err}")));
        std::fs::canonicalize(&dir).unwrap_or(dir)
    }

    /// The scoped loader (`why`/`why --plan`) and the full loader
    /// (`check`/`add --from-plan`) must produce identical finding
    /// identities for an in-package Rust file. Package-context discovery
    /// reads manifests from the scan set; a scoped scan of one source file
    /// carries no manifest, so without re-applying the inventory's package
    /// contexts the scoped finding loses `crate_name` and every
    /// why --plan → add --from-plan round trip for an in-package finding
    /// rejects with "finding identity changed" (#3581).
    #[test]
    fn scoped_and_full_scans_agree_on_in_package_finding_identity() {
        let root = fixture_dir();
        let cleanup = |dir: &Path| {
            fs::remove_dir_all(dir)
                .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture: {err}")));
        };
        fs::create_dir_all(root.join("pkg/src"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("pkg dir: {err}")));
        fs::write(
            root.join("pkg/Cargo.toml"),
            "[package]\nname = \"probe-pkg\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("pkg manifest: {err}")));
        fs::write(
            root.join("pkg/src/lib.rs"),
            "pub fn probe() {\n    let value: Option<u8> = None;\n    value.unwrap();\n}\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("pkg source: {err}")));
        fs::create_dir_all(root.join("policy"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
        fs::write(
            root.join("policy/allow.toml"),
            "schema_version = 1\npolicy = \"cargo-allow\"\n\n[workspace]\nignored = []\ngenerated = []\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy write: {err}")));
        git(root.as_path(), &["init"]);
        git(
            root.as_path(),
            &["config", "user.email", "cargo-allow@example.invalid"],
        );
        git(root.as_path(), &["config", "user.name", "cargo-allow test"]);
        git(root.as_path(), &["add", "--all"]);
        git(root.as_path(), &["commit", "-m", "scoped identity fixture"]);

        let target = root.join("pkg/src/lib.rs");
        let full = load_world(Some(&root), None, true, Some("panic"), false)
            .unwrap_or_else(|err| std::panic::panic_any(format!("full load: {err}")));
        let scoped = load_world_for_path(Some(&root), None, true, Some("panic"), false, &target)
            .unwrap_or_else(|err| std::panic::panic_any(format!("scoped load: {err}")));

        let select = |findings: &[allow_core::Finding]| {
            findings
                .iter()
                .find(|finding| {
                    finding.path.ends_with("pkg/src/lib.rs")
                        && finding.identity.callee.as_deref() == Some("unwrap")
                })
                .cloned()
                .unwrap_or_else(|| {
                    std::panic::panic_any("expected the probe unwrap finding in pkg/src/lib.rs")
                })
        };
        let full_finding = select(&full.2);
        let scoped_finding = select(&scoped.2);

        assert_eq!(
            full_finding.identity.crate_name.as_deref(),
            Some("probe-pkg"),
            "full scan must resolve the package context for an in-package file"
        );
        assert_eq!(
            scoped_finding.identity.crate_name.as_deref(),
            Some("probe-pkg"),
            "scoped scan must carry the same package context as the full scan"
        );
        assert_eq!(
            allow_core::finding_identity_key(&full_finding),
            allow_core::finding_identity_key(&scoped_finding),
            "scoped and full scans must agree on the finding identity key"
        );
        cleanup(&root);
    }
}
