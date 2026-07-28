use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};
use allow_policy::{
    PrecedenceTier, SkippedPolicyCandidate, discover_config, evaluate_source_exception_policy,
    load_policy, load_policy_with_reportable_evidence,
};
use std::path::{Path, PathBuf};

/// Centralized "no policy config found" error constructor (#2824).
/// Used by 18+ sites that previously copy-pasted this message.
pub(crate) fn missing_policy_config_error() -> CargoAllowError {
    CargoAllowError::new("no policy config found; run `cargo-allow init` or pass --config")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfigDiscovery {
    pub path: Option<PathBuf>,
    pub skipped: Vec<SkippedPolicyCandidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EvidenceValidationMode {
    Abort,
    ReportOnly,
}

impl EvidenceValidationMode {
    pub(crate) fn aborts_on_broken_local_evidence(self) -> bool {
        matches!(self, Self::Abort)
    }

    fn permits_reportable_policy_evidence(self) -> bool {
        matches!(self, Self::ReportOnly)
    }
}

#[cfg(test)]
pub(crate) fn load_config_required_with_evidence_mode(
    root: &Path,
    config: Option<&Path>,
    evidence_validation: EvidenceValidationMode,
) -> CargoAllowResult<AllowConfig> {
    let discovery = discover_config_path(root, config);
    let path = discovery
        .path
        .ok_or_else(|| missing_config_error(&discovery.skipped))?;
    load_policy_for_root(path, evidence_validation)
}

#[cfg(test)]
pub(crate) fn load_config_optional_with_evidence_mode(
    root: &Path,
    config: Option<&Path>,
    evidence_validation: EvidenceValidationMode,
) -> CargoAllowResult<Option<AllowConfig>> {
    let discovery = discover_config_path(root, config);
    match discovery.path {
        Some(path) => Ok(Some(load_policy_for_root(path, evidence_validation)?)),
        None if discovery.skipped.is_empty() => Ok(None),
        None => Err(missing_config_error(&discovery.skipped)),
    }
}

fn load_policy_for_root(
    path: PathBuf,
    evidence_validation: EvidenceValidationMode,
) -> CargoAllowResult<AllowConfig> {
    if evidence_validation.permits_reportable_policy_evidence() {
        load_policy_with_reportable_evidence(path)
    } else {
        load_policy(path)
    }
}

pub(crate) fn load_policy_at_path(
    path: PathBuf,
    evidence_validation: EvidenceValidationMode,
) -> CargoAllowResult<AllowConfig> {
    load_policy_for_root(path, evidence_validation)
}

pub(crate) fn config_path(root: &Path, config: Option<&Path>) -> Option<PathBuf> {
    discover_config_path(root, config).path
}

pub(crate) fn discover_config_path(root: &Path, config: Option<&Path>) -> ConfigDiscovery {
    match evaluate_source_exception_policy(root, config) {
        Ok((path, evaluation)) => {
            let skipped = if evaluation.precedence_applied == PrecedenceTier::DiscoveryFallback {
                discover_config(root).skipped
            } else {
                Vec::new()
            };
            ConfigDiscovery {
                path: Some(path),
                skipped,
            }
        }
        Err(_) => {
            let discovery = discover_config(root);
            ConfigDiscovery {
                path: discovery.selected,
                skipped: discovery.skipped,
            }
        }
    }
}

fn missing_config_error(skipped: &[SkippedPolicyCandidate]) -> CargoAllowError {
    if skipped.is_empty() {
        return missing_policy_config_error();
    }
    let details = skipped
        .iter()
        .map(|candidate| format!("{} ({})", candidate.path.display(), candidate.reason))
        .collect::<Vec<_>>()
        .join("; ");
    CargoAllowError::new(format!(
        "no cargo-allow policy config found; skipped {} foreign-dialect candidate(s): {}; run `cargo-allow init` or pass --config",
        skipped.len(),
        details
    ))
}

pub(crate) fn root_relative_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

/// Assert that a write/output path stays within the source-tree root.
///
/// Rejects paths that escape via `..` traversal or absolute paths outside the
/// root (#1791). The check is purely lexical: it resolves the path against the
/// root and normalizes `.`/`..` components without touching the filesystem, so
/// it works for output paths whose parent directories do not exist yet (the
/// common case for a fresh `--receipt`/`--output` under `target/`). This avoids
/// `canonicalize`-based checks, which fail on missing paths and behave
/// inconsistently across platforms in the presence of symlinks.
pub(crate) fn assert_path_within_root(root: &Path, path: &Path) -> CargoAllowResult<()> {
    repo_edit::assert_path_within_root(root, path)
}

pub(crate) fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    repo_edit::strip_verbatim_prefix(path)
}

/// Portable repository-relative path for mutation apply targets.
///
/// Prefers filesystem canonicalization when paths exist so Windows short-path
/// and verbatim-prefix forms still resolve under the root. Falls back to lexical
/// normalization for targets whose parent directories do not exist yet.
pub(crate) fn portable_relative_under_root(root: &Path, path: &Path) -> CargoAllowResult<PathBuf> {
    let root_canonical = root.canonicalize().map_err(|error| {
        CargoAllowError::new(format!(
            "failed to canonicalize {}: {error}",
            root.display()
        ))
    })?;
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root_canonical.join(path)
    };
    let resolved_canonical = best_effort_canonical(&resolved);
    resolved_canonical
        .strip_prefix(&root_canonical)
        .map(PathBuf::from)
        .map_err(|_| outside_root_error(path, root))
}

