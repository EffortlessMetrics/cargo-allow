use allow_core::{AllowConfig, CargoAllowResult, Finding};
use allow_inventory::{InventoryOptions, InventorySource, inventory, resolve_source_tree_root};
use allow_policy::federation::{
    FederationEvaluation, PrecedenceTier, evaluate_source_exception_policy,
};
use std::cell::RefCell;
use std::path::{Path, PathBuf};

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
/// Correctness caveat: the `evaluate` call downstream sees only this file's
/// findings, so `occurrence_limit` counting and cross-file drift re-anchoring
/// reflect a partial view. This is acceptable for `why` (advisory) but NOT for
/// `add` (which mutates the ledger).
pub(crate) fn load_world_for_path(
    explicit_root: Option<&Path>,
    config: Option<&Path>,
    require_config: bool,
    kind_filter: Option<&str>,
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
            return load_world_without_policy(
                &root,
                kind_filter,
                false,
                EvidenceValidationMode::ReportOnly,
                empty_federation_evaluation(PrecedenceTier::DiscoveryFallback),
            );
        }
        Err(err) => return Err(err),
    };
    let cfg = load_policy_at_path(policy_path, EvidenceValidationMode::ReportOnly)?;
    // Normalize the target path to repo-relative for the scan.
    let files = vec![normalize_to_repo_relative(&root, target_path)];
    let mut findings = Vec::new();
    let rust_scan = allow_rust::scan_rust_files(&root, &files)?;
    findings.extend(rust_scan.findings);
    findings.extend(allow_files::scan_files_with_options(
        &files,
        &allow_files::FileScanOptions {
            generated: cfg.workspace.generated.clone(),
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
    // The fast path doesn't run inventory(), so we can't claim a specific
    // inventory source. Use source_only with FilesystemFallback to avoid
    // asserting git-tracking for a file we haven't checked (#2506).
    let inventory_facts = InventoryFacts::source_only(InventorySource::FilesystemFallback);
    Ok((root, cfg, findings, inventory_facts, federation))
}

/// Normalize an arbitrary path (absolute or repo-relative) to a repo-relative
/// PathBuf suitable for the scanner's file list.
fn normalize_to_repo_relative(root: &Path, path: &Path) -> PathBuf {
    // On Windows, resolve_source_tree_root returns a canonicalized path with
    // the \\?\ verbatim prefix, but the user-supplied --path is typically
    // non-verbatim. strip_prefix compares Component-by-Component and the
    // prefix types don't match, so it silently fails. Strip the verbatim
    // prefix from root first, then compare lexically (#2505).
    let root_stripped = crate::policy_config::strip_verbatim_prefix(root);
    let path_stripped = crate::policy_config::strip_verbatim_prefix(path);
    if path_stripped.is_absolute() {
        path_stripped
            .strip_prefix(&root_stripped)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| {
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
        },
    ));
    let companion_findings = canonical_companion_findings(root, &cfg, &files)?;
    extend_unique_findings(&mut findings, companion_findings);
    if let Some(kind) = kind_filter {
        let parsed = parse_kind_filter(kind)?;
        findings.retain(|f| parsed.matches_finding(f));
    }
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
