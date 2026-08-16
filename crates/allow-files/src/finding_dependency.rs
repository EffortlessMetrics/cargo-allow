use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, FindingKind, glob_matches,
    normalize_path,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn dependency_surface_findings_from_git(
    root: impl AsRef<Path>,
    cfg: &AllowConfig,
) -> CargoAllowResult<Vec<Finding>> {
    let tracked = git_ls_files(root)?;
    Ok(dependency_surface_findings_from_paths(&tracked, cfg))
}

pub fn dependency_surface_findings_from_paths(
    inventory_paths: &[PathBuf],
    cfg: &AllowConfig,
) -> Vec<Finding> {
    let mut paths = BTreeSet::new();
    for entry in &cfg.allow {
        if entry.kind != FindingKind::PolicyException
            || entry.family.as_deref() != Some("dependency_surface")
        {
            continue;
        }
        for path in inventory_paths {
            if dependency_entry_matches_path(entry, path) {
                paths.insert(path.to_path_buf());
            }
        }
    }
    paths.into_iter().map(dependency_surface_finding).collect()
}

fn git_ls_files(root: impl AsRef<Path>) -> CargoAllowResult<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(["ls-files"])
        .current_dir(root.as_ref())
        .output()
        .map_err(|e| CargoAllowError::new(format!("failed to run git ls-files: {e}")))?;
    if !output.status.success() {
        return Err(CargoAllowError::new(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|e| CargoAllowError::new(format!("git ls-files output was not UTF-8: {e}")))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect())
}

pub fn dependency_surface_finding(path: PathBuf) -> Finding {
    let normalized = normalize_path(&path);
    let mut identity = allow_core::StructuralIdentity::new("file", "dependency_surface");
    identity.symbol = Some(normalized.clone());
    identity.target_fingerprint = Some(dependency_surface_family(&path));
    Finding {
        kind: FindingKind::PolicyException,
        family: Some("dependency_surface".to_string()),
        path,
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity,
        message: format!("tracked dependency surface {normalized}"),
        ledger: None,
    }
}

fn dependency_surface_family(path: &Path) -> String {
    let normalized = normalize_path(path);
    match normalized.as_str() {
        "Cargo.toml" => "workspace_manifest".to_string(),
        "Cargo.lock" => "workspace_lockfile".to_string(),
        "rust-toolchain.toml" => "toolchain_pin".to_string(),
        "deny.toml" => "policy_config".to_string(),
        text if text.ends_with("/Cargo.toml") => "crate_manifest".to_string(),
        text if text.ends_with("/Cargo.lock") => "lockfile".to_string(),
        text if text.ends_with("/rust-toolchain.toml") => "toolchain_pin".to_string(),
        _ => "dependency_surface".to_string(),
    }
}

