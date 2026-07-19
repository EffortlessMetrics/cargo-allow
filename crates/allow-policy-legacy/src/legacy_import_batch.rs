//! Multi-family legacy ledger import model for policy-directory batch migration.
//!
//! One batch absorbs multiple compat lanes (panic-family, lint-attribute, and
//! other legacy allowlist files) into a single canonical policy without
//! collapsing families. Import order follows the shared lane-descriptor table.

use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult, Finding, FindingKind};
use allow_policy::validate_policy;
use std::path::Path;

use crate::loader_compat::load_non_rust_compat_config;
use crate::loaders::load_legacy_or_canonical;
use crate::migration_lane_descriptors::{LegacyLaneDescriptor, all_legacy_lane_descriptors};

/// Per-family metadata preserved when importing one legacy compat lane into a batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportFamily {
    pub compat_kind: crate::migration_lane_descriptors::CompatKind,
    pub legacy_filename: String,
    pub legacy_policy_key: &'static str,
    pub finding_kind: FindingKind,
    pub entry_count: usize,
    /// Distinct entry `family` values imported from this lane (sorted, deduplicated).
    pub entry_families: Vec<String>,
}

/// Result of importing multiple legacy policy files from one directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportBatch {
    pub families: Vec<LegacyImportFamily>,
    pub config: AllowConfig,
}

impl LegacyImportBatch {
    pub fn compat_kind_ids(&self) -> Vec<&'static str> {
        self.families
            .iter()
            .map(|family| family.compat_kind.compat_kind_id())
            .collect()
    }

    pub fn legacy_source_files(&self) -> Vec<String> {
        self.families
            .iter()
            .map(|family| family.legacy_filename.clone())
            .collect()
    }
}

pub fn import_legacy_policy_dir(
    dir: &Path,
    non_rust_findings: Option<&[Finding]>,
) -> CargoAllowResult<LegacyImportBatch> {
    if !dir.is_dir() {
        return Err(CargoAllowError::new(format!(
            "{} is not a policy directory",
            dir.display()
        )));
    }

    let mut merged = AllowConfig::empty();
    let mut families = Vec::new();
    let mut loaded = 0usize;

    for descriptor in all_legacy_lane_descriptors() {
        let path = dir.join(descriptor.legacy_filename);
        if !path.is_file() {
            continue;
        }
        let cfg = load_lane_config(descriptor, &path, non_rust_findings)?;
        if loaded == 0 {
            merged.owner = cfg.owner.clone();
            merged.status = cfg.status.clone();
            merged.workspace = cfg.workspace.clone();
            merged.requirements = cfg.requirements.clone();
        }
        loaded += 1;
        families.push(LegacyImportFamily {
            compat_kind: descriptor.lane,
            legacy_filename: descriptor.legacy_filename.to_string(),
            legacy_policy_key: descriptor.legacy_policy_key,
            finding_kind: descriptor.canonical_shape.finding_kind,
            entry_count: cfg.allow.len(),
            entry_families: entry_families_from_config(&cfg),
        });
        merged.allow.extend(cfg.allow);
    }

    if loaded == 0 {
        return Err(CargoAllowError::new(format!(
            "{} contains no supported legacy policy files",
            dir.display()
        )));
    }

    validate_policy(&merged)?;
    Ok(LegacyImportBatch {
        families,
        config: merged,
    })
}

fn load_lane_config(
    descriptor: &LegacyLaneDescriptor,
    path: &Path,
    non_rust_findings: Option<&[Finding]>,
) -> CargoAllowResult<AllowConfig> {
    if descriptor.legacy_filename == "non-rust-allowlist.toml"
        && let Some(findings) = non_rust_findings
    {
        return load_non_rust_compat_config(path, findings);
    }
    load_legacy_or_canonical(path)
}

