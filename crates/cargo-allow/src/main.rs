use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, FindingKind, Lifecycle,
    Selector, normalize_path,
};
use allow_inventory::{
    InventoryOptions, discover_workspace_root, inventory_files, workspace_metadata,
};
use allow_match::{CheckMode, evaluate};
use allow_policy::{find_config, load_policy, render_policy, starter_policy};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::str::FromStr;

#[derive(Debug, Parser)]
#[command(
    name = "cargo-allow",
    about = "Source exception ledger for Rust workspaces",
    disable_version_flag = true
)]
struct CargoAllowCli {
    #[command(subcommand)]
    command: Option<CargoAllowCommand>,
}

#[derive(Debug, Subcommand)]
enum CargoAllowCommand {
    /// Create policy/allow.toml.
    Init(InitArgs),
    /// Inventory exceptions and policy health.
    Audit(ReportArgs),
    /// CI gate for the exception ledger.
    Check(CheckArgs),
    /// PR-oriented report with git changed files.
    Diff(DiffArgs),
    /// List allow entries.
    List(ListArgs),
    /// Explain one allow entry.
    Explain(ExplainArgs),
    /// Generate temporary baseline_debt entries.
    Propose(ProposeArgs),
    /// Convert compatible legacy policy files.
    Migrate(MigrateArgs),
    /// Dry-run stale cleanup guidance.
    Prune,
    /// Validate local setup.
    Doctor(ConfigArgs),
}

#[derive(Debug, Clone, Parser)]
struct ConfigArgs {
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
}

#[derive(Debug, Clone, Parser)]
struct InitArgs {
    /// Write strict-mode defaults.
    #[arg(long)]
    strict: bool,
    /// Overwrite an existing policy file.
    #[arg(long)]
    force: bool,
    /// Policy config path.
    #[arg(long, default_value = "policy/allow.toml")]
    config: PathBuf,
}

#[derive(Debug, Clone, Parser)]
struct ReportArgs {
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Filter findings by kind.
    #[arg(long)]
    kind: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    format: OutputFormat,
    /// Write report to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Clone, Parser)]
struct CheckArgs {
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Filter findings by kind.
    #[arg(long)]
    kind: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    format: OutputFormat,
    /// Write report to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,
    /// Write machine-readable receipt to a file.
    #[arg(long)]
    receipt: Option<PathBuf>,
    /// Check mode.
    #[arg(long, default_value = "no-new", value_parser = ["audit", "no-new", "strict", "release"])]
    mode: String,
}

#[derive(Debug, Clone, Parser)]
struct DiffArgs {
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Filter findings by kind.
    #[arg(long)]
    kind: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    format: OutputFormat,
    /// Write report to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,
    /// Base git revision for changed-file listing.
    #[arg(long)]
    base: String,
    /// Optional head git revision.
    #[arg(long)]
    head: Option<String>,
}

#[derive(Debug, Clone, Parser)]
struct ListArgs {
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Filter allow entries by kind.
    #[arg(long)]
    kind: Option<String>,
}

#[derive(Debug, Clone, Parser)]
struct ExplainArgs {
    /// Allow entry ID.
    id: String,
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
}

#[derive(Debug, Clone, Parser)]
struct ProposeArgs {
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Filter findings by kind.
    #[arg(long)]
    kind: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    include_untracked: bool,
    /// Expiry date for generated baseline_debt entries.
    #[arg(long, default_value = "2026-08-01")]
    expires: String,
    /// Write proposed policy to this path.
    #[arg(long)]
    write: Option<PathBuf>,
}

#[derive(Debug, Clone, Parser)]
struct MigrateArgs {
    /// Legacy or canonical policy file to migrate.
    #[arg(long)]
    from: PathBuf,
    /// Output canonical policy path.
    #[arg(long, default_value = "policy/allow.toml")]
    out: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
    #[value(alias = "md")]
    Markdown,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(2);
    }
}

