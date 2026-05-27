use allow_core::{
    AllowConfig, CargoAllowError, CargoAllowResult, FindingKind, json_escape, normalize_path,
};
use allow_inventory::{InventoryOptions, inventory, resolve_source_tree_root};
use allow_policy::{render_policy, validate_policy};
use clap::{Parser, ValueEnum};
use std::env;
use std::path::{Path, PathBuf};

use crate::{
    RootArgs, root_relative_path, source_tree_root_text, write_file, write_file_no_overwrite,
};

#[derive(Debug, Clone, Parser)]
pub(crate) struct MigrateArgs {
    #[command(flatten)]
    root: RootArgs,
    /// Legacy or canonical policy file to migrate.
    #[arg(long)]
    from: Option<PathBuf>,
    /// Directory containing compatible legacy policy files.
    #[arg(long)]
    repo_policy: Option<PathBuf>,
    /// Output canonical policy path.
    #[arg(long, default_value = "policy/allow.toml")]
    out: PathBuf,
    /// Overwrite an existing output policy file.
    #[arg(long)]
    force: bool,
    /// Summary output format. Policy output remains TOML.
    #[arg(long, value_enum, default_value_t = MigrateSummaryFormat::Human)]
    summary_format: MigrateSummaryFormat,
    /// Write migration summary to a file instead of stderr.
    #[arg(long)]
    summary_output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum MigrateSummaryFormat {
    Human,
    Json,
}

pub(crate) fn cmd_migrate(args: &MigrateArgs) -> CargoAllowResult<()> {
    let migration = match (&args.from, &args.repo_policy) {
        (Some(from), None) => MigrationLoad {
            cfg: allow_policy_legacy::load_legacy_or_canonical(from)?,
            context: MigrateContext {
                inventory_source: "unknown".to_string(),
                source_tree_root: None,
                inventory_files: None,
                input_kind: "from".to_string(),
                input_path: normalize_path(from),
            },
        },
        (None, Some(repo_policy)) => {
            load_repo_policy_migration_config(args.root.root.as_deref(), repo_policy)?
        }
        (Some(_), Some(_)) => {
            return Err(CargoAllowError::new(
                "pass either --from or --repo-policy, not both",
            ));
        }
        (None, None) => {
            return Err(CargoAllowError::new(
                "pass --from <file> or --repo-policy <dir>",
            ));
        }
    };
    let cfg = migration.cfg;
    validate_policy(&cfg)?;
    write_file_no_overwrite(&args.out, &render_policy(&cfg), args.force)?;
    let summary = match args.summary_format {
        MigrateSummaryFormat::Human => {
            render_migrate_summary(&cfg, &migration.context, &args.out, args.force)
        }
        MigrateSummaryFormat::Json => {
            render_migrate_summary_json(&cfg, &migration.context, &args.out, args.force)
        }
    };
    if let Some(path) = &args.summary_output {
        write_file(path, &summary)?;
    } else {
        eprintln!("{summary}");
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct MigrationLoad {
    cfg: AllowConfig,
    context: MigrateContext,
}

#[derive(Debug, Clone)]
struct MigrateContext {
    inventory_source: String,
    source_tree_root: Option<String>,
    inventory_files: Option<usize>,
    input_kind: String,
    input_path: String,
}

fn load_repo_policy_migration_config(
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
    let cfg = allow_policy_legacy::load_legacy_policy_dir_with_non_rust_findings(
        &repo_policy,
        &findings,
    )?;
    Ok(MigrationLoad {
        cfg,
        context: MigrateContext {
            inventory_source: inventory_source.as_str().to_string(),
            source_tree_root: Some(source_tree_root_text(&root)),
            inventory_files: Some(files_scanned),
            input_kind: "repo_policy".to_string(),
            input_path: normalize_path(&repo_policy),
        },
    })
}

fn render_migrate_summary(
    cfg: &AllowConfig,
    context: &MigrateContext,
    output: &Path,
    force: bool,
) -> String {
    let counts = migrate_summary_counts(cfg);
    let notes = allow_policy_legacy::migration_notes();
    let mut out = String::new();
    out.push_str("cargo-allow migrate summary\n");
    out.push_str(&format!("input_kind: {}\n", context.input_kind));
    out.push_str(&format!("input: {}\n", context.input_path));
    out.push_str(&format!("output: {}\n", output.display()));
    out.push_str(&format!("force: {force}\n"));
    out.push_str(&format!("allow_entries: {}\n", counts.allow_entries));
    out.push_str(&format!("baseline_debt: {}\n", counts.baseline_debt));
    out.push_str(&format!("unsafe_entries: {}\n", counts.unsafe_entries));
    if let Some(root) = &context.source_tree_root {
        out.push_str(&format!("source_tree_root: {root}\n"));
    }
    out.push_str(&format!("inventory_source: {}\n", context.inventory_source));
    if let Some(files) = context.inventory_files {
        out.push_str(&format!("files_scanned: {files}\n"));
    }
    out.push_str(notes);
    out
}

fn render_migrate_summary_json(
    cfg: &AllowConfig,
    context: &MigrateContext,
    output: &Path,
    force: bool,
) -> String {
    let counts = migrate_summary_counts(cfg);
    let output = output.display().to_string();
    let notes = allow_policy_legacy::migration_notes();
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": {},\n",
        allow_report::MIGRATE_SCHEMA_VERSION
    ));
    out.push_str(&format!(
        "  \"schema_id\": \"{}\",\n",
        allow_report::MIGRATE_SCHEMA_ID
    ));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str("  \"command\": \"migrate\",\n");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        allow_report::render_claim_boundary_json()
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        allow_report::render_scanner_limitations_json()
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&allow_report::render_inventory_json(
        allow_report::InventoryContext::new(
            "source_tree",
            "policy_migration",
            &context.inventory_source,
            context.source_tree_root.as_deref(),
            context.inventory_files,
        ),
        "  ",
    ));
    out.push_str(",\n");
    out.push_str("  \"input\": {\n");
    out.push_str(&format!(
        "    \"kind\": \"{}\",\n",
        json_escape(&context.input_kind)
    ));
    out.push_str(&format!(
        "    \"path\": \"{}\"\n",
        json_escape(&context.input_path)
    ));
    out.push_str("  },\n");
    out.push_str("  \"output\": {\n");
    out.push_str(&format!("    \"path\": \"{}\",\n", json_escape(&output)));
    out.push_str(&format!("    \"force\": {}\n", force));
    out.push_str("  },\n");
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!(
        "    \"allow_entries\": {},\n",
        counts.allow_entries
    ));
    out.push_str(&format!(
        "    \"baseline_debt\": {},\n",
        counts.baseline_debt
    ));
    out.push_str(&format!(
        "    \"unsafe_entries\": {},\n",
        counts.unsafe_entries
    ));
    out.push_str(&format!(
        "    \"entries_with_evidence\": {}\n",
        counts.entries_with_evidence
    ));
    out.push_str("  },\n");
    out.push_str(&format!("  \"notes\": \"{}\"\n", json_escape(notes)));
    out.push_str("}\n");
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MigrateSummaryCounts {
    allow_entries: usize,
    baseline_debt: usize,
    unsafe_entries: usize,
    entries_with_evidence: usize,
}

