use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult, Finding};
use allow_inventory::{InventoryOptions, inventory, resolve_source_tree_root};
use allow_policy::federation::{
    FederationEvaluation, PrecedenceTier, evaluate_source_exception_policy,
};
use std::env;
use std::path::{Path, PathBuf};

use crate::{
    EvidenceValidationMode, InventoryFacts, canonical_companion_findings,
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
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
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
    let inventory_facts = InventoryFacts::scanned(inventory.source, inventory.files.len());
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
    findings.extend(allow_rust::scan_rust_files(&root, &files)?);
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
    Ok((root, cfg, findings, inventory_facts, federation))
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
    let inventory_facts = InventoryFacts::scanned(inventory.source, inventory.files.len());
    let files = inventory.files;
    let mut findings = Vec::new();
    findings.extend(allow_rust::scan_rust_files(root, &files)?);
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
