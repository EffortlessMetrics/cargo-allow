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

use allow_diff::{
    StagedEntryKind, StagedPathRead, StagedRepositorySnapshot, read_staged_path,
    staged_repository_snapshot,
};

thread_local! {
    static SCAN_CACHE: RefCell<allow_rust::ScanCache> = RefCell::new(allow_rust::ScanCache::new());
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
    let snapshot = staged_repository_snapshot(&root)?;
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
    let cfg = allow_policy::parse_policy_with_reportable_evidence_at(&policy_path, &policy_text)?;
    reject_unsupported_staged_companion_sensors(&cfg)?;
    let options = InventoryOptions {
        ignored: cfg.workspace.ignored.clone(),
        generated: cfg.workspace.generated.clone(),
        include_untracked: false,
    };
    let source_identity = snapshot.identity.semantic_hash.clone();
    let inventory = staged_inventory(&snapshot, &options);
    let inventory_facts = InventoryFacts::scanned_inventory(&inventory);
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
    let final_snapshot = staged_repository_snapshot(&root)?;
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
    let completeness = if snapshot.completeness == allow_diff::StagedSnapshotCompleteness::Partial {
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
    let bytes = match read_staged_path(snapshot, path)? {
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

pub(crate) fn load_world(
    explicit_root: Option<&Path>,
    config: Option<&Path>,
    require_config: bool,
    kind_filter: Option<&str>,
    include_untracked: bool,
) -> CargoAllowResult<(
    PathBuf,
    AllowConfig,
    Vec<Finding>,
    InventoryFacts,
    FederationEvaluation,
    Option<allow_rust::RustFileScanOutcome>,
)> {
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
) -> CargoAllowResult<(
    PathBuf,
    AllowConfig,
    Vec<Finding>,
    InventoryFacts,
    FederationEvaluation,
)> {
    let cwd = current_dir()?;
    let root = resolve_source_tree_root(explicit_root, cwd)?;
    let (policy_path, federation) = match evaluate_source_exception_policy(&root, config) {
        Ok(value) => value,
        Err(_err) if !require_config => {
            return load_world_without_policy(
                &root,
                kind_filter,
                include_untracked,
                evidence_validation,
                empty_federation_evaluation(PrecedenceTier::DiscoveryFallback),
            );
        }
        Err(err) => return Err(err),
    };
    let cfg = load_policy_at_path(policy_path, evidence_validation)?;
    let opts = InventoryOptions {
        ignored: cfg.workspace.ignored.clone(),
        generated: cfg.workspace.generated.clone(),
        include_untracked,
    };
    let inventory = inventory(&root, &opts)?;
    let inventory_facts = InventoryFacts::scanned_inventory(&inventory);
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
    let rust_scan = SCAN_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        allow_rust::scan_rust_files_cached(&root, &files, &mut cache)
    })?;
    let rust_files_skipped = rust_scan.files_skipped;
    let rust_files_with_parse_errors = rust_scan.files_with_parse_errors;
    findings.extend(rust_scan.findings);
    findings.extend(allow_files::scan_files_with_options(
        &files,
        &allow_files::FileScanOptions {
            generated: opts.generated.clone(),
            file_families: cfg.workspace.file_families.clone(),
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
) -> CargoAllowResult<(
    PathBuf,
    AllowConfig,
    Vec<Finding>,
    InventoryFacts,
    FederationEvaluation,
)> {
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
        &InventoryOptions {
            ignored: cfg.workspace.ignored.clone(),
            generated: cfg.workspace.generated.clone(),
            include_untracked,
        },
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
    findings.extend(rust_scan.findings);
    findings.extend(allow_files::scan_files_with_options(
        &files,
        &allow_files::FileScanOptions {
            generated: cfg.workspace.generated.clone(),
            file_families: cfg.workspace.file_families.clone(),
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
        .with_rust_files_skipped(rust_scan.files_skipped)
        .with_rust_files_with_parse_errors(rust_scan.files_with_parse_errors);
    Ok((
        root,
        cfg,
        findings,
        inventory_facts,
        federation,
        rust_scan.status_for(&target).cloned(),
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
    let cfg = AllowConfig::empty();
    let opts = InventoryOptions {
        ignored: cfg.workspace.ignored.clone(),
        generated: cfg.workspace.generated.clone(),
        include_untracked,
    };
    let inventory = inventory(root, &opts)?;
    let inventory_facts = InventoryFacts::scanned_inventory(&inventory);
    let files = inventory.files;
    let mut findings = Vec::new();
    let rust_scan = allow_rust::scan_rust_files(root, &files)?;
    findings.extend(rust_scan.findings);
    findings.extend(allow_files::scan_files_with_options(
        &files,
        &allow_files::FileScanOptions {
            generated: opts.generated.clone(),
            file_families: cfg.workspace.file_families.clone(),
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

        let snapshot = staged_repository_snapshot(&root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("staged snapshot: {err}")));
        let mut partial_snapshot = snapshot.clone();
        partial_snapshot.completeness = allow_diff::StagedSnapshotCompleteness::Partial;
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
        dir
    }
}