fn run() -> CargoAllowResult<()> {
    let cli = CargoAllowCli::parse_from(normalized_args(env::args()));
    let Some(command) = cli.command else {
        CargoAllowCli::command()
            .print_help()
            .map_err(|e| CargoAllowError::new(format!("failed to print help: {e}")))?;
        println!();
        return Ok(());
    };
    match command {
        CargoAllowCommand::Init(args) => cmd_init(&args),
        CargoAllowCommand::Audit(args) => cmd_audit(&args),
        CargoAllowCommand::Check(args) => cmd_check(&args),
        CargoAllowCommand::Diff(args) => cmd_diff(&args),
        CargoAllowCommand::List(args) => cmd_list(&args),
        CargoAllowCommand::Explain(args) => cmd_explain(&args),
        CargoAllowCommand::Propose(args) => cmd_propose(&args),
        CargoAllowCommand::Migrate(args) => cmd_migrate(&args),
        CargoAllowCommand::Prune => cmd_prune(),
        CargoAllowCommand::Doctor(args) => cmd_doctor(&args),
    }
}

fn normalized_args(args: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut args = args.into_iter().collect::<Vec<_>>();
    if args.get(1).map(|s| s.as_str()) == Some("allow") {
        args.remove(1);
    }
    args
}

fn cmd_init(args: &InitArgs) -> CargoAllowResult<()> {
    let path = args.config.clone();
    if path.exists() && !args.force {
        return Err(CargoAllowError::new(format!(
            "{} already exists; use --force to overwrite",
            path.display()
        )));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            CargoAllowError::new(format!("failed to create {}: {e}", parent.display()))
        })?;
    }
    fs::write(&path, starter_policy(args.strict))
        .map_err(|e| CargoAllowError::new(format!("failed to write {}: {e}", path.display())))?;
    println!("created {}", path.display());
    Ok(())
}

fn cmd_audit(args: &ReportArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings) = load_world(
        args.config.as_deref(),
        false,
        args.kind.as_deref(),
        args.include_untracked,
    )?;
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let outcomes = evaluate(&report_cfg, &findings, CheckMode::Audit);
    print_report(
        "audit",
        args.format,
        &findings,
        &outcomes,
        false,
        args.output.as_deref(),
    )?;
    eprintln!("workspace: {}", root.display());
    Ok(())
}

fn cmd_check(args: &CheckArgs) -> CargoAllowResult<()> {
    let mode = CheckMode::parse(&args.mode);
    let (_root, cfg, findings) = load_world(
        args.config.as_deref(),
        true,
        args.kind.as_deref(),
        args.include_untracked,
    )?;
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let outcomes = evaluate(&report_cfg, &findings, mode);
    let failed = outcomes.iter().any(|o| mode.fails(o.status));
    print_report(
        "check",
        args.format,
        &findings,
        &outcomes,
        failed,
        args.output.as_deref(),
    )?;
    if let Some(path) = &args.receipt {
        write_file(
            path,
            &allow_report::render_receipt("check", &outcomes, failed),
        )?;
    }
    if failed {
        process::exit(1);
    }
    Ok(())
}

fn cmd_diff(args: &DiffArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings) = load_world(
        args.config.as_deref(),
        true,
        args.kind.as_deref(),
        args.include_untracked,
    )?;
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let outcomes = evaluate(&report_cfg, &findings, CheckMode::NoNew);
    let failed = outcomes.iter().any(|o| CheckMode::NoNew.fails(o.status));
    let mut text = match args.format {
        OutputFormat::Json => allow_report::render_json("diff", &findings, &outcomes, failed),
        OutputFormat::Markdown => {
            allow_report::render_markdown("diff", &findings, &outcomes, failed)
        }
        OutputFormat::Human => allow_report::render_human("diff", &findings, &outcomes, failed),
    };
    match allow_diff::changed_files(&root, &args.base, args.head.as_deref()) {
        Ok(changed) => {
            if args.format == OutputFormat::Human {
                text.push_str("\nChanged files from git diff:\n");
                for path in changed.iter().take(80) {
                    text.push_str(&format!("  {}\n", normalize_path(path)));
                }
            }
        }
        Err(err) => {
            if args.format == OutputFormat::Human {
                text.push_str(&format!("\nwarning: could not compute git diff: {err}\n"));
            }
        }
    }
    if let Some(path) = &args.output {
        write_file(path, &text)?;
    } else {
        println!("{text}");
    }
    if failed {
        process::exit(1);
    }
    Ok(())
}

