use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult, Finding};
use allow_policy::validate_policy;
use std::path::Path;

use crate::loader_compat::load_non_rust_compat_config;
use crate::loaders::load_legacy_or_canonical;

pub(crate) fn legacy_policy_file_names() -> &'static [&'static str] {
    LEGACY_POLICY_FILES
}

const LEGACY_POLICY_FILES: &[&str] = &[
    "non-rust-allowlist.toml",
    "generated-allowlist.toml",
    "no-panic-allowlist.toml",
    "no-panic-baseline.toml",
    "clippy-exceptions.toml",
    "unsafe-allowlist.toml",
    "executable-allowlist.toml",
    "workflow-allowlist.toml",
    "dependency-surface-allowlist.toml",
    "process-allowlist.toml",
    "network-allowlist.toml",
];

pub fn load_legacy_policy_dir(dir: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    load_legacy_policy_dir_inner(dir.as_ref(), None)
}

pub fn load_legacy_policy_dir_with_non_rust_findings(
    dir: impl AsRef<Path>,
    findings: &[Finding],
) -> CargoAllowResult<AllowConfig> {
    load_legacy_policy_dir_inner(dir.as_ref(), Some(findings))
}

fn load_legacy_policy_dir_inner(
    dir: &Path,
    non_rust_findings: Option<&[Finding]>,
) -> CargoAllowResult<AllowConfig> {
    if !dir.is_dir() {
        return Err(CargoAllowError::new(format!(
            "{} is not a policy directory",
            dir.display()
        )));
    }

    let mut merged = AllowConfig::empty();
    let mut loaded = 0usize;
    for file_name in LEGACY_POLICY_FILES {
        let path = dir.join(file_name);
        if !path.is_file() {
            continue;
        }
        let cfg = if *file_name == "non-rust-allowlist.toml" {
            if let Some(findings) = non_rust_findings {
                load_non_rust_compat_config(&path, findings)?
            } else {
                load_legacy_or_canonical(&path)?
            }
        } else {
            load_legacy_or_canonical(&path)?
        };
        if loaded == 0 {
            merged.owner = cfg.owner.clone();
            merged.status = cfg.status.clone();
            merged.workspace = cfg.workspace.clone();
            merged.requirements = cfg.requirements.clone();
        }
        loaded += 1;
        merged.allow.extend(cfg.allow);
    }

    if loaded == 0 {
        return Err(CargoAllowError::new(format!(
            "{} contains no supported legacy policy files",
            dir.display()
        )));
    }

    validate_policy(&merged)?;
    Ok(merged)
}

pub fn migration_notes() -> &'static str {
    "Legacy migration accepts canonical cargo-allow policies plus shiplog-style non-rust, generated, no-panic-allowlist, no-panic-baseline, clippy-exceptions, unsafe-allowlist, executable, workflow, dependency-surface, process, and network allowlists. Non-Rust compat expands matching legacy globs to exact current file entries; generated compat compares .gitattributes generated paths with policy/generated-allowlist.toml; no-panic allowlist migration maps retained source exceptions to structural panic receipts and treats last_seen as a hint only; no-panic baseline migration emits count-limited baseline_debt entries; clippy-exceptions compat maps retained lint suppression entries to source-syntax lint_exception receipts and uses cargo-allow's Rust source scanner for current findings; unsafe compat maps retained unsafe entries to source-syntax unsafe receipts and keeps missing evidence as temporary baseline_debt TODO evidence; executable compat compares git tree mode 100755 paths with policy/executable-allowlist.toml; workflow compat compares .github/workflows files and uses: actions with policy/workflow-allowlist.toml; dependency-surface compat preserves the legacy pattern-matches-tracked-file check; process compat validates retained process policy entries and does not scan source code for process spawns; network compat validates retained network policy entries and does not scan source code or runtime traffic."
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        finding, fixture_dir, network_policy_fixture_text, policy_fixture_text,
        process_policy_fixture_text,
    };
    use std::fs;
    use std::path::Path;

    #[test]
    fn load_legacy_policy_dir_inner_rejects_non_directory_and_empty_policy_dir() {
        let dir = fixture_dir();
        let not_a_dir = dir.join("policy.toml");
        fs::write(&not_a_dir, "not a policy directory")
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));

        let file_err = load_legacy_policy_dir_inner(&not_a_dir, None)
            .expect_err("file path should not load as a legacy policy directory");
        assert!(
            file_err.to_string().contains("is not a policy directory"),
            "unexpected non-directory error: {file_err}"
        );

        let empty = fixture_dir();
        let empty_err = load_legacy_policy_dir_inner(&empty, None)
            .expect_err("empty directory should not load as a legacy policy directory");
        assert!(
            empty_err
                .to_string()
                .contains("contains no supported legacy policy files"),
            "unexpected empty-directory error: {empty_err}"
        );
    }

    #[test]
    fn load_legacy_policy_dir_inner_merges_supported_files_and_first_metadata() {
        let dir = fixture_dir();
        fs::write(
            dir.join("process-allowlist.toml"),
            process_policy_fixture_text(),
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("process fixture write: {err}")));
        fs::write(
            dir.join("network-allowlist.toml"),
            network_policy_fixture_text(),
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("network fixture write: {err}")));
        fs::write(dir.join("README.md"), "unsupported file")
            .unwrap_or_else(|err| std::panic::panic_any(format!("extra fixture write: {err}")));

        let cfg = load_legacy_policy_dir_inner(&dir, None)
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir migrates: {err}")));

        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.owner.as_deref(), Some("EffortlessMetrics"));
        assert_eq!(cfg.status.as_deref(), Some("advisory"));
        assert_eq!(cfg.allow.len(), 4);
        assert!(
            cfg.allow
                .iter()
                .any(|entry| entry.family.as_deref() == Some("process_spawn")
                    && entry.path.as_deref() == Some(Path::new(".github/workflows/ci.yml")))
        );
        assert!(cfg.allow.iter().any(|entry| entry.family.as_deref()
            == Some("network_destination")
            && entry.selector.symbol.as_deref() == Some("api.github.com lane release")));
    }

    #[test]
    fn load_legacy_policy_dir_inner_uses_non_rust_findings_for_non_rust_policy() {
        let dir = fixture_dir();
        fs::write(dir.join("non-rust-allowlist.toml"), policy_fixture_text())
            .unwrap_or_else(|err| std::panic::panic_any(format!("non-rust fixture write: {err}")));
        let findings = vec![finding(".github/workflows/ci.yml", "tracked_file")];

        let cfg = load_legacy_policy_dir_inner(&dir, Some(&findings)).unwrap_or_else(|err| {
            std::panic::panic_any(format!("policy dir with non-rust findings migrates: {err}"))
        });

        assert_eq!(cfg.allow.len(), 1);
        let entry = cfg
            .allow
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected expanded non-rust entry"));
        assert_eq!(entry.id, "non-rust-github-workflows--0001");
        assert_eq!(
            entry.path.as_deref(),
            Some(Path::new(".github/workflows/ci.yml"))
        );
        assert_eq!(entry.links, vec!["legacy-policy:non-rust-github-workflows"]);
    }
}
