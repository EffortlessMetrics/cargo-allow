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
    /// Filenames in the legacy directory that were not recognized as known
    /// lanes and were silently skipped (#1867). Surfaced as warnings so the
    /// operator knows policy content was left behind.
    pub unmigrated_files: Vec<String>,
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

    // #1867: detect unrecognized .toml files in the legacy directory that
    // were silently skipped because they don't match a known lane descriptor.
    // Collect them so the closeout summary can warn the operator.
    let known_filenames: std::collections::BTreeSet<&str> = all_legacy_lane_descriptors()
        .iter()
        .map(|d| d.legacy_filename)
        .collect();
    let mut unmigrated_files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml")
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
                && !known_filenames.contains(name)
            {
                unmigrated_files.push(name.to_string());
            }
        }
    }
    unmigrated_files.sort();

    // #1861: detect cross-lane ID collisions before validate_policy so we can
    // namespace them with the source lane prefix instead of aborting the entire
    // batch. Each lane's entries get a prefix derived from the legacy filename
    // stem when a collision is detected (e.g. "allow-1" in two lanes becomes
    // "non-rust-allowlist--allow-1" and "no-panic-allowlist--allow-1").
    let mut seen_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut collisions = false;
    for entry in &merged.allow {
        if !seen_ids.insert(entry.id.clone()) {
            collisions = true;
            break;
        }
    }

    if collisions {
        // Re-walk the family list and namespace each lane's entries with the
        // legacy filename stem. Track which entries belong to which lane by
        // the order they were appended.
        seen_ids.clear();
        let mut offset = 0usize;
        for family in &families {
            let prefix = lane_prefix(&family.legacy_filename);
            let count = family.entry_count;
            let lane_entries = merged
                .allow
                .get_mut(offset..offset.saturating_add(count))
                .unwrap_or(&mut []);
            for entry in lane_entries {
                let namespaced = format!("{prefix}--{}", entry.id);
                // Ensure the namespaced ID is itself unique.
                let mut unique_id = namespaced.clone();
                let mut suffix = 1;
                while !seen_ids.insert(unique_id.clone()) {
                    unique_id = format!("{namespaced}-{suffix}");
                    suffix += 1;
                }
                entry.id = unique_id;
            }
            offset += count;
        }
    }

    validate_policy(&merged)?;
    Ok(LegacyImportBatch {
        families,
        config: merged,
        unmigrated_files,
    })
}

/// Derive a short namespace prefix from a legacy filename (e.g.
/// "non-rust-allowlist.toml" → "non-rust-allowlist").
fn lane_prefix(legacy_filename: &str) -> &str {
    legacy_filename
        .strip_suffix(".toml")
        .unwrap_or(legacy_filename)
}

fn load_lane_config(
    descriptor: &LegacyLaneDescriptor,
    path: &Path,
    non_rust_findings: Option<&[Finding]>,
) -> CargoAllowResult<AllowConfig> {
    let legacy_label = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("legacy-policy");
    let result = if descriptor.legacy_filename == "non-rust-allowlist.toml"
        && let Some(findings) = non_rust_findings
    {
        load_non_rust_compat_config(path, findings)
    } else {
        load_legacy_or_canonical(path)
    };
    // #1868: attach the legacy filename without discarding structured error
    // metadata such as a TOML source location.
    result.map_err(|err| {
        err.with_message_prefix(format!(
            "legacy file `{legacy_label}` (policy key `{}`): ",
            descriptor.legacy_policy_key
        ))
    })
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
    fn batch_import_preserves_toml_location_when_adding_file_context() -> Result<(), String> {
        let dir = fixture_dir();
        let path = dir.join("no-panic-allowlist.toml");
        fs::write(&path, "policy = [\n")
            .map_err(|err| format!("write malformed legacy fixture: {err}"))?;

        let error = import_legacy_policy_dir(&dir, None)
            .expect_err("malformed legacy TOML should fail with context");
        let location = error
            .location()
            .ok_or_else(|| "batch context should preserve TOML location".to_string())?;
        let expected_path = path.display().to_string();

        assert_eq!(location.path.as_deref(), Some(expected_path.as_str()));
        assert_eq!(location.line, 1);
        assert!(
            error
                .to_string()
                .contains("legacy file `no-panic-allowlist.toml`"),
            "file context should remain visible: {error}"
        );
        Ok(())
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

    #[test]
    fn batch_import_namespaces_cross_lane_id_collisions() {
        // #1861: two lanes that both produce id = "dup-id" should not abort
        // the batch. The collision detector namespaces them with the lane
        // prefix so validate_policy sees unique IDs.
        let dir = fixture_dir();
        // Two lanes with the same entry ID.
        fs::write(
            dir.join("no-panic-allowlist.toml"),
            r#"
policy = "no-panic-allowlist"
owner = "repo"
status = "advisory"

[[allow]]
id = "dup-id"
path = "src/lib.rs"
family = "unwrap"
owner = "runtime"
classification = "reviewed_panic_exception"
reason = "Checked."
evidence = ["test:dup"]
created = "2026-01-01"
review_after = "2026-09-09"

[allow.selector]
kind = "method-call"
callee = "unwrap"
"#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("write allowlist: {err}")));
        fs::write(
            dir.join("unsafe-allowlist.toml"),
            r#"
policy = "unsafe-allowlist"
owner = "repo"
status = "advisory"

[[allow]]
id = "dup-id"
path = "src/lib.rs"
family = "unsafe-block"
owner = "runtime"
classification = "reviewed_unsafe_boundary"
reason = "Checked."
evidence = ["test:dup"]
created = "2026-01-01"
review_after = "2026-09-09"
"#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("write unsafe: {err}")));

        let batch = import_legacy_policy_dir(&dir, None)
            .unwrap_or_else(|err| std::panic::panic_any(format!("collision batch: {err}")));

        // Both entries survived with namespaced IDs.
        let ids: Vec<&str> = batch.config.allow.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids.len(), 2, "both entries should survive: {ids:?}");
        let unique_ids: std::collections::BTreeSet<&str> = ids.iter().copied().collect();
        assert_eq!(
            unique_ids.len(),
            2,
            "all IDs should be unique after namespacing: {ids:?}"
        );
        assert!(
            ids.iter().any(|id| id.starts_with("no-panic-allowlist--")),
            "no-panic lane entry should be namespaced: {ids:?}"
        );
        assert!(
            ids.iter().any(|id| id.starts_with("unsafe-allowlist--")),
            "unsafe lane entry should be namespaced: {ids:?}"
        );
    }
}