fn cmd_list(args: &ListArgs) -> CargoAllowResult<()> {
    let cfg = load_config_required(args.config.as_deref())?;
    let parsed_kind = args
        .kind
        .as_deref()
        .map(FindingKind::from_str)
        .transpose()?;
    for entry in cfg
        .allow
        .iter()
        .filter(|e| parsed_kind.map(|kind| e.kind == kind).unwrap_or(true))
    {
        println!(
            "{}\t{}\t{}\t{}",
            entry.id,
            entry.kind,
            entry.path_or_glob(),
            entry.reason
        );
    }
    Ok(())
}

fn cmd_explain(args: &ExplainArgs) -> CargoAllowResult<()> {
    let cfg = load_config_required(args.config.as_deref())?;
    let entry = cfg
        .allow
        .iter()
        .find(|e| e.id == args.id)
        .ok_or_else(|| CargoAllowError::new(format!("no allow entry `{}`", args.id)))?;
    println!("{}", entry.id);
    println!(
        "kind: {}{}",
        entry.kind,
        entry
            .family
            .as_ref()
            .map(|f| format!(".{f}"))
            .unwrap_or_default()
    );
    println!("scope: {}", entry.path_or_glob());
    println!("owner: {}", entry.owner);
    println!("classification: {}", entry.classification);
    println!("reason: {}", entry.reason);
    if !entry.evidence.is_empty() {
        println!("evidence: {}", entry.evidence.join(", "));
    }
    if let Some(expires) = &entry.lifecycle.expires {
        println!("expires: {expires}");
    }
    if let Some(review_after) = &entry.lifecycle.review_after {
        println!("review_after: {review_after}");
    }
    Ok(())
}

fn cmd_propose(args: &ProposeArgs) -> CargoAllowResult<()> {
    let (_root, cfg, findings) = load_world(
        args.config.as_deref(),
        false,
        args.kind.as_deref(),
        args.include_untracked,
    )?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::Audit);
    let mut proposed = cfg.clone();
    let start = proposed.allow.len() + 1;
    for (n, outcome) in outcomes
        .iter()
        .filter(|o| o.status == allow_core::MatchStatus::New)
        .enumerate()
    {
        if let Some(finding) = outcome.finding_index.and_then(|idx| findings.get(idx)) {
            proposed
                .allow
                .push(entry_from_finding(finding, start + n, &args.expires));
        }
    }
    let rendered = render_policy(&proposed);
    if let Some(path) = &args.write {
        write_file(path, &rendered)?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}

fn cmd_migrate(args: &MigrateArgs) -> CargoAllowResult<()> {
    let cfg = allow_policy_legacy::load_legacy_or_canonical(&args.from)?;
    write_file(&args.out, &render_policy(&cfg))?;
    eprintln!("{}", allow_policy_legacy::migration_notes());
    Ok(())
}

fn cmd_prune() -> CargoAllowResult<()> {
    println!(
        "prune MVP is dry-run only. Use check/audit stale results, then edit policy/allow.toml."
    );
    Ok(())
}

fn cmd_doctor(args: &ConfigArgs) -> CargoAllowResult<()> {
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    let metadata = workspace_metadata(&cwd).ok();
    let root = metadata
        .as_ref()
        .map(|metadata| metadata.root.clone())
        .map(Ok)
        .unwrap_or_else(|| discover_workspace_root(&cwd))?;
    println!("workspace root: {}", root.display());
    if let Some(metadata) = &metadata {
        println!("workspace packages: {}", metadata.packages.len());
        println!("workspace targets: {}", metadata.target_count());
        println!("source roots: {}", metadata.source_roots().len());
    } else {
        println!("workspace metadata: unavailable");
    }
    match config_path(args.config.as_deref()) {
        Some(path) => println!("config: {}", path.display()),
        None => println!("config: not found; run `cargo allow init`"),
    }
    let opts = InventoryOptions::default();
    let files = inventory_files(&root, &opts)?;
    println!("tracked/scanned files: {}", files.len());
    println!(
        "claim boundary: source syntax only; macro expansion and type information are not analyzed"
    );
    Ok(())
}