fn dependency_entry_matches_path(entry: &AllowEntry, path: &Path) -> bool {
    entry
        .path
        .as_ref()
        .is_some_and(|scope| normalize_path(scope) == normalize_path(path))
        || entry
            .glob
            .as_ref()
            .is_some_and(|glob| glob_matches(glob, path))
        || entry
            .selector
            .glob
            .as_ref()
            .is_some_and(|glob| glob_matches(glob, path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{Lifecycle, Selector, WorkspaceConfig};

    #[test]
    fn dependency_surface_finding_preserves_family_identity_and_message() {
        let finding = dependency_surface_finding(PathBuf::from("crates\\core\\Cargo.toml"));

        assert_eq!(finding.kind, FindingKind::PolicyException);
        assert_eq!(finding.family.as_deref(), Some("dependency_surface"));
        assert_eq!(finding.path, PathBuf::from("crates\\core\\Cargo.toml"));
        assert_eq!(
            finding.span.as_ref().map(|span| (span.line, span.column)),
            Some((1, 1))
        );
        assert_eq!(finding.identity.language, "file");
        assert_eq!(finding.identity.ast_kind, "dependency_surface");
        assert_eq!(
            finding.identity.symbol.as_deref(),
            Some("crates/core/Cargo.toml")
        );
        assert_eq!(
            finding.identity.target_fingerprint.as_deref(),
            Some("crate_manifest")
        );
        assert_eq!(
            finding.message,
            "tracked dependency surface crates/core/Cargo.toml"
        );
    }

    #[test]
    fn dependency_surface_family_classifies_known_paths() {
        assert_eq!(
            dependency_surface_family(Path::new("Cargo.toml")),
            "workspace_manifest"
        );
        assert_eq!(
            dependency_surface_family(Path::new("Cargo.lock")),
            "workspace_lockfile"
        );
        assert_eq!(
            dependency_surface_family(Path::new("rust-toolchain.toml")),
            "toolchain_pin"
        );
        assert_eq!(
            dependency_surface_family(Path::new("deny.toml")),
            "policy_config"
        );
        assert_eq!(
            dependency_surface_family(Path::new("crates/core/Cargo.toml")),
            "crate_manifest"
        );
        assert_eq!(
            dependency_surface_family(Path::new("crates/core/Cargo.lock")),
            "lockfile"
        );
        assert_eq!(
            dependency_surface_family(Path::new("crates/core/rust-toolchain.toml")),
            "toolchain_pin"
        );
        assert_eq!(
            dependency_surface_family(Path::new("docs/dependencies.md")),
            "dependency_surface"
        );
    }

    #[test]
    fn dependency_entry_matches_direct_path_glob_or_selector_glob() {
        let mut path_entry = dependency_entry();
        path_entry.path = Some(PathBuf::from("Cargo.toml"));

        let mut glob_entry = dependency_entry();
        glob_entry.glob = Some("crates/*/Cargo.toml".to_string());

        let mut selector_entry = dependency_entry();
        selector_entry.selector = Selector {
            glob: Some("tools/**/Cargo.toml".to_string()),
            ..Selector::default()
        };

        assert!(dependency_entry_matches_path(
            &path_entry,
            Path::new("Cargo.toml")
        ));
        assert!(!dependency_entry_matches_path(
            &path_entry,
            Path::new("Cargo.lock")
        ));
        assert!(dependency_entry_matches_path(
            &glob_entry,
            Path::new("crates/core/Cargo.toml")
        ));
        assert!(dependency_entry_matches_path(
            &selector_entry,
            Path::new("tools/xtask/Cargo.toml")
        ));
        assert!(!dependency_entry_matches_path(
            &selector_entry,
            Path::new("docs/Cargo.toml")
        ));
    }

    #[test]
    fn dependency_surface_findings_filter_policy_entries_and_sort_paths() {
        let cfg = AllowConfig {
            schema_version: "0.1".to_string(),
            policy: "cargo-allow".to_string(),
            owner: None,
            status: None,
            workspace: WorkspaceConfig::default(),
            requirements: Default::default(),
            lanes: std::collections::BTreeMap::new(),
            allow: vec![
                dependency_entry_with_glob("crates/*/Cargo.toml"),
                non_dependency_entry(),
            ],
        };
        let paths = vec![
            PathBuf::from("README.md"),
            PathBuf::from("crates/zeta/Cargo.toml"),
            PathBuf::from("crates/alpha/Cargo.toml"),
        ];

        let findings = dependency_surface_findings_from_paths(&paths, &cfg);

        assert_eq!(
            findings
                .iter()
                .map(|finding| normalize_path(&finding.path))
                .collect::<Vec<_>>(),
            vec!["crates/alpha/Cargo.toml", "crates/zeta/Cargo.toml"]
        );
    }

    fn dependency_entry() -> AllowEntry {
        AllowEntry {
            id: "dependency-surface".to_string(),
            kind: FindingKind::PolicyException,
            family: Some("dependency_surface".to_string()),
            path: None,
            glob: None,
            owner: "release".to_string(),
            classification: "dependency_surface".to_string(),
            reason: "Dependency surface is governed.".to_string(),
            evidence: vec!["doc:docs/dependencies.md".to_string()],
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle::empty(),
            selector: Selector::default(),
            last_seen: None,
        }
    }

    fn dependency_entry_with_glob(glob: &str) -> AllowEntry {
        let mut entry = dependency_entry();
        entry.glob = Some(glob.to_string());
        entry
    }

    fn non_dependency_entry() -> AllowEntry {
        let mut entry = dependency_entry();
        entry.id = "other-policy".to_string();
        entry.family = Some("github_workflow".to_string());
        entry.glob = Some(".github/workflows/*.yml".to_string());
        entry
    }
}
