use allow_core::{AllowConfig, CargoAllowResult, Finding};
use std::path::Path;

use crate::legacy_import_batch::import_legacy_policy_dir;

pub fn load_legacy_policy_dir(dir: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    import_legacy_policy_dir(dir.as_ref(), None).map(|batch| batch.config)
}

pub fn load_legacy_policy_dir_with_non_rust_findings(
    dir: impl AsRef<Path>,
    findings: &[Finding],
) -> CargoAllowResult<AllowConfig> {
    import_legacy_policy_dir(dir.as_ref(), Some(findings)).map(|batch| batch.config)
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
    fn import_legacy_policy_dir_keeps_lane_status_and_defaults_only_when_undeclared() {
        // #1866 merged status with a first-non-None rule, but `AllowConfig::empty()`
        // seeds `status = "active"`, so every lane's declared status was discarded
        // and an advisory legacy policy silently imported as enforcing.
        let declared = fixture_dir();
        fs::write(
            declared.join("process-allowlist.toml"),
            process_policy_fixture_text(),
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("declared fixture write: {err}")));

        let batch = import_legacy_policy_dir(&declared, None)
            .unwrap_or_else(|err| std::panic::panic_any(format!("declared dir migrates: {err}")));
        assert_eq!(
            batch.config.status.as_deref(),
            Some("advisory"),
            "a lane's declared status must survive migration rather than being promoted to active"
        );

        // With no lane declaring a status, the documented default still applies.
        let undeclared = fixture_dir();
        let without_status = process_policy_fixture_text().replace("status = \"advisory\"\n", "");
        assert!(
            !without_status.contains("status ="),
            "the undeclared fixture must not declare a status"
        );
        fs::write(undeclared.join("process-allowlist.toml"), without_status).unwrap_or_else(
            |err| std::panic::panic_any(format!("undeclared fixture write: {err}")),
        );

        let batch = import_legacy_policy_dir(&undeclared, None)
            .unwrap_or_else(|err| std::panic::panic_any(format!("undeclared dir migrates: {err}")));
        assert_eq!(
            batch.config.status.as_deref(),
            Some("active"),
            "an undeclared status must fall back to the empty-config default"
        );
    }

    #[test]
    fn import_legacy_policy_dir_rejects_non_directory_and_empty_policy_dir() {
        let dir = fixture_dir();
        let not_a_dir = dir.join("policy.toml");
        fs::write(&not_a_dir, "not a policy directory")
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));

        let file_err = import_legacy_policy_dir(&not_a_dir, None)
            .expect_err("file path should not load as a legacy policy directory");
        assert!(
            file_err.to_string().contains("is not a policy directory"),
            "unexpected non-directory error: {file_err}"
        );

        let empty = fixture_dir();
        let empty_err = import_legacy_policy_dir(&empty, None)
            .expect_err("empty directory should not load as a legacy policy directory");
        assert!(
            empty_err
                .to_string()
                .contains("contains no supported legacy policy files"),
            "unexpected empty-directory error: {empty_err}"
        );
    }

    #[test]
    fn import_legacy_policy_dir_merges_supported_files_and_first_metadata() {
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

        let batch = import_legacy_policy_dir(&dir, None)
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir migrates: {err}")));

        assert_eq!(batch.families.len(), 2);
        assert_eq!(batch.config.policy, "cargo-allow");
        assert_eq!(batch.config.owner.as_deref(), Some("EffortlessMetrics"));
        assert_eq!(batch.config.status.as_deref(), Some("advisory"));
        assert_eq!(batch.config.allow.len(), 4);
        assert!(
            batch
                .config
                .allow
                .iter()
                .any(|entry| entry.family.as_deref() == Some("process_spawn")
                    && entry.path.as_deref() == Some(Path::new(".github/workflows/ci.yml")))
        );
        assert!(
            batch
                .config
                .allow
                .iter()
                .any(
                    |entry| entry.family.as_deref() == Some("network_destination")
                        && entry.selector.symbol.as_deref() == Some("api.github.com lane release")
                )
        );
    }

    #[test]
    fn import_legacy_policy_dir_uses_non_rust_findings_for_non_rust_policy() {
        let dir = fixture_dir();
        fs::write(dir.join("non-rust-allowlist.toml"), policy_fixture_text())
            .unwrap_or_else(|err| std::panic::panic_any(format!("non-rust fixture write: {err}")));
        let findings = vec![finding(".github/workflows/ci.yml", "tracked_file")];

        let batch = import_legacy_policy_dir(&dir, Some(&findings)).unwrap_or_else(|err| {
            std::panic::panic_any(format!("policy dir with non-rust findings migrates: {err}"))
        });

        assert_eq!(batch.families.len(), 1);
        assert_eq!(batch.config.allow.len(), 1);
        let entry = batch
            .config
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