fn best_effort_canonical(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }
    let mut current = path.to_path_buf();
    while let Some(parent) = current.parent() {
        if parent.as_os_str().is_empty() {
            break;
        }
        if let Ok(parent_canonical) = parent.canonicalize() {
            if let Ok(suffix) = path.strip_prefix(parent) {
                return parent_canonical.join(suffix);
            }
            break;
        }
        current = parent.to_path_buf();
    }
    path.to_path_buf()
}

fn outside_root_error(path: &Path, root: &Path) -> CargoAllowError {
    CargoAllowError::new(format!(
        "{} is outside source tree {}",
        path.display(),
        root.display()
    ))
}

pub(crate) fn git_relative_config_path(
    root: &Path,
    config: Option<&Path>,
) -> CargoAllowResult<PathBuf> {
    let discovery = discover_config_path(root, config);
    let path = discovery
        .path
        .ok_or_else(|| missing_config_error(&discovery.skipped))?;
    let root = root.canonicalize().map_err(|e| {
        CargoAllowError::new(format!("failed to canonicalize {}: {e}", root.display()))
    })?;
    let path = path.canonicalize().map_err(|e| {
        CargoAllowError::new(format!("failed to canonicalize {}: {e}", path.display()))
    })?;
    path.strip_prefix(&root).map(PathBuf::from).map_err(|_| {
        CargoAllowError::new(format!(
            "policy config {} is not inside source tree {}",
            path.display(),
            root.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{AllowEntry, FindingKind, Lifecycle, Selector};
    use allow_policy::render_policy;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn direct_error_discriminators_match_missing_policy_config_messages() {
        let missing_root = unique_test_dir("policy-config-discriminator-missing");
        let expected =
            CargoAllowError::new("no policy config found; run `cargo-allow init` or pass --config");

        let err = load_config_required_with_evidence_mode(
            &missing_root,
            None,
            EvidenceValidationMode::Abort,
        )
        .expect_err("missing required config should fail");
        assert_eq!(err, expected);

        let err = git_relative_config_path(&missing_root, None)
            .expect_err("missing config should fail git relativization");
        assert_eq!(err, expected);

        remove_test_dir(&missing_root);
    }

    #[test]
    fn required_config_loads_explicit_policy_and_reports_missing_config() {
        let root = unique_test_dir("policy-config-required");
        let policy_path = write_policy(&root, valid_policy_config());

        let cfg = load_config_required_with_evidence_mode(
            &root,
            Some(Path::new("policy/allow.toml")),
            EvidenceValidationMode::Abort,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("load explicit policy: {err}")));

        assert_eq!(cfg.allow.len(), 1);
        assert_eq!(
            cfg.allow.first().map(|entry| entry.id.as_str()),
            Some("allow-doc")
        );
        assert_eq!(
            root_relative_path(&root, Path::new("policy/allow.toml")),
            policy_path
        );

        let missing_root = unique_test_dir("policy-config-required-missing");
        let err = load_config_required_with_evidence_mode(
            &missing_root,
            None,
            EvidenceValidationMode::Abort,
        )
        .expect_err("missing required config should fail");
        assert_eq!(
            err,
            CargoAllowError::new("no policy config found; run `cargo-allow init` or pass --config")
        );
        remove_test_dir(&root);
        remove_test_dir(&missing_root);
    }

    #[test]
    fn optional_config_distinguishes_present_and_missing_policy() {
        let root = unique_test_dir("policy-config-optional");
        write_policy(&root, valid_policy_config());

        let present = load_config_optional_with_evidence_mode(
            &root,
            Some(Path::new("policy/allow.toml")),
            EvidenceValidationMode::Abort,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("load optional policy: {err}")));
        assert!(present.is_some());

        let missing_root = unique_test_dir("policy-config-optional-missing");
        let missing = load_config_optional_with_evidence_mode(
            &missing_root,
            None,
            EvidenceValidationMode::Abort,
        )
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("load missing optional policy: {err}"))
        });
        assert!(missing.is_none());
        remove_test_dir(&root);
        remove_test_dir(&missing_root);
    }

    #[test]
    fn evidence_validation_mode_selects_reportable_or_strict_policy_loading() {
        let root = unique_test_dir("policy-config-evidence-mode");
        write_policy(&root, reportable_link_policy_config());

        let reportable = load_config_required_with_evidence_mode(
            &root,
            Some(Path::new("policy/allow.toml")),
            EvidenceValidationMode::ReportOnly,
        )
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("report-only evidence mode should load: {err}"))
        });
        assert_eq!(
            reportable
                .allow
                .first()
                .and_then(|entry| entry.links.first())
                .map(String::as_str),
            Some("doc:../outside.md")
        );

        let err = load_config_required_with_evidence_mode(
            &root,
            Some(Path::new("policy/allow.toml")),
            EvidenceValidationMode::Abort,
        )
        .expect_err("abort evidence mode should reject invalid local link scope");
        assert!(
            err.to_string()
                .contains("must not contain parent directory segments"),
            "strict link-scope diagnostic should mention source-tree scope: {err}"
        );
        remove_test_dir(&root);
    }

    #[test]
    fn root_relative_path_keeps_absolute_and_joins_relative_paths() {
        let root = unique_test_dir("policy-config-root-relative");
        let absolute = root.join("policy").join("allow.toml");

        assert_eq!(root_relative_path(&root, &absolute), absolute);
        assert_eq!(
            root_relative_path(&root, Path::new("policy/allow.toml")),
            root.join("policy").join("allow.toml")
        );
        remove_test_dir(&root);
    }

    #[test]
    fn git_relative_config_path_reports_inside_outside_and_missing_paths() {
        let root = unique_test_dir("policy-config-git-relative");
        write_policy(&root, valid_policy_config());

        let relative = git_relative_config_path(&root, Some(Path::new("policy/allow.toml")))
            .unwrap_or_else(|err| {
                std::panic::panic_any(format!("inside config should relativize: {err}"))
            });
        assert_eq!(relative, PathBuf::from("policy").join("allow.toml"));

        let missing_root = unique_test_dir("policy-config-git-relative-missing");
        let err = git_relative_config_path(&missing_root, None)
            .expect_err("missing config should report init/config guidance");
        assert_eq!(
            err,
            CargoAllowError::new("no policy config found; run `cargo-allow init` or pass --config")
        );

        let err = git_relative_config_path(&root, Some(Path::new("policy/missing.toml")))
            .expect_err("missing explicit config should report canonicalization failure");
        let missing_config_path = root.join("policy").join("missing.toml");
        let err_text = normalize_slashes(&err.to_string());
        assert!(
            err_text.contains(&normalize_slashes(&format!(
                "failed to canonicalize {}",
                missing_config_path.display()
            ))),
            "missing explicit config diagnostic should mention the config path: {err}"
        );

        let outside_root = unique_test_dir("policy-config-git-relative-outside");
        let outside_policy = write_policy(&outside_root, valid_policy_config());
        let missing_canonical_root = unique_test_dir("policy-config-git-relative-root-gone");
        remove_test_dir(&missing_canonical_root);
        let err = git_relative_config_path(&missing_canonical_root, Some(&outside_policy))
            .expect_err("missing root should report root canonicalization failure");
        let err_text = normalize_slashes(&err.to_string());
        assert!(
            err_text.contains(&normalize_slashes(&format!(
                "failed to canonicalize {}",
                missing_canonical_root.display()
            ))),
            "missing root diagnostic should mention the root path: {err}"
        );

        let err = git_relative_config_path(&root, Some(&outside_policy))
            .expect_err("outside config should be rejected");
        assert!(
            err.to_string().contains("is not inside source tree"),
            "outside config diagnostic should mention source-tree boundary: {err}"
        );
        remove_test_dir(&root);
        remove_test_dir(&missing_root);
        remove_test_dir(&outside_root);
    }

    #[test]
    fn git_relative_config_path_root_canonicalize_error_discriminator() {
        let root = unique_test_dir("policy-config-root-error");
        remove_test_dir(&root);
        let outside_root = unique_test_dir("policy-config-root-error-outside");
        let outside_policy = write_policy(&outside_root, valid_policy_config());

        let err = git_relative_config_path(&root, Some(&outside_policy))
            .expect_err("missing root should fail before config relativization");

        assert!(
            normalize_slashes(&err.to_string()).contains(&normalize_slashes(&format!(
                "failed to canonicalize {}",
                root.display()
            ))),
            "root canonicalization diagnostic should name root: {err}"
        );
        remove_test_dir(&outside_root);
    }

    #[test]
    fn git_relative_config_path_config_canonicalize_error_discriminator() {
        let root = unique_test_dir("policy-config-path-error");
        let missing_config = root.join("policy").join("missing.toml");

        let err = git_relative_config_path(&root, Some(Path::new("policy/missing.toml")))
            .expect_err("missing explicit config should fail during config canonicalization");

        assert!(
            normalize_slashes(&err.to_string()).contains(&normalize_slashes(&format!(
                "failed to canonicalize {}",
                missing_config.display()
            ))),
            "config canonicalization diagnostic should name config path: {err}"
        );
        remove_test_dir(&root);
    }

    fn valid_policy_config() -> AllowConfig {
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(allow_entry(
            "allow-doc",
            FindingKind::NonRustFile,
            Vec::new(),
        ));
        cfg
    }

    fn reportable_link_policy_config() -> AllowConfig {
        let mut cfg = AllowConfig::empty();
        let mut entry = allow_entry(
            "allow-missing-doc",
            FindingKind::NonRustFile,
            vec!["doc:docs/missing.md".to_string()],
        );
        entry.links = vec!["doc:../outside.md".to_string()];
        cfg.allow.push(entry);
        cfg
    }

    fn allow_entry(id: &str, kind: FindingKind, evidence: Vec<String>) -> AllowEntry {
        AllowEntry {
            id: id.to_string(),
            kind,
            family: Some("documentation".to_string()),
            path: Some(PathBuf::from("docs/readme.md")),
            glob: None,
            owner: "docs".to_string(),
            classification: "reviewed".to_string(),
            reason: "fixture".to_string(),
            evidence,
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle {
                created: Some("2026-06-14".to_string()),
                review_after: Some("2026-12-14".to_string()),
                expires: None,
            },
            selector: Selector {
                ast_kind: Some("tracked_file".to_string()),
                ..Selector::default()
            },
            last_seen: None,
        }
    }

    fn write_policy(root: &Path, cfg: AllowConfig) -> PathBuf {
        let policy_dir = root.join("policy");
        fs::create_dir_all(&policy_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
        let policy_path = policy_dir.join("allow.toml");
        fs::write(&policy_path, render_policy(&cfg))
            .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
        policy_path
    }

    fn unique_test_dir(slug: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("cargo-allow-{slug}-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("create fixture dir: {err}")));
        dir
    }

    fn remove_test_dir(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }

    fn normalize_slashes(value: &str) -> String {
        value.replace('\\', "/")
    }
}