fn load_world(
    config: Option<&Path>,
    require_config: bool,
    kind_filter: Option<&str>,
    include_untracked: bool,
) -> CargoAllowResult<(PathBuf, AllowConfig, Vec<Finding>)> {
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    let root = discover_workspace_root(cwd)?;
    let cfg = if require_config {
        load_config_required(config)?
    } else {
        load_config_optional(config)?.unwrap_or_else(AllowConfig::empty)
    };
    let opts = InventoryOptions {
        ignored: cfg.workspace.ignored.clone(),
        generated: cfg.workspace.generated.clone(),
        include_untracked,
    };
    let files = inventory_files(&root, &opts)?;
    let mut findings = Vec::new();
    findings.extend(allow_rust::scan_rust_files(&root, &files)?);
    findings.extend(allow_files::scan_files_with_options(
        &files,
        &allow_files::FileScanOptions {
            generated: opts.generated.clone(),
        },
    ));
    if let Some(kind) = kind_filter {
        let parsed = FindingKind::from_str(kind)?;
        findings.retain(|f| {
            f.kind == parsed || (parsed == FindingKind::Panic && f.kind == FindingKind::Panic)
        });
    }
    Ok((root, cfg, findings))
}

fn report_config(cfg: &AllowConfig, kind_filter: Option<&str>) -> CargoAllowResult<AllowConfig> {
    let Some(kind) = kind_filter else {
        return Ok(cfg.clone());
    };
    let parsed = FindingKind::from_str(kind)?;
    let mut filtered = cfg.clone();
    filtered.allow.retain(|entry| entry.kind == parsed);
    Ok(filtered)
}

fn load_config_required(config: Option<&Path>) -> CargoAllowResult<AllowConfig> {
    let path = config_path(config).ok_or_else(|| {
        CargoAllowError::new("no policy config found; run `cargo allow init` or pass --config")
    })?;
    load_policy(path)
}

fn load_config_optional(config: Option<&Path>) -> CargoAllowResult<Option<AllowConfig>> {
    match config_path(config) {
        Some(path) => Ok(Some(load_policy(path)?)),
        None => Ok(None),
    }
}

fn config_path(config: Option<&Path>) -> Option<PathBuf> {
    config
        .map(PathBuf::from)
        .or_else(|| find_config(env::current_dir().ok()?))
}

fn print_report(
    command: &str,
    format: OutputFormat,
    findings: &[Finding],
    outcomes: &[allow_core::MatchOutcome],
    failed: bool,
    output: Option<&Path>,
) -> CargoAllowResult<()> {
    let text = match format {
        OutputFormat::Human => allow_report::render_human(command, findings, outcomes, failed),
        OutputFormat::Json => allow_report::render_json(command, findings, outcomes, failed),
        OutputFormat::Markdown => {
            allow_report::render_markdown(command, findings, outcomes, failed)
        }
    };
    if let Some(path) = output {
        write_file(path, &text)?;
    } else {
        println!("{text}");
    }
    Ok(())
}