fn entry_families_from_config(cfg: &AllowConfig) -> Vec<String> {
    let mut families = cfg
        .allow
        .iter()
        .filter_map(|entry| entry.family.clone())
        .collect::<Vec<_>>();
    families.sort();
    families.dedup();
    families
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration_lane_descriptors::CompatKind;
    use crate::test_support::{
        clippy_policy_fixture_text, fixture_dir, no_panic_allowlist_fixture_text,
        no_panic_baseline_fixture_text,
    };
    use allow_core::FindingKind;
    use std::fs;

    fn migration_fixture_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/migration")
    }

    fn stage_migration_fixture(dir: &Path, fixture_file: &str, legacy_filename: &str) {
        let source = migration_fixture_root().join(fixture_file);
        let text = fs::read_to_string(&source).unwrap_or_else(|err| {
            std::panic::panic_any(format!(
                "read migration fixture {}: {err}",
                source.display()
            ))
        });
        fs::write(dir.join(legacy_filename), text).unwrap_or_else(|err| {
            std::panic::panic_any(format!(
                "write staged migration fixture {}: {err}",
                legacy_filename
            ))
        });
    }

    #[test]
    fn multi_family_batch_import_preserves_panic_and_lint_metadata() {
        let dir = fixture_dir();
        stage_migration_fixture(&dir, "no-panic-allowlist.toml", "no-panic-allowlist.toml");
        stage_migration_fixture(&dir, "panic-baseline.toml", "no-panic-baseline.toml");
        stage_migration_fixture(&dir, "lint-exception.toml", "clippy-exceptions.toml");

        let batch = import_legacy_policy_dir(&dir, None).unwrap_or_else(|err| {
            std::panic::panic_any(format!("multi-family batch import: {err}"))
        });

        assert_eq!(batch.families.len(), 3);
        assert_eq!(
            batch.families[0].compat_kind,
            CompatKind::NoPanicAllowlist,
            "batch import order follows lane-descriptor table"
        );
        assert_eq!(batch.families[1].compat_kind, CompatKind::PanicBaseline);
        assert_eq!(batch.families[2].compat_kind, CompatKind::LintException);

        assert_eq!(batch.families[0].finding_kind, FindingKind::Panic);
        assert_eq!(batch.families[1].finding_kind, FindingKind::Panic);
        assert_eq!(batch.families[2].finding_kind, FindingKind::LintException);

        assert!(
            batch.families[0].entry_families == vec!["unwrap".to_string()],
            "no-panic allowlist should preserve unwrap family"
        );
        assert!(
            batch.families[1].entry_families == vec!["unwrap".to_string()],
            "panic baseline should preserve unwrap family without collapsing allowlist"
        );
        assert!(
            batch.families[2].entry_families == vec!["expect_attribute".to_string()],
            "lint lane should preserve expect_attribute family"
        );

        let cfg = &batch.config;
        assert!(
            cfg.allow
                .iter()
                .any(|entry| entry.id == "fixture-no-panic-unwrap"
                    && entry.kind == FindingKind::Panic
                    && entry.family.as_deref() == Some("unwrap")
                    && entry.classification == "reviewed_panic_exception"
                    && entry
                        .reason
                        .contains("Parser validates the optional value.")),
            "reviewed panic allowlist reason should survive batch import"
        );
        assert!(
            cfg.allow
                .iter()
                .any(|entry| entry.id == "panic-baseline-0001"
                    && entry.kind == FindingKind::Panic
                    && entry.family.as_deref() == Some("unwrap")
                    && entry.classification == "baseline_debt"
                    && entry.occurrence_limit == Some(2)),
            "panic baseline occurrence limit should survive batch import"
        );
        assert!(
            cfg.allow.iter().any(|entry| entry.id == "fixture-clippy"
                && entry.kind == FindingKind::LintException
                && entry.family.as_deref() == Some("expect_attribute")
                && entry
                    .reason
                    .contains("Parser validates optional value before unwrap.")),
            "lint exception reason should survive batch import"
        );
    }

    #[test]
    fn multi_family_batch_import_is_deterministic() {
        let dir = fixture_dir();
        fs::write(
            dir.join("no-panic-allowlist.toml"),
            no_panic_allowlist_fixture_text(),
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("allowlist fixture: {err}")));
        fs::write(
            dir.join("no-panic-baseline.toml"),
            no_panic_baseline_fixture_text(),
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("baseline fixture: {err}")));
        fs::write(
            dir.join("clippy-exceptions.toml"),
            clippy_policy_fixture_text(),
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("clippy fixture: {err}")));

        let first = import_legacy_policy_dir(&dir, None)
            .unwrap_or_else(|err| std::panic::panic_any(format!("first batch import: {err}")));
        let second = import_legacy_policy_dir(&dir, None)
            .unwrap_or_else(|err| std::panic::panic_any(format!("second batch import: {err}")));

        assert_eq!(first.families, second.families);
        assert_eq!(
            batch_fingerprint(&first.config),
            batch_fingerprint(&second.config)
        );
    }

    fn batch_fingerprint(cfg: &AllowConfig) -> Vec<String> {
        let mut rows = cfg
            .allow
            .iter()
            .map(|entry| {
                format!(
                    "{}|{:?}|{}|{}|{:?}",
                    entry.id,
                    entry.family,
                    entry.classification,
                    entry.reason,
                    entry.occurrence_limit
                )
            })
            .collect::<Vec<_>>();
        rows.sort();
        rows
    }
}