fn migrate_summary_counts(cfg: &AllowConfig) -> MigrateSummaryCounts {
    MigrateSummaryCounts {
        allow_entries: cfg.allow.len(),
        baseline_debt: cfg
            .allow
            .iter()
            .filter(|entry| entry.classification == "baseline_debt")
            .count(),
        unsafe_entries: cfg
            .allow
            .iter()
            .filter(|entry| entry.kind == FindingKind::Unsafe)
            .count(),
        entries_with_evidence: cfg
            .allow
            .iter()
            .filter(|entry| !entry.evidence.is_empty())
            .count(),
    }
}

fn repo_policy_source_tree_root(
    explicit_root: Option<&Path>,
    repo_policy: &Path,
) -> CargoAllowResult<PathBuf> {
    if let Some(root) = explicit_root {
        return resolve_source_tree_root(Some(root), root);
    }
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
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

#[cfg(test)]
pub(crate) fn sample_migrate_json_for_contract_test() -> String {
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(allow_core::AllowEntry {
        id: "allow-migrate".to_string(),
        kind: FindingKind::NonRustFile,
        family: None,
        path: Some("src/lib.rs".into()),
        glob: None,
        owner: "team".to_string(),
        classification: "reviewed".to_string(),
        reason: "test".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: allow_core::Lifecycle::empty(),
        selector: allow_core::Selector::default(),
        last_seen: None,
    });
    render_migrate_summary_json(
        &cfg,
        &MigrateContext {
            inventory_source: "unknown".to_string(),
            source_tree_root: None,
            inventory_files: None,
            input_kind: "from".to_string(),
            input_path: "policy/legacy.toml".to_string(),
        },
        Path::new("policy/allow.toml"),
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CargoAllowCli, CargoAllowCommand};
    use allow_core::{AllowEntry, Lifecycle, Selector};
    use clap::Parser;
    use serde_json::Value;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn render_migrate_summary_json_records_policy_migration_context() {
        let mut cfg = AllowConfig::empty();
        let mut baseline = test_entry("allow-baseline", FindingKind::Panic);
        baseline.classification = "baseline_debt".to_string();
        let mut unsafe_entry = test_entry("allow-unsafe", FindingKind::Unsafe);
        unsafe_entry.evidence = vec!["unsafe-review:docs/evidence/unsafe.json".to_string()];
        cfg.allow.push(baseline);
        cfg.allow.push(unsafe_entry);
        let context = MigrateContext {
            inventory_source: "git_tracked".to_string(),
            source_tree_root: Some("H:/Code/Rust/cargo-allow".to_string()),
            inventory_files: Some(53),
            input_kind: "repo_policy".to_string(),
            input_path: "policy".to_string(),
        };

        let json =
            render_migrate_summary_json(&cfg, &context, Path::new("policy/allow.toml"), true);
        let value =
            parse_json_artifact("migrate", &json, allow_report::MIGRATE_SCHEMA_ID, "migrate");

        assert_inventory_contract(
            "migrate",
            &value,
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(53),
        );
        assert_eq!(
            value.pointer("/inventory/scanner").and_then(Value::as_str),
            Some("policy_migration"),
            "migrate scanner"
        );
        assert_eq!(
            value.pointer("/input/kind").and_then(Value::as_str),
            Some("repo_policy"),
            "migrate input kind"
        );
        assert_eq!(
            value.pointer("/output/path").and_then(Value::as_str),
            Some("policy/allow.toml"),
            "migrate output path"
        );
        assert_eq!(
            value.pointer("/output/force").and_then(Value::as_bool),
            Some(true),
            "migrate force"
        );
        assert_eq!(
            value
                .pointer("/summary/allow_entries")
                .and_then(Value::as_u64),
            Some(2),
            "migrate allow entries"
        );
        assert_eq!(
            value
                .pointer("/summary/baseline_debt")
                .and_then(Value::as_u64),
            Some(1),
            "migrate baseline debt"
        );
        assert_eq!(
            value
                .pointer("/summary/unsafe_entries")
                .and_then(Value::as_u64),
            Some(1),
            "migrate unsafe entries"
        );
        assert_eq!(
            value
                .pointer("/summary/entries_with_evidence")
                .and_then(Value::as_u64),
            Some(1),
            "migrate evidence entries"
        );
        assert!(json.contains("policy_migration"));
    }

    #[test]
    fn clap_parses_repo_policy_migrate() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "migrate",
            "--repo-policy",
            "policy",
            "--out",
            "target/allow.toml",
            "--force",
            "--summary-format",
            "json",
            "--summary-output",
            "target/migrate-summary.json",
        ]))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("CLI should parse repo-policy migrate: {err}"))
        });

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Migrate(MigrateArgs {
                repo_policy: Some(dir),
                out,
                force: true,
                summary_format: MigrateSummaryFormat::Json,
                summary_output: Some(summary_output),
                ..
            })) if dir == Path::new("policy")
                && out == Path::new("target/allow.toml")
                && summary_output == Path::new("target/migrate-summary.json")
        ));
    }

    #[test]
    fn migrate_requires_one_input_source() {
        let missing = cmd_migrate(&MigrateArgs {
            root: RootArgs::default(),
            from: None,
            repo_policy: None,
            out: PathBuf::from("target/unused.toml"),
            force: false,
            summary_format: MigrateSummaryFormat::Human,
            summary_output: None,
        })
        .expect_err("missing input source should fail");
        assert!(
            missing
                .to_string()
                .contains("pass --from <file> or --repo-policy <dir>")
        );

        let conflicting = cmd_migrate(&MigrateArgs {
            root: RootArgs::default(),
            from: Some(PathBuf::from("legacy.toml")),
            repo_policy: Some(PathBuf::from("policy")),
            out: PathBuf::from("target/unused.toml"),
            force: false,
            summary_format: MigrateSummaryFormat::Human,
            summary_output: None,
        })
        .expect_err("conflicting input sources should fail");
        assert!(
            conflicting
                .to_string()
                .contains("pass either --from or --repo-policy")
        );
    }

    #[test]
    fn migrate_refuses_existing_output_without_force() {
        let dir = migrate_fixture_dir();
        let policy_dir = dir.join("policy");
        fs::create_dir_all(&policy_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
        fs::write(
            policy_dir.join("network-allowlist.toml"),
            network_policy_fixture_text(),
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("network fixture write: {err}")));
        let out = dir.join("allow.toml");
        fs::write(&out, "existing")
            .unwrap_or_else(|err| std::panic::panic_any(format!("existing output write: {err}")));

        let err = cmd_migrate(&MigrateArgs {
            root: RootArgs::default(),
            from: None,
            repo_policy: Some(policy_dir),
            out,
            force: false,
            summary_format: MigrateSummaryFormat::Human,
            summary_output: None,
        })
        .expect_err("existing output should require --force");
        assert!(err.to_string().contains("use --force to overwrite"));
    }

    #[test]
    fn migrate_repo_policy_writes_combined_canonical_policy() {
        let dir = migrate_fixture_dir();
        let policy_dir = dir.join("policy");
        fs::create_dir_all(&policy_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
        fs::write(
            policy_dir.join("process-allowlist.toml"),
            process_policy_fixture_text(),
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("process fixture write: {err}")));
        fs::write(
            policy_dir.join("network-allowlist.toml"),
            network_policy_fixture_text(),
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("network fixture write: {err}")));
        let out = dir.join("allow.toml");

        cmd_migrate(&MigrateArgs {
            root: RootArgs::default(),
            from: None,
            repo_policy: Some(policy_dir),
            out: out.clone(),
            force: false,
            summary_format: MigrateSummaryFormat::Human,
            summary_output: None,
        })
        .unwrap_or_else(|err| std::panic::panic_any(format!("repo-policy migrate: {err}")));

        let rendered = fs::read_to_string(&out)
            .unwrap_or_else(|err| std::panic::panic_any(format!("read migrated policy: {err}")));
        assert!(rendered.contains("process_spawn"));
        assert!(rendered.contains("network_destination"));
    }

    #[test]
    fn migrate_schema_documents_current_contract() {
        let schema = include_str!("../../../docs/schemas/migrate.schema.json");

        assert!(schema.contains(allow_report::MIGRATE_SCHEMA_ID));
        assert!(schema.contains("\"policy_migration\""));
        assert!(schema.contains("\"input\""));
        assert!(schema.contains("\"output\""));
        assert!(schema.contains("\"allow_entries\""));
        assert!(schema.contains("\"baseline_debt\""));
        assert!(schema.contains("\"unsafe_entries\""));
        assert!(schema.contains("\"entries_with_evidence\""));
        assert!(schema.contains("\"scanner_limitations\""));
        assert!(schema.contains("\"scanner_limitation\""));
        assert!(schema.contains("\"cargo_metadata_not_invoked\""));
        assert!(schema.contains("\"repository_code_not_executed\""));
    }

    fn parse_json_artifact(
        name: &str,
        json: &str,
        expected_schema: &str,
        expected_command: &str,
    ) -> Value {
        let value: Value = serde_json::from_str(json)
            .unwrap_or_else(|err| std::panic::panic_any(format!("{name} json: {err}\n{json}")));
        assert_eq!(
            value.pointer("/schema_id").and_then(Value::as_str),
            Some(expected_schema),
            "{name} schema id"
        );
        assert_eq!(
            value.pointer("/command").and_then(Value::as_str),
            Some(expected_command),
            "{name} command"
        );
        value
    }

    fn assert_inventory_contract(
        name: &str,
        value: &Value,
        expected_source: &str,
        expected_root: Option<&str>,
        expected_files: Option<u64>,
    ) {
        assert_eq!(
            value.pointer("/inventory/scope").and_then(Value::as_str),
            Some("source_tree"),
            "{name} inventory scope"
        );
        assert_eq!(
            value.pointer("/inventory/source").and_then(Value::as_str),
            Some(expected_source),
            "{name} inventory source"
        );
        assert_eq!(
            value.pointer("/inventory/root").and_then(Value::as_str),
            expected_root,
            "{name} inventory root"
        );
        assert_eq!(
            value
                .pointer("/inventory/files_scanned")
                .and_then(Value::as_u64),
            expected_files,
            "{name} inventory files"
        );
    }

    fn test_entry(id: &str, kind: FindingKind) -> AllowEntry {
        AllowEntry {
            id: id.to_string(),
            kind,
            family: None,
            path: Some("src/lib.rs".into()),
            glob: None,
            owner: "team".to_string(),
            classification: "reviewed".to_string(),
            reason: "test".to_string(),
            evidence: Vec::new(),
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle::empty(),
            selector: Selector::default(),
            last_seen: None,
        }
    }

    fn migrate_fixture_dir() -> PathBuf {
        static NEXT_MIGRATE_FIXTURE: AtomicUsize = AtomicUsize::new(0);
        let id = NEXT_MIGRATE_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "cargo-allow-cli-migrate-{}-{stamp}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture dir: {err}")));
        dir
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

    fn network_policy_fixture_text() -> &'static str {
        r#"schema_version = 1
policy = "network-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "net-crates-io-fetch"
destination = "crates.io"
auth_required = false
lane = "build"
owner = "release"
reason = "cargo fetch resolves and downloads crate dependencies."
created = "2026-05-09"
expires = "permanent"
"#
    }

    fn argv(items: Vec<&str>) -> Vec<String> {
        items.into_iter().map(String::from).collect()
    }
}
