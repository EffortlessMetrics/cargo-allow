use allow_core::{CargoAllowError, CargoAllowResult, FindingKind, normalize_path};
use allow_inventory::{InventoryOptions, inventory, resolve_source_tree_root};
use allow_policy::import_bespoke_ledger_at;
use allow_report::MigrateBaselineDebtProjection;
use std::path::{Path, PathBuf};

use super::migrate_types::{MigrateContext, MigrationLoad};
use crate::{current_dir, root_relative_path};

/// Compute the baseline-debt closeout projection from legacy lane descriptors.
/// `allow-report` consumes the result without depending on `allow-policy-legacy`
/// (#2941).
fn baseline_debt_projection(legacy_compat_kinds: &[&'static str]) -> MigrateBaselineDebtProjection {
    let (signal, label) = allow_policy_legacy::baseline_debt_projection(legacy_compat_kinds);
    MigrateBaselineDebtProjection { signal, label }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SingleFileMigrationSource {
    BespokeLedger,
    LegacyOrCanonical,
}

pub(super) fn load_single_file_migration_config(
    explicit_root: Option<&Path>,
    from: &Path,
) -> CargoAllowResult<MigrationLoad> {
    let root = resolve_source_tree_root(explicit_root, from)?;
    let inventory = inventory(&root, &InventoryOptions::default())?;
    let inventory_source = inventory.source;
    let files_scanned = inventory.files.len();
    let (cfg, source) = load_single_file_policy(from)?;
    let legacy_compat_kinds = match source {
        SingleFileMigrationSource::BespokeLedger => vec!["bespoke-ledger"],
        SingleFileMigrationSource::LegacyOrCanonical => legacy_source_compat_kinds(from),
    };
    Ok(MigrationLoad {
        cfg,
        context: MigrateContext {
            inventory_source: inventory_source.as_str().to_string(),
            source_tree_root: Some(allow_report::source_tree_path_text(&root)),
            inventory_files: Some(files_scanned),
            input_kind: match source {
                SingleFileMigrationSource::BespokeLedger => "bespoke".to_string(),
                SingleFileMigrationSource::LegacyOrCanonical => "from".to_string(),
            },
            input_path: normalize_path(from),
            legacy_source_files: match source {
                SingleFileMigrationSource::BespokeLedger => vec![
                    from.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("bespoke-ledger.toml")
                        .to_string(),
                ],
                SingleFileMigrationSource::LegacyOrCanonical => legacy_source_file_names(from),
            },
            baseline_debt_projection: baseline_debt_projection(&legacy_compat_kinds),
            legacy_compat_kinds,
        },
        root: Some(root),
    })
}

fn load_single_file_policy(
    from: &Path,
) -> CargoAllowResult<(allow_core::AllowConfig, SingleFileMigrationSource)> {
    if let Some(cfg) = import_bespoke_ledger_at(from)? {
        return Ok((cfg, SingleFileMigrationSource::BespokeLedger));
    }
    Ok((
        allow_policy_legacy::load_legacy_or_canonical(from)?,
        SingleFileMigrationSource::LegacyOrCanonical,
    ))
}

pub(super) fn load_repo_policy_migration_config(
    explicit_root: Option<&Path>,
    repo_policy: &Path,
) -> CargoAllowResult<MigrationLoad> {
    let root = repo_policy_source_tree_root(explicit_root, repo_policy)?;
    let repo_policy = root_relative_path(&root, repo_policy);
    let inventory = inventory(&root, &InventoryOptions::default())?;
    let inventory_source = inventory.source;
    let files_scanned = inventory.files.len();
    let findings = allow_files::scan_files(&inventory.files)
        .into_iter()
        .filter(|finding| finding.kind == FindingKind::NonRustFile)
        .collect::<Vec<_>>();
    let batch = allow_policy_legacy::import_legacy_policy_dir(&repo_policy, Some(&findings))?;
    let legacy_source_files = batch.legacy_source_files();
    let legacy_compat_kinds = batch.compat_kind_ids();
    // #1867: surface unrecognized files from the legacy directory as warnings.
    for file in &batch.unmigrated_files {
        eprintln!("warning: legacy directory contains unrecognized file `{file}` — not migrated");
    }
    Ok(MigrationLoad {
        cfg: batch.config,
        context: MigrateContext {
            inventory_source: inventory_source.as_str().to_string(),
            source_tree_root: Some(allow_report::source_tree_path_text(&root)),
            inventory_files: Some(files_scanned),
            input_kind: "repo_policy".to_string(),
            input_path: normalize_path(&repo_policy),
            legacy_source_files,
            baseline_debt_projection: baseline_debt_projection(&legacy_compat_kinds),
            legacy_compat_kinds,
        },
        root: Some(root),
    })
}

fn repo_policy_source_tree_root(
    explicit_root: Option<&Path>,
    repo_policy: &Path,
) -> CargoAllowResult<PathBuf> {
    if let Some(root) = explicit_root {
        return resolve_source_tree_root(Some(root), root);
    }
    let cwd = current_dir()?;
    let full_policy_path = if repo_policy.is_absolute() {
        repo_policy.to_path_buf()
    } else {
        cwd.join(repo_policy)
    };
    if full_policy_path.file_name().and_then(|name| name.to_str()) == Some("policy") {
        return full_policy_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| {
                CargoAllowError::new(format!(
                    "failed to infer repository root from {}",
                    repo_policy.display()
                ))
            });
    }
    resolve_source_tree_root(None, cwd)
}

fn legacy_source_file_names(from: &Path) -> Vec<String> {
    allow_policy_legacy::legacy_policy_source_for_path(from)
        .map(|source| vec![source.file_name])
        .unwrap_or_default()
}

fn legacy_source_compat_kinds(from: &Path) -> Vec<&'static str> {
    allow_policy_legacy::legacy_policy_source_for_path(from)
        .map(|source| vec![source.compat_kind])
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{AllowConfig, AllowEntry, Lifecycle, Selector};
    use allow_policy::render_policy;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn load_single_file_migration_config_imports_bespoke_ledger() {
        let root = unique_test_dir("migrate-load-bespoke");
        let from = root.join("bespoke.toml");
        fs::write(&from, bespoke_ledger_fixture_text())
            .unwrap_or_else(|err| std::panic::panic_any(format!("write bespoke ledger: {err}")));

        let migration = load_single_file_migration_config(Some(&root), &from)
            .unwrap_or_else(|err| std::panic::panic_any(format!("load bespoke migrate: {err}")));

        assert_eq!(migration.cfg.allow.len(), 1);
        assert_eq!(
            migration.cfg.allow.first().map(|entry| entry.id.as_str()),
            Some("fixture-semantic-unwrap")
        );
        assert_eq!(migration.context.input_kind, "bespoke");
        assert_eq!(
            migration.context.legacy_compat_kinds,
            vec!["bespoke-ledger"]
        );
        remove_test_dir(&root);
    }

    #[test]
    fn load_bespoke_ledger_migration_reports_location_drift_without_failing_no_new() {
        use allow_core::{Finding, MatchStatus, Span, StructuralIdentity};
        use std::path::PathBuf;

        let root = unique_test_dir("migrate-load-bespoke-drift");
        let from = root.join("bespoke-drift.toml");
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/migration/bespoke-ledger-advisory-drift-v1.toml");
        let text = fs::read_to_string(&fixture).unwrap_or_else(|err| {
            std::panic::panic_any(format!("read bespoke advisory drift fixture: {err}"))
        });
        fs::write(&from, text).unwrap_or_else(|err| {
            std::panic::panic_any(format!("write bespoke drift ledger: {err}"))
        });

        let migration =
            load_single_file_migration_config(Some(&root), &from).unwrap_or_else(|err| {
                std::panic::panic_any(format!("load bespoke drift migrate: {err}"))
            });

        let finding = Finding {
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: PathBuf::from("src/lib.rs"),
            span: Some(Span {
                line: 22,
                column: 4,
            }),
            identity: {
                let mut identity = StructuralIdentity::new("rust", "method_call");
                identity.container = Some("load".to_string());
                identity.callee = Some("unwrap".to_string());
                identity.receiver_fingerprint = Some("optional_value".to_string());
                identity
            },
            message: String::new(),
            ledger: None,
        };

        let outcomes = allow_match::evaluate(
            &migration.cfg,
            std::slice::from_ref(&finding),
            allow_match::CheckMode::NoNew,
        );

        let drift = outcomes
            .iter()
            .find(|outcome| outcome.status == MatchStatus::LocationDrift)
            .unwrap_or_else(|| std::panic::panic_any("expected location_drift outcome"));
        assert!(drift.message.contains("last_seen changed from 14:8"));
        assert!(
            !allow_match::CheckMode::NoNew.fails(drift.status),
            "location drift should remain advisory in no-new mode"
        );
        remove_test_dir(&root);
    }

    #[test]
    fn load_single_file_migration_config_call_presence_observer() {
        let root = unique_test_dir("migrate-load-single");
        let canonical_root = root
            .canonicalize()
            .unwrap_or_else(|err| std::panic::panic_any(format!("canonicalize root: {err}")));
        let from = root.join("legacy.allow.toml");
        fs::write(&from, render_policy(&canonical_policy_config()))
            .unwrap_or_else(|err| std::panic::panic_any(format!("write canonical policy: {err}")));

        let migration = load_single_file_migration_config(Some(&root), &from)
            .unwrap_or_else(|err| std::panic::panic_any(format!("load single migrate: {err}")));

        assert_eq!(migration.cfg.allow.len(), 1);
        assert_eq!(
            migration.cfg.allow.first().map(|entry| entry.id.as_str()),
            Some("allow-migrated-doc")
        );
        assert_eq!(migration.context.inventory_source, "filesystem_fallback");
        assert_eq!(
            migration.context.source_tree_root.as_deref(),
            Some(allow_report::source_tree_path_text(&canonical_root).as_str())
        );
        assert!(
            migration
                .context
                .inventory_files
                .is_some_and(|count| count >= 1),
            "single-file migration should report inventory file count"
        );
        assert_eq!(migration.context.input_kind, "from");
        assert_eq!(migration.context.input_path, normalize_path(&from));
        remove_test_dir(&root);
    }

    #[test]
    fn load_repo_policy_migration_config_call_presence_observer() {
        let root = unique_test_dir("migrate-load-repo-policy");
        let canonical_root = root
            .canonicalize()
            .unwrap_or_else(|err| std::panic::panic_any(format!("canonicalize root: {err}")));
        let policy_dir = root.join("policy");
        fs::create_dir_all(&policy_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
        fs::write(
            policy_dir.join("process-allowlist.toml"),
            process_policy_fixture_text(),
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("write process policy: {err}")));

        let migration = load_repo_policy_migration_config(Some(&root), Path::new("policy"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("load repo policy: {err}")));

        assert_eq!(migration.cfg.policy, "cargo-allow");
        assert_eq!(migration.cfg.allow.len(), 1);
        let entry = migration
            .cfg
            .allow
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected migrated process entry"));
        assert_eq!(entry.id, "proc-cargo-install-cargo-deny");
        assert_eq!(entry.kind, FindingKind::PolicyException);
        assert_eq!(migration.context.inventory_source, "filesystem_fallback");
        assert_eq!(
            migration.context.source_tree_root.as_deref(),
            Some(allow_report::source_tree_path_text(&canonical_root).as_str())
        );
        assert!(
            migration
                .context
                .inventory_files
                .is_some_and(|count| count >= 1),
            "repo-policy migration should report inventory file count"
        );
        assert_eq!(migration.context.input_kind, "repo_policy");
        assert_eq!(
            migration.context.input_path,
            normalize_path(canonical_root.join("policy"))
        );
        remove_test_dir(&root);
    }

    #[test]
    fn repo_policy_source_tree_root_call_presence_observer() {
        let explicit = unique_test_dir("migrate-load-explicit-root");
        let policy_parent = unique_test_dir("migrate-load-policy-parent");
        let policy_dir = policy_parent.join("policy");
        fs::create_dir_all(&policy_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));

        let explicit_result = repo_policy_source_tree_root(Some(&explicit), Path::new("ignored"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("explicit root: {err}")));
        assert_eq!(
            explicit_result,
            explicit.canonicalize().unwrap_or_else(|err| {
                std::panic::panic_any(format!("canonicalize explicit root: {err}"))
            })
        );

        let inferred = repo_policy_source_tree_root(None, &policy_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("infer policy parent: {err}")));
        assert_eq!(inferred, policy_parent);

        let fallback = repo_policy_source_tree_root(None, Path::new("not-policy-dir"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("fallback source root: {err}")));
        assert!(fallback.is_absolute());
        assert!(fallback.exists());
        remove_test_dir(&explicit);
        remove_test_dir(&policy_parent);
    }

    fn bespoke_ledger_fixture_text() -> &'static str {
        r#"