fn entry_from_finding(finding: &Finding, index: usize, expires: &str) -> AllowEntry {
    let selector = Selector {
        ast_kind: Some(finding.identity.ast_kind.clone()),
        container: finding.identity.container.clone(),
        callee: finding.identity.callee.clone(),
        macro_name: finding.identity.macro_name.clone(),
        lint: finding.identity.lint.clone(),
        symbol: finding.identity.symbol.clone(),
        receiver_fingerprint: finding.identity.receiver_fingerprint.clone(),
        target_fingerprint: finding.identity.target_fingerprint.clone(),
        normalized_snippet_hash: finding.identity.normalized_snippet_hash.clone(),
        line_hint: finding.span.as_ref().map(|s| s.line),
        glob: matches!(
            finding.kind,
            FindingKind::NonRustFile | FindingKind::GeneratedCode
        )
        .then(|| normalize_path(&finding.path)),
    };
    AllowEntry {
        id: format!("allow-{index:04}"),
        kind: finding.kind,
        family: finding.family.clone(),
        path: Some(finding.path.clone()),
        glob: None,
        owner: "unowned".to_string(),
        classification: "baseline_debt".to_string(),
        reason: "Generated by cargo allow propose; requires human review.".to_string(),
        evidence: if finding.kind == FindingKind::Unsafe {
            vec!["TODO: add unsafe-review or boundary-test evidence".to_string()]
        } else {
            Vec::new()
        },
        links: Vec::new(),
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: None,
            expires: Some(expires.to_string()),
        },
        selector,
        last_seen: finding.span.as_ref().map(|s| allow_core::LastSeen {
            line: s.line,
            column: s.column,
        }),
    }
}

fn write_file(path: impl AsRef<Path>, contents: &str) -> CargoAllowResult<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            CargoAllowError::new(format!("failed to create {}: {e}", parent.display()))
        })?;
    }
    fs::write(path, contents)
        .map_err(|e| CargoAllowError::new(format!("failed to write {}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_args_accepts_cargo_subcommand_prefix() {
        let normalized = normalized_args(argv(vec!["cargo-allow", "allow", "audit"]));
        let expected = argv(vec!["cargo-allow", "audit"]);
        assert_eq!(normalized, expected);
    }

    #[test]
    fn clap_parses_markdown_alias() {
        let parsed =
            CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "check", "--format", "md"]))
                .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Check(CheckArgs {
                format: OutputFormat::Markdown,
                ..
            }))
        ));
    }

    #[test]
    fn clap_requires_diff_base() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "diff"]));

        assert!(parsed.is_err());
    }

    #[test]
    fn clap_parses_explain_id_and_config() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "explain",
            "allow-0001",
            "--config",
            "policy/custom.toml",
        ]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Explain(ExplainArgs { id, config }))
                if id == "allow-0001" && config.as_deref() == Some(Path::new("policy/custom.toml"))
        ));
    }

    #[test]
    fn clap_parses_include_untracked_audit_flag() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "audit",
            "--include-untracked",
        ]))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("CLI should parse include-untracked: {err}"))
        });

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Audit(ReportArgs {
                include_untracked: true,
                ..
            }))
        ));
    }

    #[test]
    fn report_config_filters_allow_entries_by_kind() {
        let mut cfg = AllowConfig::empty();
        cfg.allow
            .push(test_entry("allow-file", FindingKind::NonRustFile));
        cfg.allow
            .push(test_entry("allow-panic", FindingKind::Panic));

        let filtered = report_config(&cfg, Some("non-rust")).unwrap_or_else(|err| {
            std::panic::panic_any(format!("kind filter should parse: {err}"))
        });

        assert_eq!(filtered.allow.len(), 1);
        assert!(
            filtered
                .allow
                .iter()
                .any(|entry| entry.id == "allow-file" && entry.kind == FindingKind::NonRustFile)
        );
    }

    fn argv(items: Vec<&str>) -> Vec<String> {
        items.into_iter().map(String::from).collect()
    }

    fn test_entry(id: &str, kind: FindingKind) -> AllowEntry {
        AllowEntry {
            id: id.to_string(),
            kind,
            family: None,
            path: Some(PathBuf::from("tracked.file")),
            glob: None,
            owner: "owner".to_string(),
            classification: "classification".to_string(),
            reason: "reason".to_string(),
            evidence: Vec::new(),
            links: Vec::new(),
            lifecycle: Lifecycle::empty(),
            selector: Selector {
                ast_kind: Some("tracked_file".to_string()),
                ..Selector::default()
            },
            last_seen: None,
        }
    }
}