schema_version = 1
dialect = "xtask-ripr"

[[entries]]
id = "fixture-semantic-unwrap"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "parser"
reason = "Semantic selector pins unwrap on optional after validation."
selector = "method_call"
container = "load"
callee = "unwrap"
receiver = "optional_value"
"#
    }

    fn canonical_policy_config() -> AllowConfig {
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(AllowEntry {
            id: "allow-migrated-doc".to_string(),
            kind: FindingKind::NonRustFile,
            family: Some("documentation".to_string()),
            path: Some(PathBuf::from("README.md")),
            glob: None,
            owner: "docs".to_string(),
            classification: "reviewed_documentation".to_string(),
            reason: "Retained documentation file carried forward from legacy migration."
                .to_string(),
            evidence: Vec::new(),
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle {
                created: Some("2026-06-02".to_string()),
                review_after: Some("2026-11-01".to_string()),
                expires: None,
            },
            selector: Selector {
                ast_kind: Some("tracked_file".to_string()),
                symbol: Some("README.md".to_string()),
                target_fingerprint: Some("md".to_string()),
                line_hint: Some(1),
                ..Selector::default()
            },
            last_seen: None,
        });
        cfg
    }

    fn process_policy_fixture_text() -> &'static str {
        r#"schema_version = 1
policy = "process-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "proc-cargo-install-cargo-deny"
binary = "cargo"
argv_shape = ["install", "cargo-deny", "--locked"]
network_reach = true
called_by = [".github/workflows/ci.yml"]
owner = "release/ci"
reason = "Installs cargo-deny in the deny job."
created = "2026-05-09"
review_after = "2026-09-09"
"#
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
}
