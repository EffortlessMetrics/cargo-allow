use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, FindingKind, Lifecycle,
    MatchOutcome, MatchStatus, Selector, SimpleDate, glob_matches_str, json_escape, normalize_path,
};
use allow_inventory::{
    InventoryOptions, InventorySource, inventory, inventory_files, resolve_source_tree_root,
};
use allow_match::{CheckMode, evaluate, finding_location, score_match};
use allow_policy::{
    evidence_reference_diagnostics, find_config, load_policy, render_policy, starter_policy,
    validate_local_evidence_references, validate_policy,
};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::str::FromStr;

#[derive(Debug, Parser)]
#[command(
    name = "cargo-allow",
    about = "Source exception ledger for source trees",
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
    /// Generate an allow entry from a current finding.
    Add(AddArgs),
    /// Generate temporary baseline_debt entries.
    Propose(ProposeArgs),
    /// Emit actionable work items for humans or agents.
    Worklist(WorklistArgs),
    /// Convert compatible legacy policy files.
    Migrate(MigrateArgs),
    /// Preview or remove stale allow entries.
    Prune(PruneArgs),
    /// Validate local setup.
    Doctor(ConfigArgs),
}

#[derive(Debug, Clone, Default, Args)]
struct RootArgs {
    /// Source tree root. Defaults to the nearest git root, then current directory.
    #[arg(long)]
    root: Option<PathBuf>,
}

#[derive(Debug, Clone, Parser)]
struct ConfigArgs {
    #[command(flatten)]
    root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = DoctorFormat::Human)]
    format: DoctorFormat,
    /// Write doctor output to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InventoryFacts {
    source: InventorySource,
    files_scanned: Option<usize>,
}

impl InventoryFacts {
    fn source_only(source: InventorySource) -> Self {
        Self {
            source,
            files_scanned: None,
        }
    }

    fn scanned(source: InventorySource, files_scanned: usize) -> Self {
        Self {
            source,
            files_scanned: Some(files_scanned),
        }
    }
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
    #[command(flatten)]
    root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Use a compatible legacy policy for the selected kind.
    #[arg(long)]
    compat: bool,
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
    #[command(flatten)]
    root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Use a compatible legacy policy for the selected kind.
    #[arg(long)]
    compat: bool,
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
    #[command(flatten)]
    root: RootArgs,
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
    #[command(flatten)]
    root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Filter allow entries by kind.
    #[arg(long)]
    kind: Option<String>,
    /// Filter allow entries by scanner or policy family.
    #[arg(long)]
    family: Option<String>,
    /// Filter allow entries by owner.
    #[arg(long)]
    owner: Option<String>,
    /// Filter allow entries by classification.
    #[arg(long)]
    classification: Option<String>,
    /// Filter allow entries by source-tree path or path prefix.
    #[arg(long)]
    path: Option<String>,
    /// Filter allow entries by scanner-provided source-tree package context.
    #[arg(long)]
    source_package: Option<String>,
    /// Filter allow entries by current match status.
    #[arg(
        long,
        value_parser = [
            "matched",
            "new",
            "stale",
            "expired",
            "review_due",
            "ambiguous",
            "invalid_selector",
            "missing_required_field",
            "evidence_missing",
            "baseline_debt"
        ]
    )]
    status: Option<String>,
    /// Include only expired allow entries.
    #[arg(long)]
    expired: bool,
    /// Include only review-due allow entries.
    #[arg(long)]
    review_due: bool,
    /// Include only stale allow entries.
    #[arg(long)]
    stale: bool,
    /// Include only generated baseline debt entries.
    #[arg(long)]
    baseline_debt: bool,
    /// Include only entries with wildcard source-tree scopes.
    #[arg(long)]
    broad_scope: bool,
    /// Include only entries with no evidence references.
    #[arg(long)]
    missing_evidence: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = ListFormat::Human)]
    format: ListFormat,
    /// Write list output to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,
    /// Include untracked files when determining current match status.
    #[arg(long)]
    include_untracked: bool,
}

#[derive(Debug, Clone, Parser)]
struct ExplainArgs {
    /// Allow entry ID.
    id: String,
    #[command(flatten)]
    root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = ExplainFormat::Human)]
    format: ExplainFormat,
    /// Write explanation output to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Clone, Parser)]
struct AddArgs {
    #[command(flatten)]
    root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Finding kind to add.
    #[arg(long)]
    kind: String,
    /// Path containing the finding.
    #[arg(long)]
    path: PathBuf,
    /// Line near the finding.
    #[arg(long)]
    line: u32,
    /// Owner for the retained exception.
    #[arg(long)]
    owner: String,
    /// Reason this exception is acceptable.
    #[arg(long)]
    reason: String,
    /// Classification for the retained exception.
    #[arg(long, default_value = "reviewed_exception")]
    classification: String,
    /// Review date for the retained exception.
    #[arg(long, default_value = "2026-11-01")]
    review_after: String,
    /// Optional expiry date for the retained exception.
    #[arg(long)]
    expires: Option<String>,
    /// Evidence reference supporting this exception.
    #[arg(long)]
    evidence: Vec<String>,
    /// Entry ID. Defaults to the next allow-NNNN ID.
    #[arg(long)]
    id: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    include_untracked: bool,
    /// Write proposed policy to this path.
    #[arg(long)]
    write: Option<PathBuf>,
    /// Overwrite an existing output policy file.
    #[arg(long)]
    force: bool,
    /// Summary output format. Policy output remains TOML.
    #[arg(long, value_enum, default_value_t = AddSummaryFormat::Human)]
    summary_format: AddSummaryFormat,
    /// Write add summary to a file instead of stderr.
    #[arg(long)]
    summary_output: Option<PathBuf>,
}

#[derive(Debug, Clone, Parser)]
struct ProposeArgs {
    #[command(flatten)]
    root: RootArgs,
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
    /// Overwrite an existing output policy file.
    #[arg(long)]
    force: bool,
    /// Summary output format. Policy output remains TOML.
    #[arg(long, value_enum, default_value_t = ProposeSummaryFormat::Human)]
    summary_format: ProposeSummaryFormat,
    /// Write proposal summary to a file instead of stderr.
    #[arg(long)]
    summary_output: Option<PathBuf>,
}

#[derive(Debug, Clone, Parser)]
struct WorklistArgs {
    #[command(flatten)]
    root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Filter findings by kind.
    #[arg(long)]
    kind: Option<String>,
    /// Filter work items by scanner or policy family.
    #[arg(long)]
    family: Option<String>,
    /// Filter work items by queue item kind, such as stale_allow or baseline_debt.
    #[arg(long)]
    item_kind: Option<String>,
    /// Filter work items by match status.
    #[arg(
        long,
        value_parser = [
            "matched",
            "new",
            "stale",
            "expired",
            "review_due",
            "ambiguous",
            "invalid_selector",
            "missing_required_field",
            "evidence_missing",
            "baseline_debt"
        ]
    )]
    status: Option<String>,
    /// Filter work items by durable allow entry ID.
    #[arg(long)]
    allow_id: Option<String>,
    /// Filter work items by source-tree path or path prefix.
    #[arg(long)]
    path: Option<String>,
    /// Filter work items by scanner-provided source-tree package context.
    #[arg(long)]
    source_package: Option<String>,
    /// Filter work items by policy owner.
    #[arg(long)]
    owner: Option<String>,
    /// Filter work items by policy classification.
    #[arg(long)]
    classification: Option<String>,
    /// Include only generated baseline debt work items.
    #[arg(long)]
    baseline_debt: bool,
    /// Include only broad source-tree scope advisory work items.
    #[arg(long)]
    broad_scope: bool,
    /// Filter work items by risk.
    #[arg(long, value_parser = ["low", "medium", "high"])]
    risk: Option<String>,
    /// Filter work items by estimated difficulty.
    #[arg(long, value_parser = ["small", "medium"])]
    difficulty: Option<String>,
    /// Include only policy-backed work items with no evidence references.
    #[arg(long)]
    missing_evidence: bool,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = WorklistFormat::Json)]
    format: WorklistFormat,
    /// Write worklist to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Clone, Parser)]
struct MigrateArgs {
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
}

#[derive(Debug, Clone, Parser)]
struct PruneArgs {
    #[command(flatten)]
    root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Preview stale allow entries.
    #[arg(long)]
    stale: bool,
    /// Explicitly run without writing policy changes.
    #[arg(long, conflicts_with = "write")]
    dry_run: bool,
    /// Remove stale entries from the policy file.
    #[arg(long, conflicts_with = "dry_run")]
    write: bool,
    /// Include untracked files when determining stale entries.
    #[arg(long)]
    include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = PruneFormat::Human)]
    format: PruneFormat,
    /// Write prune preview/result to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Human,
    Html,
    Json,
    Sarif,
    #[value(alias = "md")]
    Markdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum WorklistFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ListFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ExplainFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum PruneFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum DoctorFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ProposeSummaryFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum AddSummaryFormat {
    Human,
    Json,
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
        CargoAllowCommand::Add(args) => cmd_add(&args),
        CargoAllowCommand::Propose(args) => cmd_propose(&args),
        CargoAllowCommand::Worklist(args) => cmd_worklist(&args),
        CargoAllowCommand::Migrate(args) => cmd_migrate(&args),
        CargoAllowCommand::Prune(args) => cmd_prune(&args),
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
    let (root, cfg, findings, inventory_facts) = if args.compat {
        load_compat_world(
            args.root.root.as_deref(),
            args.config.as_deref(),
            args.kind.as_deref(),
            args.include_untracked,
        )?
    } else {
        load_world(
            args.root.root.as_deref(),
            args.config.as_deref(),
            false,
            args.kind.as_deref(),
            args.include_untracked,
        )?
    };
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let outcomes = evaluate(&report_cfg, &findings, CheckMode::Audit);
    print_report(ReportRenderArgs {
        command: "audit",
        format: args.format,
        baseline_debt_entries: policy_baseline_debt_entries(&report_cfg),
        findings: &findings,
        outcomes: &outcomes,
        failed: false,
        output: args.output.as_deref(),
        root: &root,
        inventory_facts,
    })?;
    eprintln!("source tree: {}", root.display());
    Ok(())
}

fn cmd_check(args: &CheckArgs) -> CargoAllowResult<()> {
    let mode = CheckMode::parse(&args.mode);
    let (root, cfg, findings, inventory_facts) = if args.compat {
        load_compat_world(
            args.root.root.as_deref(),
            args.config.as_deref(),
            args.kind.as_deref(),
            args.include_untracked,
        )?
    } else {
        load_world(
            args.root.root.as_deref(),
            args.config.as_deref(),
            true,
            args.kind.as_deref(),
            args.include_untracked,
        )?
    };
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let outcomes = evaluate(&report_cfg, &findings, mode);
    let failed = outcomes.iter().any(|o| mode.fails(o.status));
    print_report(ReportRenderArgs {
        command: "check",
        format: args.format,
        baseline_debt_entries: policy_baseline_debt_entries(&report_cfg),
        findings: &findings,
        outcomes: &outcomes,
        failed,
        output: args.output.as_deref(),
        root: &root,
        inventory_facts,
    })?;
    if let Some(path) = &args.receipt {
        let root_text = source_tree_root_text(&root);
        write_file(
            path,
            &allow_report::render_receipt_with_context(
                "check",
                &outcomes,
                failed,
                allow_report::ReportContext {
                    inventory_source: inventory_facts.source.as_str(),
                    source_tree_root: Some(&root_text),
                    inventory_files: inventory_facts.files_scanned,
                    ..allow_report::ReportContext::default()
                },
            ),
        )?;
    }
    if failed {
        process::exit(1);
    }
    Ok(())
}

fn cmd_diff(args: &DiffArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings, inventory_facts) = load_world(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        args.kind.as_deref(),
        args.include_untracked,
    )?;
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let outcomes = evaluate(&report_cfg, &findings, CheckMode::NoNew);
    let policy_path = git_relative_config_path(&root, args.config.as_deref())?;
    let base_cfg = allow_diff::policy_config_at_revision(&root, &args.base, &policy_path)?
        .unwrap_or_else(|| report_cfg.clone());
    let head_cfg_for_diff = if let Some(head) = &args.head {
        allow_diff::policy_config_at_revision(&root, head, &policy_path)?
            .unwrap_or_else(|| report_cfg.clone())
    } else {
        report_cfg.clone()
    };
    let mut base_findings = allow_diff::findings_at_revision(&root, &args.base, &base_cfg)?;
    if let Some(kind) = &args.kind {
        let parsed = parse_kind_filter(kind)?;
        base_findings.retain(|finding| parsed.matches_finding(finding));
    }
    let mut head_findings_for_diff = if let Some(head) = &args.head {
        allow_diff::findings_at_revision(&root, head, &head_cfg_for_diff)?
    } else {
        findings.clone()
    };
    if let Some(kind) = &args.kind {
        let parsed = parse_kind_filter(kind)?;
        head_findings_for_diff.retain(|finding| parsed.matches_finding(finding));
    }
    let finding_changes =
        allow_diff::finding_posture_changes(&base_findings, &head_findings_for_diff);
    let policy_changes =
        allow_diff::policy_changes_from_git(&root, &args.base, &policy_path, &head_cfg_for_diff)?;
    let policy_failed = policy_changes.iter().any(|change| change.severity.fails());
    let failed = outcomes.iter().any(|o| CheckMode::NoNew.fails(o.status)) || policy_failed;
    let root_text = source_tree_root_text(&root);
    let report_context = allow_report::ReportContext {
        inventory_source: inventory_facts.source.as_str(),
        source_tree_root: Some(&root_text),
        inventory_files: inventory_facts.files_scanned,
        baseline_debt_entries: Some(policy_baseline_debt_entries(&report_cfg)),
    };
    let mut text = match args.format {
        OutputFormat::Json => render_diff_json_with_posture(
            allow_report::render_json_with_context(
                "diff",
                &findings,
                &outcomes,
                failed,
                report_context,
            ),
            &outcomes,
            &finding_changes,
            &policy_changes,
        ),
        OutputFormat::Html => allow_report::render_html_with_context(
            "diff",
            &findings,
            &outcomes,
            failed,
            report_context,
        ),
        OutputFormat::Sarif => allow_report::render_sarif_with_context(
            "diff",
            &findings,
            &outcomes,
            failed,
            report_context,
        ),
        OutputFormat::Markdown => allow_report::render_markdown_with_context(
            "diff",
            &findings,
            &outcomes,
            failed,
            report_context,
        ),
        OutputFormat::Human => allow_report::render_human_with_context(
            "diff",
            &findings,
            &outcomes,
            failed,
            report_context,
        ),
    };
    if args.format == OutputFormat::Markdown {
        let summary = render_diff_pr_summary_markdown(&outcomes, &finding_changes, &policy_changes);
        insert_markdown_pr_summary(&mut text, &summary);
    }
    append_finding_posture_changes(&mut text, args.format, &finding_changes);
    append_policy_changes(&mut text, args.format, &policy_changes);
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
    if args.format == OutputFormat::Json && !policy_changes.is_empty() {
        eprintln!("{}", render_policy_changes_human(&policy_changes));
    }
    if args.format == OutputFormat::Json && !finding_changes.is_empty() {
        eprintln!("{}", render_finding_posture_changes_human(&finding_changes));
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

fn insert_markdown_pr_summary(text: &mut String, summary: &str) {
    let marker = "Findings scanned:";
    if let Some(index) = text.find(marker) {
        text.insert_str(index, summary);
    } else {
        text.push('\n');
        text.push_str(summary);
    }
}

fn render_diff_pr_summary_markdown(
    outcomes: &[MatchOutcome],
    finding_changes: &[allow_diff::FindingPostureChange],
    policy_changes: &[allow_diff::PolicyChange],
) -> String {
    let summary = diff_posture_summary(outcomes, finding_changes, policy_changes);
    let posture = summary.net_posture();
    let mut out = String::new();
    out.push_str("## PR Summary\n\n");
    out.push_str(&format!("**Net posture:** `{}`\n\n", posture.as_str()));
    out.push_str("| Signal | Count |\n|---|---:|\n");
    out.push_str(&format!(
        "| Current no-new failures | {} |\n",
        summary.current_failures
    ));
    out.push_str(&format!(
        "| New source findings | {} |\n",
        summary.new_findings
    ));
    out.push_str(&format!(
        "| Removed source findings | {} |\n",
        summary.removed_findings
    ));
    out.push_str(&format!(
        "| Policy failures | {} |\n",
        summary.policy_failures
    ));
    out.push_str(&format!(
        "| Policy review items | {} |\n",
        summary.policy_review_items
    ));
    out.push_str(&format!(
        "| Policy improvements | {} |\n",
        summary.policy_improvements
    ));
    out.push_str(&format!(
        "\n**Reviewer action:** {}\n\n",
        posture.reviewer_action()
    ));
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DiffPostureSummary {
    current_failures: usize,
    new_findings: usize,
    removed_findings: usize,
    policy_failures: usize,
    policy_review_items: usize,
    policy_improvements: usize,
}

impl DiffPostureSummary {
    fn net_posture(self) -> DiffNetPosture {
        diff_net_posture(
            self.current_failures,
            self.new_findings,
            self.removed_findings,
            self.policy_failures,
            self.policy_review_items,
            self.policy_improvements,
        )
    }
}

fn diff_posture_summary(
    outcomes: &[MatchOutcome],
    finding_changes: &[allow_diff::FindingPostureChange],
    policy_changes: &[allow_diff::PolicyChange],
) -> DiffPostureSummary {
    DiffPostureSummary {
        current_failures: outcomes
            .iter()
            .filter(|outcome| CheckMode::NoNew.fails(outcome.status))
            .count(),
        new_findings: finding_changes
            .iter()
            .filter(|change| change.kind == allow_diff::FindingPostureKind::New)
            .count(),
        removed_findings: finding_changes
            .iter()
            .filter(|change| change.kind == allow_diff::FindingPostureKind::Removed)
            .count(),
        policy_failures: policy_changes
            .iter()
            .filter(|change| change.severity == allow_diff::PolicyChangeSeverity::Fail)
            .count(),
        policy_review_items: policy_changes
            .iter()
            .filter(|change| change.severity == allow_diff::PolicyChangeSeverity::Review)
            .count(),
        policy_improvements: policy_changes
            .iter()
            .filter(|change| change.severity == allow_diff::PolicyChangeSeverity::Improvement)
            .count(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffNetPosture {
    Worse,
    ReviewRequired,
    Improved,
    Unchanged,
}

impl DiffNetPosture {
    fn as_str(self) -> &'static str {
        match self {
            Self::Worse => "worse",
            Self::ReviewRequired => "review-required",
            Self::Improved => "improved",
            Self::Unchanged => "unchanged",
        }
    }

    fn reviewer_action(self) -> &'static str {
        match self {
            Self::Worse => {
                "block until failing source exception changes are fixed, narrowed, or receipted."
            }
            Self::ReviewRequired => "review the source exception posture change before merging.",
            Self::Improved => "verify the cleanup was intentional and keep the narrower posture.",
            Self::Unchanged => "no source exception posture change detected.",
        }
    }
}

fn diff_net_posture(
    current_failures: usize,
    new_findings: usize,
    removed_findings: usize,
    policy_failures: usize,
    policy_review_items: usize,
    policy_improvements: usize,
) -> DiffNetPosture {
    if current_failures > 0 || policy_failures > 0 {
        return DiffNetPosture::Worse;
    }
    if new_findings > 0 || policy_review_items > 0 {
        return DiffNetPosture::ReviewRequired;
    }
    if removed_findings > 0 || policy_improvements > 0 {
        return DiffNetPosture::Improved;
    }
    DiffNetPosture::Unchanged
}

fn append_finding_posture_changes(
    text: &mut String,
    format: OutputFormat,
    changes: &[allow_diff::FindingPostureChange],
) {
    match format {
        OutputFormat::Human => text.push_str(&render_finding_posture_changes_human(changes)),
        OutputFormat::Markdown => text.push_str(&render_finding_posture_changes_markdown(changes)),
        OutputFormat::Html | OutputFormat::Json | OutputFormat::Sarif => {}
    }
}

fn render_diff_json_with_posture(
    report_json: String,
    outcomes: &[MatchOutcome],
    finding_changes: &[allow_diff::FindingPostureChange],
    policy_changes: &[allow_diff::PolicyChange],
) -> String {
    let diff_json = render_diff_posture_json(outcomes, finding_changes, policy_changes);
    let trimmed = report_json.trim_end();
    if let Some(prefix) = trimmed.strip_suffix('}') {
        format!("{prefix},\n  \"diff\": {diff_json}\n}}\n")
    } else {
        eprintln!("warning: failed to append diff posture to JSON report");
        report_json
    }
}

fn render_diff_posture_json(
    outcomes: &[MatchOutcome],
    finding_changes: &[allow_diff::FindingPostureChange],
    policy_changes: &[allow_diff::PolicyChange],
) -> String {
    let summary = diff_posture_summary(outcomes, finding_changes, policy_changes);
    let posture = summary.net_posture();
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("    \"net_posture\": \"{}\",\n", posture.as_str()));
    out.push_str(&format!(
        "    \"reviewer_action\": \"{}\",\n",
        json_escape(posture.reviewer_action())
    ));
    out.push_str("    \"summary\": {\n");
    out.push_str(&format!(
        "      \"current_failures\": {},\n",
        summary.current_failures
    ));
    out.push_str(&format!(
        "      \"new_findings\": {},\n",
        summary.new_findings
    ));
    out.push_str(&format!(
        "      \"removed_findings\": {},\n",
        summary.removed_findings
    ));
    out.push_str(&format!(
        "      \"policy_failures\": {},\n",
        summary.policy_failures
    ));
    out.push_str(&format!(
        "      \"policy_review_items\": {},\n",
        summary.policy_review_items
    ));
    out.push_str(&format!(
        "      \"policy_improvements\": {}\n",
        summary.policy_improvements
    ));
    out.push_str("    },\n");
    out.push_str("    \"finding_changes\": [\n");
    for (index, change) in finding_changes.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str("      {");
        out.push_str(&format!("\"change\": \"{}\", ", change.kind.as_str()));
        out.push_str(&format!("\"key\": \"{}\", ", json_escape(&change.key)));
        out.push_str(&format!(
            "\"kind\": \"{}\", ",
            json_escape(&change.finding_kind)
        ));
        out.push_str(&format!(
            "\"family\": {}, ",
            option_json_string(change.family.as_deref())
        ));
        out.push_str(&format!("\"path\": \"{}\"", json_escape(&change.path)));
        out.push('}');
    }
    out.push_str("\n    ],\n");
    out.push_str("    \"policy_changes\": [\n");
    for (index, change) in policy_changes.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str("      {");
        out.push_str(&format!("\"severity\": \"{}\", ", change.severity.as_str()));
        out.push_str(&format!(
            "\"allow_id\": \"{}\", ",
            json_escape(&change.allow_id)
        ));
        out.push_str(&format!("\"kind\": \"{}\", ", change.kind.as_str()));
        out.push_str(&format!(
            "\"message\": \"{}\"",
            json_escape(&change.message)
        ));
        out.push('}');
    }
    out.push_str("\n    ]\n");
    out.push_str("  }");
    out
}

fn render_finding_posture_changes_human(changes: &[allow_diff::FindingPostureChange]) -> String {
    let mut out = String::new();
    out.push_str("\nFinding posture changes:\n");
    if changes.is_empty() {
        out.push_str("  none\n");
        return out;
    }
    for change in changes.iter().take(120) {
        out.push_str(&format!(
            "  {} {}{} at {}\n",
            change.kind.as_str(),
            change.finding_kind,
            change
                .family
                .as_ref()
                .map(|family| format!(".{family}"))
                .unwrap_or_default(),
            change.path
        ));
    }
    if changes.len() > 120 {
        out.push_str(&format!("  ... {} more omitted\n", changes.len() - 120));
    }
    out
}

fn render_finding_posture_changes_markdown(changes: &[allow_diff::FindingPostureChange]) -> String {
    let mut out = String::new();
    out.push_str("\n## Finding Posture Changes\n\n");
    if changes.is_empty() {
        out.push_str("No source finding posture changes detected.\n");
        return out;
    }
    out.push_str("| Change | Kind | Family | Path |\n|---|---|---|---|\n");
    for change in changes.iter().take(120) {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` |\n",
            markdown_cell(change.kind.as_str()),
            markdown_cell(&change.finding_kind),
            markdown_cell(change.family.as_deref().unwrap_or("")),
            markdown_cell(&change.path)
        ));
    }
    if changes.len() > 120 {
        out.push_str(&format!(
            "\n{} additional finding posture changes omitted.\n",
            changes.len() - 120
        ));
    }
    out
}

fn append_policy_changes(
    text: &mut String,
    format: OutputFormat,
    changes: &[allow_diff::PolicyChange],
) {
    match format {
        OutputFormat::Human => text.push_str(&render_policy_changes_human(changes)),
        OutputFormat::Markdown => text.push_str(&render_policy_changes_markdown(changes)),
        OutputFormat::Html | OutputFormat::Json | OutputFormat::Sarif => {}
    }
}

fn render_policy_changes_human(changes: &[allow_diff::PolicyChange]) -> String {
    let mut out = String::new();
    out.push_str("\nPolicy posture changes:\n");
    if changes.is_empty() {
        out.push_str("  none\n");
        return out;
    }
    for change in changes {
        out.push_str(&format!(
            "  {} {} {}: {}\n",
            change.severity.as_str(),
            change.allow_id,
            change.kind.as_str(),
            change.message
        ));
    }
    out
}

fn render_policy_changes_markdown(changes: &[allow_diff::PolicyChange]) -> String {
    let mut out = String::new();
    out.push_str("\n## Policy Posture Changes\n\n");
    if changes.is_empty() {
        out.push_str("No policy weakening detected.\n");
        return out;
    }
    out.push_str("| Severity | Allow ID | Kind | Message |\n|---|---|---|---|\n");
    for change in changes {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | {} |\n",
            markdown_cell(change.severity.as_str()),
            markdown_cell(&change.allow_id),
            markdown_cell(change.kind.as_str()),
            markdown_cell(&change.message)
        ));
    }
    out
}

fn cmd_list(args: &ListArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings, inventory_facts) = load_world(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        None,
        args.include_untracked,
    )?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);
    let parsed_filter = args.kind.as_deref().map(parse_kind_filter).transpose()?;
    let rows = list_rows(&cfg, &findings, &outcomes);
    let filters = ListFilters {
        kind: parsed_filter,
        family: args.family.as_deref(),
        owner: args.owner.as_deref(),
        classification: args.classification.as_deref(),
        path: args.path.as_deref(),
        source_package: args.source_package.as_deref(),
        status: args.status.as_deref(),
        expired: args.expired,
        review_due: args.review_due,
        stale: args.stale,
        baseline_debt: args.baseline_debt,
        broad_scope: args.broad_scope,
        missing_evidence: args.missing_evidence,
    };
    let root_text = source_tree_root_text(&root);
    let context = ListContext {
        inventory_source: inventory_facts.source.as_str(),
        source_tree_root: Some(&root_text),
        inventory_files: inventory_facts.files_scanned,
        kind_arg: args.kind.as_deref(),
    };
    let text = match args.format {
        ListFormat::Human => render_list_rows(&rows, &filters),
        ListFormat::Json => render_list_rows_json(&rows, &filters, context),
    };
    if let Some(path) = &args.output {
        write_file(path, &text)?;
    } else {
        println!("{text}");
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct ListRow {
    id: String,
    status: MatchStatus,
    matches: usize,
    kind: FindingKind,
    family: Option<String>,
    owner: String,
    classification: String,
    scope: String,
    source_package: Option<String>,
    evidence_count: usize,
    review_after: String,
    expires: String,
    reason: String,
}

#[derive(Debug, Clone, Copy)]
struct ListFilters<'a> {
    kind: Option<KindFilter>,
    family: Option<&'a str>,
    owner: Option<&'a str>,
    classification: Option<&'a str>,
    path: Option<&'a str>,
    source_package: Option<&'a str>,
    status: Option<&'a str>,
    expired: bool,
    review_due: bool,
    stale: bool,
    baseline_debt: bool,
    broad_scope: bool,
    missing_evidence: bool,
}

#[derive(Debug, Clone, Copy)]
struct ListContext<'a> {
    inventory_source: &'a str,
    source_tree_root: Option<&'a str>,
    inventory_files: Option<usize>,
    kind_arg: Option<&'a str>,
}

impl Default for ListContext<'static> {
    fn default() -> Self {
        Self {
            inventory_source: "unknown",
            source_tree_root: None,
            inventory_files: None,
            kind_arg: None,
        }
    }
}

fn list_rows(cfg: &AllowConfig, findings: &[Finding], outcomes: &[MatchOutcome]) -> Vec<ListRow> {
    let today = SimpleDate::today_utc_approx();
    cfg.allow
        .iter()
        .map(|entry| {
            let entry_outcomes = outcomes
                .iter()
                .filter(|outcome| outcome.allow_id.as_deref() == Some(entry.id.as_str()))
                .collect::<Vec<_>>();
            ListRow {
                id: entry.id.clone(),
                status: list_entry_status(entry, &entry_outcomes, today),
                matches: entry_outcomes
                    .iter()
                    .filter(|outcome| outcome.finding_index.is_some())
                    .count(),
                kind: entry.kind,
                family: entry.family.clone(),
                owner: entry.owner.clone(),
                classification: entry.classification.clone(),
                scope: entry.path_or_glob(),
                source_package: entry_outcomes
                    .iter()
                    .filter_map(|outcome| outcome.finding_index)
                    .filter_map(|index| findings.get(index))
                    .find_map(source_package_name),
                evidence_count: entry.evidence.len(),
                review_after: entry
                    .lifecycle
                    .review_after
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
                expires: entry
                    .lifecycle
                    .expires
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
                reason: entry.reason.clone(),
            }
        })
        .collect()
}

fn list_entry_status(
    entry: &AllowEntry,
    outcomes: &[&MatchOutcome],
    today: SimpleDate,
) -> MatchStatus {
    if date_is_before(entry.lifecycle.expires.as_deref(), today) {
        return MatchStatus::Expired;
    }
    if date_is_due(entry.lifecycle.review_after.as_deref(), today) {
        return MatchStatus::ReviewDue;
    }
    for status in [
        MatchStatus::New,
        MatchStatus::Ambiguous,
        MatchStatus::EvidenceMissing,
        MatchStatus::MissingRequiredField,
        MatchStatus::InvalidSelector,
        MatchStatus::Stale,
    ] {
        if outcomes.iter().any(|outcome| outcome.status == status) {
            return status;
        }
    }
    if entry.classification == "baseline_debt" {
        return MatchStatus::BaselineDebt;
    }
    MatchStatus::Matched
}

fn date_is_before(date: Option<&str>, today: SimpleDate) -> bool {
    date.and_then(SimpleDate::parse)
        .map(|date| date < today)
        .unwrap_or(false)
}

fn date_is_due(date: Option<&str>, today: SimpleDate) -> bool {
    date.and_then(SimpleDate::parse)
        .map(|date| date <= today)
        .unwrap_or(false)
}

fn render_list_rows(rows: &[ListRow], filters: &ListFilters<'_>) -> String {
    let mut out = String::new();
    out.push_str("id\tstatus\tmatches\tkind\tfamily\towner\tclassification\tscope\tsource_package\tevidence_count\treview_after\texpires\treason\n");
    let mut count = 0;
    for row in rows.iter().filter(|row| list_row_matches(row, filters)) {
        count += 1;
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            row.id,
            row.status.as_str(),
            row.matches,
            row.kind,
            row.family.as_deref().unwrap_or("-"),
            empty_as_dash(&row.owner),
            empty_as_dash(&row.classification),
            row.scope,
            row.source_package.as_deref().unwrap_or("-"),
            row.evidence_count,
            row.review_after,
            row.expires,
            row.reason
        ));
    }
    if count == 0 {
        out.push_str("(no allow entries matched filters)\n");
    }
    out
}

fn render_list_rows_json(
    rows: &[ListRow],
    filters: &ListFilters<'_>,
    context: ListContext<'_>,
) -> String {
    let filtered = rows
        .iter()
        .filter(|row| list_row_matches(row, filters))
        .collect::<Vec<_>>();
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": {},\n",
        allow_report::LIST_SCHEMA_VERSION
    ));
    out.push_str(&format!(
        "  \"schema_id\": \"{}\",\n",
        allow_report::LIST_SCHEMA_ID
    ));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str("  \"command\": \"list\",\n");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        json_string_array(allow_report::CLAIM_BOUNDARY)
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        json_string_array(allow_report::SCANNER_LIMITATIONS)
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&list_inventory_json(context, "  "));
    out.push_str(",\n");
    out.push_str("  \"filters\": ");
    out.push_str(&list_filters_json(filters, context.kind_arg, "  "));
    out.push_str(",\n");
    out.push_str(&format!(
        "  \"summary\": {{\n    \"allow_entries\": {}\n  }},\n",
        filtered.len()
    ));
    out.push_str("  \"allow_entries\": [\n");
    for (index, row) in filtered.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&render_list_row_json(row));
    }
    out.push_str("\n  ]\n");
    out.push_str("}\n");
    out
}

fn render_list_row_json(row: &ListRow) -> String {
    let mut out = String::new();
    out.push_str("    {\n");
    out.push_str(&format!("      \"id\": \"{}\",\n", json_escape(&row.id)));
    out.push_str(&format!("      \"status\": \"{}\",\n", row.status.as_str()));
    out.push_str(&format!("      \"matches\": {},\n", row.matches));
    out.push_str(&format!("      \"kind\": \"{}\",\n", row.kind));
    out.push_str(&format!(
        "      \"family\": {},\n",
        option_json_string(row.family.as_deref())
    ));
    out.push_str(&format!(
        "      \"owner\": \"{}\",\n",
        json_escape(&row.owner)
    ));
    out.push_str(&format!(
        "      \"classification\": \"{}\",\n",
        json_escape(&row.classification)
    ));
    out.push_str(&format!(
        "      \"scope\": \"{}\",\n",
        json_escape(&row.scope)
    ));
    out.push_str(&format!(
        "      \"source_package\": {},\n",
        option_json_string(row.source_package.as_deref())
    ));
    out.push_str(&format!(
        "      \"evidence_count\": {},\n",
        row.evidence_count
    ));
    out.push_str(&format!(
        "      \"review_after\": {},\n",
        dash_as_null_json(&row.review_after)
    ));
    out.push_str(&format!(
        "      \"expires\": {},\n",
        dash_as_null_json(&row.expires)
    ));
    out.push_str(&format!(
        "      \"reason\": \"{}\"\n",
        json_escape(&row.reason)
    ));
    out.push_str("    }");
    out
}

fn list_inventory_json(context: ListContext<'_>, indent: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("{indent}  \"scope\": \"source_tree\",\n"));
    out.push_str(&format!("{indent}  \"scanner\": \"source_syntax\",\n"));
    out.push_str(&format!(
        "{indent}  \"source\": \"{}\"",
        json_escape(context.inventory_source)
    ));
    if let Some(root) = context.source_tree_root {
        out.push_str(&format!(",\n{indent}  \"root\": \"{}\"", json_escape(root)));
    }
    if let Some(files) = context.inventory_files {
        out.push_str(&format!(",\n{indent}  \"files_scanned\": {files}"));
    }
    out.push_str(&format!("\n{indent}}}"));
    out
}

fn list_filters_json(filters: &ListFilters<'_>, kind_arg: Option<&str>, indent: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "{indent}  \"kind\": {},\n",
        option_json_string(kind_arg)
    ));
    out.push_str(&format!(
        "{indent}  \"family\": {},\n",
        option_json_string(filters.family)
    ));
    out.push_str(&format!(
        "{indent}  \"owner\": {},\n",
        option_json_string(filters.owner)
    ));
    out.push_str(&format!(
        "{indent}  \"classification\": {},\n",
        option_json_string(filters.classification)
    ));
    out.push_str(&format!(
        "{indent}  \"path\": {},\n",
        option_json_string(filters.path)
    ));
    out.push_str(&format!(
        "{indent}  \"source_package\": {},\n",
        option_json_string(filters.source_package)
    ));
    out.push_str(&format!(
        "{indent}  \"status\": {},\n",
        option_json_string(filters.status)
    ));
    out.push_str(&format!("{indent}  \"expired\": {},\n", filters.expired));
    out.push_str(&format!(
        "{indent}  \"review_due\": {},\n",
        filters.review_due
    ));
    out.push_str(&format!("{indent}  \"stale\": {},\n", filters.stale));
    out.push_str(&format!(
        "{indent}  \"baseline_debt\": {},\n",
        filters.baseline_debt
    ));
    out.push_str(&format!(
        "{indent}  \"broad_scope\": {},\n",
        filters.broad_scope
    ));
    out.push_str(&format!(
        "{indent}  \"missing_evidence\": {}\n",
        filters.missing_evidence
    ));
    out.push_str(&format!("{indent}}}"));
    out
}

fn dash_as_null_json(value: &str) -> String {
    if value == "-" {
        "null".to_string()
    } else {
        format!("\"{}\"", json_escape(value))
    }
}

fn list_row_matches(row: &ListRow, filters: &ListFilters<'_>) -> bool {
    if let Some(kind) = filters.kind {
        if row.kind != kind.kind || !kind.family.matches(row.family.as_deref()) {
            return false;
        }
    }
    if let Some(family) = filters.family {
        if row.family.as_deref() != Some(family) {
            return false;
        }
    }
    if let Some(owner) = filters.owner {
        if row.owner != owner {
            return false;
        }
    }
    if let Some(classification) = filters.classification {
        if row.classification != classification {
            return false;
        }
    }
    if let Some(path) = filters.path {
        if !source_tree_path_matches_filter(&row.scope, path) {
            return false;
        }
    }
    if let Some(source_package) = filters.source_package {
        if row.source_package.as_deref() != Some(source_package) {
            return false;
        }
    }
    if let Some(status) = filters.status {
        if row.status.as_str() != status {
            return false;
        }
    }
    if filters.expired && row.status != MatchStatus::Expired {
        return false;
    }
    if filters.review_due && row.status != MatchStatus::ReviewDue {
        return false;
    }
    if filters.stale && row.status != MatchStatus::Stale {
        return false;
    }
    if filters.baseline_debt && row.classification != "baseline_debt" {
        return false;
    }
    if filters.broad_scope && !scope_has_wildcard(&row.scope) {
        return false;
    }
    if filters.missing_evidence && row.evidence_count != 0 {
        return false;
    }
    true
}

fn empty_as_dash(value: &str) -> &str {
    if value.trim().is_empty() { "-" } else { value }
}

fn cmd_explain(args: &ExplainArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings, inventory_facts) = load_world_with_evidence_validation(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        None,
        args.include_untracked,
        false,
    )?;
    let entry = cfg
        .allow
        .iter()
        .find(|e| e.id == args.id)
        .ok_or_else(|| CargoAllowError::new(format!("no allow entry `{}`", args.id)))?;
    let root_text = source_tree_root_text(&root);
    let context = ExplainContext {
        inventory_source: inventory_facts.source.as_str(),
        source_tree_root: Some(&root_text),
        inventory_files: inventory_facts.files_scanned,
    };
    let text = match args.format {
        ExplainFormat::Human => explain_entry_text(&root, &cfg, entry, &findings),
        ExplainFormat::Json => explain_entry_json(&root, &cfg, entry, &findings, context),
    };
    if let Some(path) = &args.output {
        write_file(path, &text)?;
    } else {
        println!("{text}");
    }
    Ok(())
}

fn cmd_add(args: &AddArgs) -> CargoAllowResult<()> {
    let parsed_kind = parse_kind_filter(&args.kind)?;
    let (root, mut cfg, findings, inventory_facts) = load_world(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        Some(args.kind.as_str()),
        args.include_untracked,
    )?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::Audit);
    let (finding_index, finding) =
        select_add_finding(&findings, parsed_kind, &args.path, args.line)?;
    let selected_outcome = outcomes
        .iter()
        .find(|outcome| outcome.finding_index == Some(finding_index))
        .ok_or_else(|| CargoAllowError::new("selected finding did not produce a match outcome"))?;
    ensure_addable_outcome(selected_outcome.status)?;
    if finding.kind == FindingKind::Unsafe && args.evidence.is_empty() {
        return Err(CargoAllowError::new(
            "unsafe allow entries require at least one --evidence reference",
        ));
    }
    let id = args.id.clone().unwrap_or_else(|| next_allow_id(&cfg));
    if cfg.allow.iter().any(|entry| entry.id == id) {
        return Err(CargoAllowError::new(format!(
            "allow entry id `{id}` already exists"
        )));
    }
    let entry = allow_entry_from_finding(AddEntryRequest {
        finding,
        id,
        owner: args.owner.clone(),
        classification: args.classification.clone(),
        reason: args.reason.clone(),
        evidence: args.evidence.clone(),
        review_after: args.review_after.clone(),
        expires: args.expires.clone(),
    });
    let root_text = source_tree_root_text(&root);
    let context = AddContext {
        inventory_source: inventory_facts.source.as_str(),
        source_tree_root: Some(&root_text),
        inventory_files: inventory_facts.files_scanned,
    };
    let summary = match args.summary_format {
        AddSummaryFormat::Human => render_add_summary(&entry, finding, args.write.as_deref()),
        AddSummaryFormat::Json => {
            render_add_summary_json(&entry, finding, args.write.as_deref(), args.force, context)
        }
    };
    cfg.allow.push(entry);
    validate_policy(&cfg)?;
    validate_local_evidence_references(&root, &cfg)?;
    let rendered = render_policy(&cfg);
    if let Some(path) = &args.write {
        write_file_no_overwrite(path, &rendered, args.force)?;
    } else {
        println!("{rendered}");
    }
    if let Some(path) = &args.summary_output {
        write_file(path, &summary)?;
    } else {
        eprintln!("{summary}");
    }
    Ok(())
}

fn select_add_finding<'a>(
    findings: &'a [Finding],
    kind: KindFilter,
    path: &Path,
    line: u32,
) -> CargoAllowResult<(usize, &'a Finding)> {
    let normalized_path = normalize_path(path);
    let mut candidates = findings
        .iter()
        .enumerate()
        .filter(|(_, finding)| kind.matches_finding(finding))
        .filter(|(_, finding)| normalize_path(&finding.path) == normalized_path)
        .filter_map(|(index, finding)| {
            finding
                .span
                .as_ref()
                .map(|span| (span.line.abs_diff(line), index, finding))
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(distance, _, finding)| (*distance, normalize_path(&finding.path)));
    let Some((distance, index, finding)) = candidates.first().copied() else {
        return Err(CargoAllowError::new(format!(
            "no current {} finding found near {}:{}",
            kind.kind, normalized_path, line
        )));
    };
    let tied = candidates
        .iter()
        .filter(|(candidate_distance, _, _)| *candidate_distance == distance)
        .count();
    if tied > 1 {
        return Err(CargoAllowError::new(format!(
            "ambiguous add request: {tied} findings are equally near {}:{}",
            normalized_path, line
        )));
    }
    Ok((index, finding))
}

fn ensure_addable_outcome(status: MatchStatus) -> CargoAllowResult<()> {
    if status == MatchStatus::New {
        return Ok(());
    }
    Err(CargoAllowError::new(format!(
        "selected finding is already receipted or blocked with status `{}`; use list or explain before editing policy",
        status.as_str()
    )))
}

struct AddEntryRequest<'a> {
    finding: &'a Finding,
    id: String,
    owner: String,
    classification: String,
    reason: String,
    evidence: Vec<String>,
    review_after: String,
    expires: Option<String>,
}

fn allow_entry_from_finding(request: AddEntryRequest<'_>) -> AllowEntry {
    let selector = selector_from_finding(request.finding);
    AllowEntry {
        id: request.id,
        kind: request.finding.kind,
        family: request.finding.family.clone(),
        path: Some(request.finding.path.clone()),
        glob: None,
        owner: request.owner,
        classification: request.classification,
        reason: request.reason,
        evidence: request.evidence,
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some(SimpleDate::today_utc_approx().to_string()),
            review_after: Some(request.review_after),
            expires: request.expires,
        },
        selector,
        last_seen: request.finding.span.as_ref().map(|s| allow_core::LastSeen {
            line: s.line,
            column: s.column,
        }),
    }
}

fn render_add_summary(entry: &AllowEntry, finding: &Finding, output: Option<&Path>) -> String {
    let mut out = String::new();
    out.push_str("cargo-allow add summary\n");
    out.push_str(&format!("id: {}\n", entry.id));
    out.push_str(&format!("kind: {}\n", entry.kind));
    if let Some(family) = &entry.family {
        out.push_str(&format!("family: {family}\n"));
    }
    out.push_str(&format!("scope: {}\n", entry.path_or_glob()));
    out.push_str(&format!("owner: {}\n", entry.owner));
    out.push_str(&format!("classification: {}\n", entry.classification));
    out.push_str(&format!("matched finding: {}\n", finding_location(finding)));
    if let Some(output) = output {
        out.push_str(&format!("output: {}\n", output.display()));
    } else {
        out.push_str("output: stdout\n");
    }
    out.push_str("claim boundary: generated policy entry requires human review before merge.\n");
    out
}

#[derive(Debug, Clone, Copy)]
struct AddContext<'a> {
    inventory_source: &'a str,
    source_tree_root: Option<&'a str>,
    inventory_files: Option<usize>,
}

impl Default for AddContext<'static> {
    fn default() -> Self {
        Self {
            inventory_source: "unknown",
            source_tree_root: None,
            inventory_files: None,
        }
    }
}

fn render_add_summary_json(
    entry: &AllowEntry,
    finding: &Finding,
    output: Option<&Path>,
    force: bool,
    context: AddContext<'_>,
) -> String {
    let policy_output = output.map(|path| path.display().to_string());
    let path = entry.path.as_ref().map(normalize_path);
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": {},\n",
        allow_report::ADD_SCHEMA_VERSION
    ));
    out.push_str(&format!(
        "  \"schema_id\": \"{}\",\n",
        allow_report::ADD_SCHEMA_ID
    ));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str("  \"command\": \"add\",\n");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        json_string_array(allow_report::CLAIM_BOUNDARY)
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        json_string_array(allow_report::SCANNER_LIMITATIONS)
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&add_inventory_json(context, "  "));
    out.push_str(",\n");
    out.push_str("  \"options\": {\n");
    out.push_str(&format!(
        "    \"policy_output\": {},\n",
        option_json_string(policy_output.as_deref())
    ));
    out.push_str(&format!("    \"force\": {}\n", force));
    out.push_str("  },\n");
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!(
        "    \"entry_id\": \"{}\",\n",
        json_escape(&entry.id)
    ));
    out.push_str(&format!(
        "    \"selected_finding\": \"{}\",\n",
        json_escape(&finding_location(finding))
    ));
    out.push_str("    \"human_review_required\": true\n");
    out.push_str("  },\n");
    out.push_str("  \"allow_entry\": {\n");
    out.push_str(&format!("    \"id\": \"{}\",\n", json_escape(&entry.id)));
    out.push_str(&format!("    \"kind\": \"{}\",\n", entry.kind));
    out.push_str(&format!(
        "    \"family\": {},\n",
        option_json_string(entry.family.as_deref())
    ));
    out.push_str(&format!(
        "    \"path\": {},\n",
        option_json_string(path.as_deref())
    ));
    out.push_str(&format!(
        "    \"glob\": {},\n",
        option_json_string(entry.glob.as_deref())
    ));
    out.push_str(&format!(
        "    \"owner\": \"{}\",\n",
        json_escape(&entry.owner)
    ));
    out.push_str(&format!(
        "    \"classification\": \"{}\",\n",
        json_escape(&entry.classification)
    ));
    out.push_str(&format!(
        "    \"reason\": \"{}\",\n",
        json_escape(&entry.reason)
    ));
    out.push_str(&format!(
        "    \"review_after\": {},\n",
        option_json_string(entry.lifecycle.review_after.as_deref())
    ));
    out.push_str(&format!(
        "    \"expires\": {},\n",
        option_json_string(entry.lifecycle.expires.as_deref())
    ));
    out.push_str(&format!(
        "    \"evidence_count\": {},\n",
        entry.evidence.len()
    ));
    out.push_str("    \"selector\": ");
    out.push_str(&selector_json(&entry.selector, "    "));
    out.push_str(",\n");
    out.push_str("    \"last_seen\": ");
    out.push_str(&last_seen_json(entry.last_seen.as_ref(), "    "));
    out.push_str("\n  },\n");
    out.push_str("  \"selected_finding\": ");
    out.push_str(&explain_finding_json(finding, "selected", "  "));
    out.push_str("\n}\n");
    out
}

fn add_inventory_json(context: AddContext<'_>, indent: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("{indent}  \"scope\": \"source_tree\",\n"));
    out.push_str(&format!("{indent}  \"scanner\": \"source_syntax\",\n"));
    out.push_str(&format!(
        "{indent}  \"source\": \"{}\"",
        json_escape(context.inventory_source)
    ));
    if let Some(root) = context.source_tree_root {
        out.push_str(&format!(",\n{indent}  \"root\": \"{}\"", json_escape(root)));
    }
    if let Some(files) = context.inventory_files {
        out.push_str(&format!(",\n{indent}  \"files_scanned\": {files}"));
    }
    out.push_str(&format!("\n{indent}}}"));
    out
}

#[derive(Debug, Clone, Copy)]
struct ExplainContext<'a> {
    inventory_source: &'a str,
    source_tree_root: Option<&'a str>,
    inventory_files: Option<usize>,
}

impl Default for ExplainContext<'static> {
    fn default() -> Self {
        Self {
            inventory_source: "unknown",
            source_tree_root: None,
            inventory_files: None,
        }
    }
}

fn next_allow_id(cfg: &AllowConfig) -> String {
    let mut index = cfg.allow.len() + 1;
    loop {
        let candidate = format!("allow-{index:04}");
        if !cfg.allow.iter().any(|entry| entry.id == candidate) {
            return candidate;
        }
        index += 1;
    }
}

fn explain_entry_text(
    root: &Path,
    cfg: &AllowConfig,
    entry: &AllowEntry,
    findings: &[Finding],
) -> String {
    let (matching_findings, outcomes) = explain_entry_state(cfg, entry, findings);
    render_explain_entry(root, entry, &matching_findings, &outcomes)
}

fn explain_entry_json(
    root: &Path,
    cfg: &AllowConfig,
    entry: &AllowEntry,
    findings: &[Finding],
    context: ExplainContext<'_>,
) -> String {
    let (matching_findings, outcomes) = explain_entry_state(cfg, entry, findings);
    render_explain_entry_json(root, entry, &matching_findings, &outcomes, context)
}

fn explain_entry_state(
    cfg: &AllowConfig,
    entry: &AllowEntry,
    findings: &[Finding],
) -> (Vec<Finding>, Vec<MatchOutcome>) {
    let matching_findings = findings
        .iter()
        .filter(|finding| score_match(entry, finding).is_some())
        .cloned()
        .collect::<Vec<_>>();
    let mut single_entry_cfg = cfg.clone();
    single_entry_cfg.allow = vec![entry.clone()];
    let outcomes = evaluate(&single_entry_cfg, &matching_findings, CheckMode::NoNew);
    (matching_findings, outcomes)
}

fn render_explain_entry(
    root: &Path,
    entry: &AllowEntry,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("{}\n", entry.id));
    out.push_str(&format!("kind: {}\n", kind_label(entry)));
    out.push_str(&format!("scope: {}\n", entry.path_or_glob()));
    out.push_str(&format!("owner: {}\n", empty_as_none(&entry.owner)));
    out.push_str(&format!(
        "classification: {}\n",
        empty_as_none(&entry.classification)
    ));
    out.push_str(&format!("reason: {}\n", empty_as_none(&entry.reason)));
    out.push_str(&format!("evidence: {}\n", list_or_none(&entry.evidence)));
    let evidence_diagnostics = evidence_reference_diagnostics(root, entry);
    if !evidence_diagnostics.is_empty() {
        out.push_str("\nevidence references:\n");
        for diagnostic in evidence_diagnostics {
            let target = diagnostic
                .target
                .as_ref()
                .map(normalize_path)
                .unwrap_or_else(|| "-".to_string());
            let prefix = diagnostic.prefix.as_deref().unwrap_or("-");
            out.push_str(&format!(
                "- {} prefix={} target={} status={} message={}\n",
                diagnostic.raw,
                prefix,
                target,
                diagnostic.status.as_str(),
                diagnostic.message
            ));
        }
    }
    if !entry.links.is_empty() {
        out.push_str(&format!("links: {}\n", entry.links.join(", ")));
    }
    if let Some(limit) = entry.occurrence_limit {
        out.push_str(&format!("occurrence_limit: {limit}\n"));
    }
    if let Some(created) = &entry.lifecycle.created {
        out.push_str(&format!("created: {created}\n"));
    }
    if let Some(expires) = &entry.lifecycle.expires {
        out.push_str(&format!("expires: {expires}\n"));
    }
    if let Some(review_after) = &entry.lifecycle.review_after {
        out.push_str(&format!("review_after: {review_after}\n"));
    }
    if let Some(last_seen) = &entry.last_seen {
        out.push_str(&format!(
            "last_seen: {}:{}\n",
            last_seen.line, last_seen.column
        ));
    }
    out.push_str(&format!("selector: {}\n\n", selector_summary(entry)));
    out.push_str(&format!(
        "current_status: {}\n",
        explain_status(outcomes).as_str()
    ));
    out.push_str(&format!("current_matches: {}\n", findings.len()));
    out.push_str(&format!("match_outcomes: {}\n", outcome_summary(outcomes)));
    if !findings.is_empty() {
        out.push_str("\ncurrent findings:\n");
        for (index, finding) in findings.iter().enumerate().take(20) {
            let status = outcomes
                .iter()
                .find(|outcome| outcome.finding_index == Some(index))
                .map(|outcome| outcome.status.as_str())
                .unwrap_or("unmatched");
            let package = source_package_name(finding)
                .map(|package| format!(", source_package={package}"))
                .unwrap_or_default();
            out.push_str(&format!(
                "- {status}: {} ({}{})\n",
                finding_location(finding),
                finding.identity.ast_kind,
                package
            ));
        }
        if findings.len() > 20 {
            out.push_str(&format!(
                "- ... {} more matching findings omitted\n",
                findings.len() - 20
            ));
        }
    }
    let attention = outcomes
        .iter()
        .filter(|outcome| outcome.status != MatchStatus::Matched)
        .collect::<Vec<_>>();
    if !attention.is_empty() {
        out.push_str("\nattention:\n");
        for outcome in attention.iter().take(20) {
            out.push_str(&format!(
                "- {}: {}\n",
                outcome.status.as_str(),
                outcome.message
            ));
        }
        if let Some(outcome) = attention.first() {
            let finding = outcome.finding_index.and_then(|index| findings.get(index));
            let kind = work_item_kind(outcome, finding, Some(entry));
            out.push_str("\nnext:\n");
            for action in suggested_actions(&kind).into_iter().take(2) {
                out.push_str(&format!("- action: {action}\n"));
            }
            for command in proof_commands(&kind, finding, Some(entry))
                .into_iter()
                .take(3)
            {
                out.push_str(&format!("- proof: {command}\n"));
            }
        }
    } else if entry.classification == "baseline_debt" {
        out.push_str("\nattention:\n");
        out.push_str(&format!(
            "- baseline_debt: {} is generated baseline_debt and still needs human review\n",
            entry.id
        ));
        let finding = findings.first();
        let kind = "baseline_debt";
        out.push_str("\nnext:\n");
        for action in suggested_actions(kind).into_iter().take(2) {
            out.push_str(&format!("- action: {action}\n"));
        }
        for command in proof_commands(kind, finding, Some(entry))
            .into_iter()
            .take(3)
        {
            out.push_str(&format!("- proof: {command}\n"));
        }
    }
    out.push('\n');
    out.push_str(allow_report::CLAIM_BOUNDARY_TEXT);
    out
}

fn render_explain_entry_json(
    root: &Path,
    entry: &AllowEntry,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    context: ExplainContext<'_>,
) -> String {
    let (suggested_actions, proof_commands) = explain_next_steps(entry, findings, outcomes);
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": {},\n",
        allow_report::EXPLAIN_SCHEMA_VERSION
    ));
    out.push_str(&format!(
        "  \"schema_id\": \"{}\",\n",
        allow_report::EXPLAIN_SCHEMA_ID
    ));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str("  \"command\": \"explain\",\n");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        json_string_array(allow_report::CLAIM_BOUNDARY)
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        json_string_array(allow_report::SCANNER_LIMITATIONS)
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&explain_inventory_json(context, "  "));
    out.push_str(",\n");
    out.push_str("  \"allow_entry\": ");
    out.push_str(&allow_entry_json(entry, "  "));
    out.push_str(",\n");
    out.push_str(&format!(
        "  \"summary\": {{\n    \"current_status\": \"{}\",\n    \"current_matches\": {},\n    \"match_outcomes\": {}\n  }},\n",
        explain_status(outcomes).as_str(),
        findings.len(),
        outcomes.len()
    ));
    out.push_str("  \"evidence_references\": [\n");
    for (index, diagnostic) in evidence_reference_diagnostics(root, entry)
        .iter()
        .enumerate()
    {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&evidence_reference_diagnostic_json(diagnostic, "  "));
    }
    out.push_str("\n  ],\n");
    out.push_str("  \"current_findings\": [\n");
    for (index, finding) in findings.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        let status = outcomes
            .iter()
            .find(|outcome| outcome.finding_index == Some(index))
            .map(|outcome| outcome.status.as_str())
            .unwrap_or("unmatched");
        out.push_str(&explain_finding_json(finding, status, "  "));
    }
    out.push_str("\n  ],\n");
    out.push_str("  \"match_outcomes\": [\n");
    for (index, outcome) in outcomes.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&match_outcome_json(outcome, "  "));
    }
    out.push_str("\n  ],\n");
    out.push_str("  \"next\": {\n");
    out.push_str(&format!(
        "    \"suggested_actions\": {},\n",
        json_string_array(&suggested_actions)
    ));
    out.push_str(&format!(
        "    \"proof_commands\": {}\n",
        json_string_array(&proof_commands)
    ));
    out.push_str("  }\n");
    out.push_str("}\n");
    out
}

fn explain_inventory_json(context: ExplainContext<'_>, indent: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("{indent}  \"scope\": \"source_tree\",\n"));
    out.push_str(&format!("{indent}  \"scanner\": \"source_syntax\",\n"));
    out.push_str(&format!(
        "{indent}  \"source\": \"{}\"",
        json_escape(context.inventory_source)
    ));
    if let Some(root) = context.source_tree_root {
        out.push_str(&format!(",\n{indent}  \"root\": \"{}\"", json_escape(root)));
    }
    if let Some(files) = context.inventory_files {
        out.push_str(&format!(",\n{indent}  \"files_scanned\": {files}"));
    }
    out.push_str(&format!("\n{indent}}}"));
    out
}

fn allow_entry_json(entry: &AllowEntry, indent: &str) -> String {
    let path = entry.path.as_ref().map(normalize_path);
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "{indent}  \"id\": \"{}\",\n",
        json_escape(&entry.id)
    ));
    out.push_str(&format!("{indent}  \"kind\": \"{}\",\n", entry.kind));
    out.push_str(&format!(
        "{indent}  \"family\": {},\n",
        option_json_string(entry.family.as_deref())
    ));
    out.push_str(&format!(
        "{indent}  \"scope\": \"{}\",\n",
        json_escape(&entry.path_or_glob())
    ));
    out.push_str(&format!(
        "{indent}  \"path\": {},\n",
        option_json_string(path.as_deref())
    ));
    out.push_str(&format!(
        "{indent}  \"glob\": {},\n",
        option_json_string(entry.glob.as_deref())
    ));
    out.push_str(&format!(
        "{indent}  \"owner\": \"{}\",\n",
        json_escape(&entry.owner)
    ));
    out.push_str(&format!(
        "{indent}  \"classification\": \"{}\",\n",
        json_escape(&entry.classification)
    ));
    out.push_str(&format!(
        "{indent}  \"reason\": \"{}\",\n",
        json_escape(&entry.reason)
    ));
    out.push_str(&format!(
        "{indent}  \"evidence\": {},\n",
        json_string_array(&entry.evidence)
    ));
    out.push_str(&format!(
        "{indent}  \"links\": {},\n",
        json_string_array(&entry.links)
    ));
    out.push_str(&format!(
        "{indent}  \"occurrence_limit\": {},\n",
        option_u32_json(entry.occurrence_limit)
    ));
    out.push_str(&format!(
        "{indent}  \"lifecycle\": {},\n",
        lifecycle_json(&entry.lifecycle, indent)
    ));
    out.push_str(&format!(
        "{indent}  \"selector\": {},\n",
        selector_json(&entry.selector, indent)
    ));
    out.push_str(&format!(
        "{indent}  \"last_seen\": {}\n",
        last_seen_json(entry.last_seen.as_ref(), indent)
    ));
    out.push_str(&format!("{indent}}}"));
    out
}

fn lifecycle_json(lifecycle: &Lifecycle, indent: &str) -> String {
    format!(
        "{{\n{indent}    \"created\": {},\n{indent}    \"review_after\": {},\n{indent}    \"expires\": {}\n{indent}  }}",
        option_json_string(lifecycle.created.as_deref()),
        option_json_string(lifecycle.review_after.as_deref()),
        option_json_string(lifecycle.expires.as_deref())
    )
}

fn selector_json(selector: &Selector, indent: &str) -> String {
    format!(
        "{{\n{indent}    \"ast_kind\": {},\n{indent}    \"container\": {},\n{indent}    \"callee\": {},\n{indent}    \"macro_name\": {},\n{indent}    \"lint\": {},\n{indent}    \"symbol\": {},\n{indent}    \"receiver_fingerprint\": {},\n{indent}    \"target_fingerprint\": {},\n{indent}    \"normalized_snippet_hash\": {},\n{indent}    \"line_hint\": {},\n{indent}    \"glob\": {}\n{indent}  }}",
        option_json_string(selector.ast_kind.as_deref()),
        option_json_string(selector.container.as_deref()),
        option_json_string(selector.callee.as_deref()),
        option_json_string(selector.macro_name.as_deref()),
        option_json_string(selector.lint.as_deref()),
        option_json_string(selector.symbol.as_deref()),
        option_json_string(selector.receiver_fingerprint.as_deref()),
        option_json_string(selector.target_fingerprint.as_deref()),
        option_json_string(selector.normalized_snippet_hash.as_deref()),
        option_u32_json(selector.line_hint),
        option_json_string(selector.glob.as_deref())
    )
}

fn last_seen_json(last_seen: Option<&allow_core::LastSeen>, indent: &str) -> String {
    last_seen
        .map(|last_seen| {
            format!(
                "{{\n{indent}    \"line\": {},\n{indent}    \"column\": {}\n{indent}  }}",
                last_seen.line, last_seen.column
            )
        })
        .unwrap_or_else(|| "null".to_string())
}

fn evidence_reference_diagnostic_json(
    diagnostic: &allow_policy::EvidenceReferenceDiagnostic,
    indent: &str,
) -> String {
    let target = diagnostic.target.as_ref().map(normalize_path);
    format!(
        "{indent}  {{\n{indent}    \"raw\": \"{}\",\n{indent}    \"prefix\": {},\n{indent}    \"target\": {},\n{indent}    \"status\": \"{}\",\n{indent}    \"message\": \"{}\"\n{indent}  }}",
        json_escape(&diagnostic.raw),
        option_json_string(diagnostic.prefix.as_deref()),
        option_json_string(target.as_deref()),
        diagnostic.status.as_str(),
        json_escape(&diagnostic.message)
    )
}

fn explain_finding_json(finding: &Finding, status: &str, indent: &str) -> String {
    let span = finding.span.as_ref();
    format!(
        "{indent}  {{\n{indent}    \"status\": \"{}\",\n{indent}    \"kind\": \"{}\",\n{indent}    \"family\": {},\n{indent}    \"path\": \"{}\",\n{indent}    \"line\": {},\n{indent}    \"column\": {},\n{indent}    \"source_package\": {},\n{indent}    \"identity\": {},\n{indent}    \"message\": \"{}\"\n{indent}  }}",
        json_escape(status),
        finding.kind,
        option_json_string(finding.family.as_deref()),
        json_escape(&normalize_path(&finding.path)),
        option_u32_json(span.map(|span| span.line)),
        option_u32_json(span.map(|span| span.column)),
        option_json_string(source_package_name(finding).as_deref()),
        structural_identity_json(&finding.identity, indent),
        json_escape(&finding.message)
    )
}

fn structural_identity_json(identity: &allow_core::StructuralIdentity, indent: &str) -> String {
    format!(
        "{{\n{indent}      \"language\": \"{}\",\n{indent}      \"crate_name\": {},\n{indent}      \"module\": {},\n{indent}      \"container\": {},\n{indent}      \"ast_kind\": \"{}\",\n{indent}      \"symbol\": {},\n{indent}      \"callee\": {},\n{indent}      \"macro_name\": {},\n{indent}      \"lint\": {},\n{indent}      \"receiver_fingerprint\": {},\n{indent}      \"target_fingerprint\": {},\n{indent}      \"normalized_snippet_hash\": {},\n{indent}      \"line_hint\": {},\n{indent}      \"column_hint\": {}\n{indent}    }}",
        json_escape(&identity.language),
        option_json_string(identity.crate_name.as_deref()),
        option_json_string(identity.module.as_deref()),
        option_json_string(identity.container.as_deref()),
        json_escape(&identity.ast_kind),
        option_json_string(identity.symbol.as_deref()),
        option_json_string(identity.callee.as_deref()),
        option_json_string(identity.macro_name.as_deref()),
        option_json_string(identity.lint.as_deref()),
        option_json_string(identity.receiver_fingerprint.as_deref()),
        option_json_string(identity.target_fingerprint.as_deref()),
        option_json_string(identity.normalized_snippet_hash.as_deref()),
        option_u32_json(identity.line_hint),
        option_u32_json(identity.column_hint)
    )
}

fn match_outcome_json(outcome: &MatchOutcome, indent: &str) -> String {
    format!(
        "{indent}  {{\n{indent}    \"status\": \"{}\",\n{indent}    \"allow_id\": {},\n{indent}    \"finding_index\": {},\n{indent}    \"score\": {},\n{indent}    \"message\": \"{}\"\n{indent}  }}",
        outcome.status.as_str(),
        option_json_string(outcome.allow_id.as_deref()),
        option_usize_json(outcome.finding_index),
        outcome.score,
        json_escape(&outcome.message)
    )
}

fn explain_next_steps(
    entry: &AllowEntry,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
) -> (Vec<String>, Vec<String>) {
    let attention = outcomes
        .iter()
        .filter(|outcome| outcome.status != MatchStatus::Matched)
        .collect::<Vec<_>>();
    if let Some(outcome) = attention.first() {
        let finding = outcome.finding_index.and_then(|index| findings.get(index));
        let kind = work_item_kind(outcome, finding, Some(entry));
        return (
            suggested_actions(&kind).into_iter().take(2).collect(),
            proof_commands(&kind, finding, Some(entry))
                .into_iter()
                .take(3)
                .collect(),
        );
    }
    if entry.classification == "baseline_debt" {
        let finding = findings.first();
        let kind = "baseline_debt";
        return (
            suggested_actions(kind).into_iter().take(2).collect(),
            proof_commands(kind, finding, Some(entry))
                .into_iter()
                .take(3)
                .collect(),
        );
    }
    (Vec::new(), Vec::new())
}

fn kind_label(entry: &AllowEntry) -> String {
    entry
        .family
        .as_ref()
        .map(|family| format!("{}.{}", entry.kind, family))
        .unwrap_or_else(|| entry.kind.to_string())
}

fn empty_as_none(value: &str) -> &str {
    if value.trim().is_empty() {
        "none"
    } else {
        value
    }
}

fn list_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

fn selector_summary(entry: &AllowEntry) -> String {
    let selector = &entry.selector;
    let mut fields = Vec::new();
    if let Some(value) = &selector.ast_kind {
        fields.push(format!("ast_kind={value}"));
    }
    if let Some(value) = &selector.container {
        fields.push(format!("container={value}"));
    }
    if let Some(value) = &selector.callee {
        fields.push(format!("callee={value}"));
    }
    if let Some(value) = &selector.macro_name {
        fields.push(format!("macro_name={value}"));
    }
    if let Some(value) = &selector.lint {
        fields.push(format!("lint={value}"));
    }
    if let Some(value) = &selector.symbol {
        fields.push(format!("symbol={value}"));
    }
    if let Some(value) = &selector.receiver_fingerprint {
        fields.push(format!("receiver={value}"));
    }
    if let Some(value) = &selector.target_fingerprint {
        fields.push(format!("target={value}"));
    }
    if let Some(value) = &selector.normalized_snippet_hash {
        fields.push(format!("normalized_snippet_hash={value}"));
    }
    if let Some(value) = selector.line_hint {
        fields.push(format!("line_hint={value}"));
    }
    if let Some(value) = &selector.glob {
        fields.push(format!("glob={value}"));
    }
    if fields.is_empty() {
        "none".to_string()
    } else {
        fields.join(", ")
    }
}

fn explain_status(outcomes: &[MatchOutcome]) -> MatchStatus {
    for status in [
        MatchStatus::New,
        MatchStatus::Expired,
        MatchStatus::EvidenceMissing,
        MatchStatus::MissingRequiredField,
        MatchStatus::InvalidSelector,
        MatchStatus::Ambiguous,
        MatchStatus::BaselineDebt,
        MatchStatus::Stale,
        MatchStatus::ReviewDue,
    ] {
        if outcomes.iter().any(|outcome| outcome.status == status) {
            return status;
        }
    }
    MatchStatus::Matched
}

fn outcome_summary(outcomes: &[MatchOutcome]) -> String {
    let parts = [
        MatchStatus::Matched,
        MatchStatus::New,
        MatchStatus::Expired,
        MatchStatus::ReviewDue,
        MatchStatus::Stale,
        MatchStatus::Ambiguous,
        MatchStatus::InvalidSelector,
        MatchStatus::MissingRequiredField,
        MatchStatus::EvidenceMissing,
        MatchStatus::BaselineDebt,
    ]
    .into_iter()
    .filter_map(|status| {
        let count = outcomes
            .iter()
            .filter(|outcome| outcome.status == status)
            .count();
        (count > 0).then(|| format!("{}={count}", status.as_str()))
    })
    .collect::<Vec<_>>();
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(", ")
    }
}

fn cmd_propose(args: &ProposeArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings, inventory_facts) = load_world(
        args.root.root.as_deref(),
        args.config.as_deref(),
        false,
        args.kind.as_deref(),
        args.include_untracked,
    )?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::Audit);
    let mut proposed = cfg.clone();
    let start = proposed.allow.len() + 1;
    let mut proposed_entries = 0;
    for (n, outcome) in outcomes
        .iter()
        .filter(|o| o.status == allow_core::MatchStatus::New)
        .enumerate()
    {
        if let Some(finding) = outcome.finding_index.and_then(|idx| findings.get(idx)) {
            proposed
                .allow
                .push(entry_from_finding(finding, start + n, &args.expires));
            proposed_entries += 1;
        }
    }
    let rendered = render_policy(&proposed);
    if let Some(path) = &args.write {
        write_file_no_overwrite(path, &rendered, args.force)?;
    } else {
        println!("{rendered}");
    }
    let root_text = source_tree_root_text(&root);
    let context = ProposeContext {
        inventory_source: inventory_facts.source.as_str(),
        source_tree_root: Some(&root_text),
        inventory_files: inventory_facts.files_scanned,
        kind_filter: args.kind.as_deref(),
    };
    let summary = match args.summary_format {
        ProposeSummaryFormat::Human => render_propose_summary(
            findings.len(),
            proposed_entries,
            args.expires.as_str(),
            args.write.as_deref(),
        ),
        ProposeSummaryFormat::Json => render_propose_summary_json(
            findings.len(),
            proposed_entries,
            args.expires.as_str(),
            args.write.as_deref(),
            args.force,
            context,
        ),
    };
    if let Some(path) = &args.summary_output {
        write_file(path, &summary)?;
    } else {
        eprintln!("{summary}");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct ProposeContext<'a> {
    inventory_source: &'a str,
    source_tree_root: Option<&'a str>,
    inventory_files: Option<usize>,
    kind_filter: Option<&'a str>,
}

impl Default for ProposeContext<'static> {
    fn default() -> Self {
        Self {
            inventory_source: "unknown",
            source_tree_root: None,
            inventory_files: None,
            kind_filter: None,
        }
    }
}

fn render_propose_summary(
    findings: usize,
    proposed_entries: usize,
    expires: &str,
    output: Option<&Path>,
) -> String {
    let mut out = String::new();
    out.push_str("cargo-allow propose summary\n");
    out.push_str(&format!("findings scanned: {findings}\n"));
    out.push_str(&format!(
        "baseline_debt entries proposed: {proposed_entries}\n"
    ));
    out.push_str("owner: unowned\n");
    out.push_str("classification: baseline_debt\n");
    out.push_str("reason: Generated by cargo-allow propose; requires human review.\n");
    out.push_str(&format!("expires: {expires}\n"));
    if let Some(output) = output {
        out.push_str(&format!("output: {}\n", output.display()));
    } else {
        out.push_str("output: stdout\n");
    }
    out.push_str(
        "claim boundary: proposal only; generated debt still requires human review and evidence.\n",
    );
    out
}

fn render_propose_summary_json(
    findings: usize,
    proposed_entries: usize,
    expires: &str,
    output: Option<&Path>,
    force: bool,
    context: ProposeContext<'_>,
) -> String {
    let output_text = output.map(|path| path.display().to_string());
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": {},\n",
        allow_report::PROPOSE_SCHEMA_VERSION
    ));
    out.push_str(&format!(
        "  \"schema_id\": \"{}\",\n",
        allow_report::PROPOSE_SCHEMA_ID
    ));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str("  \"command\": \"propose\",\n");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        json_string_array(allow_report::CLAIM_BOUNDARY)
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        json_string_array(allow_report::SCANNER_LIMITATIONS)
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&propose_inventory_json(context, "  "));
    out.push_str(",\n");
    out.push_str("  \"options\": {\n");
    out.push_str(&format!(
        "    \"kind\": {},\n",
        option_json_string(context.kind_filter)
    ));
    out.push_str(&format!("    \"expires\": \"{}\",\n", json_escape(expires)));
    out.push_str(&format!(
        "    \"policy_output\": {},\n",
        option_json_string(output_text.as_deref())
    ));
    out.push_str(&format!("    \"force\": {}\n", force));
    out.push_str("  },\n");
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!("    \"findings_scanned\": {findings},\n"));
    out.push_str(&format!(
        "    \"baseline_debt_entries_proposed\": {proposed_entries}\n"
    ));
    out.push_str("  },\n");
    out.push_str("  \"generated_entry_defaults\": {\n");
    out.push_str("    \"owner\": \"unowned\",\n");
    out.push_str("    \"classification\": \"baseline_debt\",\n");
    out.push_str("    \"reason\": \"Generated by cargo-allow propose; requires human review.\",\n");
    out.push_str(&format!("    \"expires\": \"{}\"\n", json_escape(expires)));
    out.push_str("  }\n");
    out.push_str("}\n");
    out
}

fn propose_inventory_json(context: ProposeContext<'_>, indent: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("{indent}  \"scope\": \"source_tree\",\n"));
    out.push_str(&format!("{indent}  \"scanner\": \"source_syntax\",\n"));
    out.push_str(&format!(
        "{indent}  \"source\": \"{}\"",
        json_escape(context.inventory_source)
    ));
    if let Some(root) = context.source_tree_root {
        out.push_str(&format!(",\n{indent}  \"root\": \"{}\"", json_escape(root)));
    }
    if let Some(files) = context.inventory_files {
        out.push_str(&format!(",\n{indent}  \"files_scanned\": {files}"));
    }
    out.push_str(&format!("\n{indent}}}"));
    out
}

fn cmd_worklist(args: &WorklistArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings, inventory_facts) = load_world_with_evidence_validation(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        args.kind.as_deref(),
        args.include_untracked,
        false,
    )?;
    let report_cfg = report_config(&cfg, args.kind.as_deref())?;
    let outcomes = evaluate(&report_cfg, &findings, CheckMode::NoNew);
    let mut items = work_items_from_outcomes(&report_cfg, &findings, &outcomes);
    items.extend(work_items_from_policy_advisories(
        &report_cfg,
        &findings,
        &outcomes,
        items.len() + 1,
    ));
    items.extend(work_items_from_evidence_diagnostics(
        &root,
        &report_cfg,
        items.len() + 1,
    ));
    let filters = WorklistFilters {
        kind: args.kind.as_deref(),
        family: args.family.as_deref(),
        item_kind: args.item_kind.as_deref(),
        status: args.status.as_deref(),
        allow_id: args.allow_id.as_deref(),
        path: args.path.as_deref(),
        source_package: args.source_package.as_deref(),
        owner: args.owner.as_deref(),
        classification: args.classification.as_deref(),
        baseline_debt: args.baseline_debt,
        broad_scope: args.broad_scope,
        risk: args.risk.as_deref(),
        difficulty: args.difficulty.as_deref(),
        missing_evidence: args.missing_evidence,
    };
    let mut items = filter_work_items(items, filters);
    sort_work_items(&mut items);
    renumber_work_items(&mut items);
    let root_text = source_tree_root_text(&root);
    let context = WorklistContext {
        inventory_source: inventory_facts.source.as_str(),
        source_tree_root: Some(&root_text),
        inventory_files: inventory_facts.files_scanned,
        filters,
    };
    let text = match args.format {
        WorklistFormat::Json => render_worklist_json_with_context(&items, context),
        WorklistFormat::Human => render_worklist_human_with_context(&items, context),
    };
    if let Some(path) = &args.output {
        write_file(path, &text)?;
    } else {
        println!("{text}");
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkItem {
    id: String,
    kind: String,
    exception_kind: Option<String>,
    family: Option<String>,
    owner: Option<String>,
    classification: Option<String>,
    reason: Option<String>,
    created: Option<String>,
    review_after: Option<String>,
    expires: Option<String>,
    evidence_count: Option<usize>,
    risk: &'static str,
    difficulty: &'static str,
    status: MatchStatus,
    allow_id: Option<String>,
    finding_index: Option<usize>,
    path: Option<String>,
    source_package: Option<String>,
    message: String,
    suggested_actions: Vec<String>,
    proof_commands: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct WorklistContext<'a> {
    inventory_source: &'a str,
    source_tree_root: Option<&'a str>,
    inventory_files: Option<usize>,
    filters: WorklistFilters<'a>,
}

#[derive(Debug, Clone, Copy, Default)]
struct WorklistFilters<'a> {
    kind: Option<&'a str>,
    family: Option<&'a str>,
    item_kind: Option<&'a str>,
    status: Option<&'a str>,
    allow_id: Option<&'a str>,
    path: Option<&'a str>,
    source_package: Option<&'a str>,
    owner: Option<&'a str>,
    classification: Option<&'a str>,
    baseline_debt: bool,
    broad_scope: bool,
    risk: Option<&'a str>,
    difficulty: Option<&'a str>,
    missing_evidence: bool,
}

impl Default for WorklistContext<'static> {
    fn default() -> Self {
        Self {
            inventory_source: "unknown",
            source_tree_root: None,
            inventory_files: None,
            filters: WorklistFilters::default(),
        }
    }
}

fn work_items_from_outcomes(
    cfg: &AllowConfig,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
) -> Vec<WorkItem> {
    outcomes
        .iter()
        .filter(|outcome| outcome.status != MatchStatus::Matched)
        .enumerate()
        .map(|(index, outcome)| work_item_from_outcome(index + 1, cfg, findings, outcome))
        .collect()
}

fn filter_work_items(items: Vec<WorkItem>, filters: WorklistFilters<'_>) -> Vec<WorkItem> {
    items
        .into_iter()
        .filter(|item| {
            filters
                .family
                .map(|family| item.family.as_deref() == Some(family))
                .unwrap_or(true)
                && filters
                    .item_kind
                    .map(|item_kind| item.kind == item_kind)
                    .unwrap_or(true)
                && filters
                    .status
                    .map(|status| item.status.as_str() == status)
                    .unwrap_or(true)
                && filters
                    .allow_id
                    .map(|allow_id| item.allow_id.as_deref() == Some(allow_id))
                    .unwrap_or(true)
                && filters
                    .path
                    .map(|path| {
                        item.path
                            .as_deref()
                            .map(|item_path| source_tree_path_matches_filter(item_path, path))
                            .unwrap_or(false)
                    })
                    .unwrap_or(true)
                && filters
                    .source_package
                    .map(|source_package| item.source_package.as_deref() == Some(source_package))
                    .unwrap_or(true)
                && filters
                    .owner
                    .map(|owner| item.owner.as_deref() == Some(owner))
                    .unwrap_or(true)
                && filters
                    .classification
                    .map(|classification| item.classification.as_deref() == Some(classification))
                    .unwrap_or(true)
                && (!filters.baseline_debt
                    || item.kind == "baseline_debt"
                    || item.classification.as_deref() == Some("baseline_debt")
                    || item.status == MatchStatus::BaselineDebt)
                && (!filters.broad_scope || item.kind == "broad_scope")
                && filters.risk.map(|risk| item.risk == risk).unwrap_or(true)
                && filters
                    .difficulty
                    .map(|difficulty| item.difficulty == difficulty)
                    .unwrap_or(true)
                && (!filters.missing_evidence || item.evidence_count == Some(0))
        })
        .collect()
}

fn source_tree_path_matches_filter(item_path: &str, filter_path: &str) -> bool {
    let item_path = normalize_path(item_path);
    let filter_path = normalize_path(filter_path);
    let filter_path = filter_path.trim_end_matches('/');
    if filter_path.is_empty() || filter_path == "." {
        return true;
    }
    item_path == filter_path
        || item_path
            .strip_prefix(filter_path)
            .map(|suffix| suffix.starts_with('/'))
            .unwrap_or(false)
        || (scope_has_wildcard(&item_path) && glob_matches_str(&item_path, filter_path))
}

fn sort_work_items(items: &mut [WorkItem]) {
    items.sort_by(|left, right| {
        work_item_risk_rank(left.risk)
            .cmp(&work_item_risk_rank(right.risk))
            .then_with(|| {
                work_item_difficulty_rank(left.difficulty)
                    .cmp(&work_item_difficulty_rank(right.difficulty))
            })
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.allow_id.cmp(&right.allow_id))
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn work_item_risk_rank(risk: &str) -> u8 {
    match risk {
        "high" => 0,
        "medium" => 1,
        "low" => 2,
        _ => 3,
    }
}

fn work_item_difficulty_rank(difficulty: &str) -> u8 {
    match difficulty {
        "small" => 0,
        "medium" => 1,
        _ => 2,
    }
}

fn renumber_work_items(items: &mut [WorkItem]) {
    for (index, item) in items.iter_mut().enumerate() {
        item.id = format!("work-{}-{:04}", item.kind.replace('_', "-"), index + 1);
    }
}

fn work_item_from_outcome(
    item_index: usize,
    cfg: &AllowConfig,
    findings: &[Finding],
    outcome: &MatchOutcome,
) -> WorkItem {
    let finding = outcome.finding_index.and_then(|idx| findings.get(idx));
    let entry = outcome
        .allow_id
        .as_deref()
        .and_then(|id| cfg.allow.iter().find(|entry| entry.id == id));
    let kind = work_item_kind(outcome, finding, entry);
    let path = finding
        .map(|finding| normalize_path(&finding.path))
        .or_else(|| entry.map(|entry| entry.path_or_glob()));
    let source_package = finding.and_then(source_package_name);
    let exception_kind = work_item_exception_kind(finding, entry);
    let family = exception_family(finding, entry).map(ToOwned::to_owned);
    let mut suggested_actions = suggested_actions(&kind);
    if let Some(package) = &source_package {
        suggested_actions.push(format!(
            "focus source-tree review on package `{package}` without assuming Cargo metadata"
        ));
    }
    WorkItem {
        id: format!("work-{}-{item_index:04}", kind.replace('_', "-")),
        exception_kind,
        family,
        owner: entry.map(|entry| entry.owner.clone()),
        classification: entry.map(|entry| entry.classification.clone()),
        reason: entry.map(|entry| entry.reason.clone()),
        created: entry.and_then(|entry| entry.lifecycle.created.clone()),
        review_after: entry.and_then(|entry| entry.lifecycle.review_after.clone()),
        expires: entry.and_then(|entry| entry.lifecycle.expires.clone()),
        evidence_count: entry.map(|entry| entry.evidence.len()),
        risk: work_item_risk(&kind, outcome.status, finding, entry),
        difficulty: work_item_difficulty(&kind, finding, entry),
        status: outcome.status,
        allow_id: outcome.allow_id.clone(),
        finding_index: outcome.finding_index,
        path,
        source_package,
        message: outcome.message.clone(),
        suggested_actions,
        proof_commands: proof_commands(&kind, finding, entry),
        kind,
    }
}

fn work_items_from_evidence_diagnostics(
    root: &Path,
    cfg: &AllowConfig,
    start_index: usize,
) -> Vec<WorkItem> {
    let mut items = Vec::new();
    for entry in &cfg.allow {
        for diagnostic in evidence_reference_diagnostics(root, entry)
            .into_iter()
            .filter(|diagnostic| {
                matches!(
                    diagnostic.status,
                    allow_policy::EvidenceReferenceStatus::LocalFileMissing
                        | allow_policy::EvidenceReferenceStatus::InvalidLocalPath
                )
            })
        {
            let item_index = start_index + items.len();
            let kind = "broken_evidence_link".to_string();
            items.push(WorkItem {
                id: format!("work-broken-evidence-link-{item_index:04}"),
                kind,
                exception_kind: Some(entry.kind.as_str().to_string()),
                family: entry.family.clone(),
                owner: Some(entry.owner.clone()),
                classification: Some(entry.classification.clone()),
                reason: Some(entry.reason.clone()),
                created: entry.lifecycle.created.clone(),
                review_after: entry.lifecycle.review_after.clone(),
                expires: entry.lifecycle.expires.clone(),
                evidence_count: Some(entry.evidence.len()),
                risk: if entry.kind == FindingKind::Unsafe {
                    "high"
                } else {
                    "medium"
                },
                difficulty: "small",
                status: MatchStatus::EvidenceMissing,
                allow_id: Some(entry.id.clone()),
                finding_index: None,
                path: diagnostic.target.as_ref().map(normalize_path),
                source_package: None,
                message: format!(
                    "{} evidence `{}`: {}",
                    entry.id, diagnostic.raw, diagnostic.message
                ),
                suggested_actions: vec![
                    "restore or commit the referenced local evidence artifact".to_string(),
                    "or update the evidence reference to a valid source-tree-relative path"
                        .to_string(),
                ],
                proof_commands: vec![
                    format!("cargo-allow explain {}", entry.id),
                    "cargo-allow check --mode no-new".to_string(),
                ],
            });
        }
    }
    items
}

fn work_items_from_policy_advisories(
    cfg: &AllowConfig,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    start_index: usize,
) -> Vec<WorkItem> {
    let mut items = Vec::new();
    for entry in &cfg.allow {
        let Some(outcome) = matched_outcome_for_entry(outcomes, entry) else {
            continue;
        };
        let finding = outcome.finding_index.and_then(|idx| findings.get(idx));
        if entry.classification == "baseline_debt" {
            let item_index = start_index + items.len();
            let kind = "baseline_debt".to_string();
            items.push(WorkItem {
                id: format!("work-baseline-debt-{item_index:04}"),
                exception_kind: Some(entry.kind.as_str().to_string()),
                family: exception_family(finding, Some(entry)).map(ToOwned::to_owned),
                owner: Some(entry.owner.clone()),
                classification: Some(entry.classification.clone()),
                reason: Some(entry.reason.clone()),
                created: entry.lifecycle.created.clone(),
                review_after: entry.lifecycle.review_after.clone(),
                expires: entry.lifecycle.expires.clone(),
                evidence_count: Some(entry.evidence.len()),
                risk: work_item_risk(&kind, MatchStatus::BaselineDebt, finding, Some(entry)),
                difficulty: work_item_difficulty(&kind, finding, Some(entry)),
                status: MatchStatus::BaselineDebt,
                allow_id: Some(entry.id.clone()),
                finding_index: outcome.finding_index,
                path: finding
                    .map(|finding| normalize_path(&finding.path))
                    .or_else(|| Some(entry.path_or_glob())),
                source_package: finding.and_then(source_package_name),
                message: format!(
                    "{} is generated baseline_debt and still needs human review",
                    entry.id
                ),
                suggested_actions: suggested_actions(&kind),
                proof_commands: proof_commands(&kind, finding, Some(entry)),
                kind,
            });
            continue;
        }
        if let Some(scope) = entry_broad_scope(entry) {
            let item_index = start_index + items.len();
            let kind = "broad_scope".to_string();
            items.push(WorkItem {
                id: format!("work-broad-scope-{item_index:04}"),
                kind,
                exception_kind: Some(entry.kind.as_str().to_string()),
                family: entry.family.clone(),
                owner: Some(entry.owner.clone()),
                classification: Some(entry.classification.clone()),
                reason: Some(entry.reason.clone()),
                created: entry.lifecycle.created.clone(),
                review_after: entry.lifecycle.review_after.clone(),
                expires: entry.lifecycle.expires.clone(),
                evidence_count: Some(entry.evidence.len()),
                risk: "medium",
                difficulty: "small",
                status: MatchStatus::Matched,
                allow_id: Some(entry.id.clone()),
                finding_index: outcome.finding_index,
                path: Some(scope.clone()),
                source_package: finding.and_then(source_package_name),
                message: format!("{} uses a broad source-tree scope `{}`", entry.id, scope),
                suggested_actions: suggested_actions("broad_scope"),
                proof_commands: proof_commands("broad_scope", finding, Some(entry)),
            });
        }
    }
    items
}

fn matched_outcome_for_entry<'a>(
    outcomes: &'a [MatchOutcome],
    entry: &AllowEntry,
) -> Option<&'a MatchOutcome> {
    outcomes.iter().find(|outcome| {
        outcome.status == MatchStatus::Matched
            && outcome.allow_id.as_deref() == Some(entry.id.as_str())
    })
}

fn entry_broad_scope(entry: &AllowEntry) -> Option<String> {
    entry
        .path
        .as_ref()
        .map(normalize_path)
        .filter(|scope| scope_has_wildcard(scope))
        .or_else(|| entry.glob.clone().filter(|scope| scope_has_wildcard(scope)))
        .or_else(|| {
            entry
                .selector
                .glob
                .clone()
                .filter(|scope| scope_has_wildcard(scope))
        })
}

fn scope_has_wildcard(scope: &str) -> bool {
    scope
        .chars()
        .any(|ch| matches!(ch, '*' | '?' | '[' | ']' | '{' | '}'))
}

fn source_package_name(finding: &Finding) -> Option<String> {
    finding
        .identity
        .crate_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
}

fn work_item_exception_kind(
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
) -> Option<String> {
    finding
        .map(|finding| finding.kind.as_str())
        .or_else(|| entry.map(|entry| entry.kind.as_str()))
        .map(ToOwned::to_owned)
}

fn work_item_kind(
    outcome: &MatchOutcome,
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
) -> String {
    match outcome.status {
        MatchStatus::New if outcome.allow_id.is_some() => "occurrence_limit_exceeded".to_string(),
        MatchStatus::New => "new_unreceipted_finding".to_string(),
        MatchStatus::Expired => "expired_allow".to_string(),
        MatchStatus::Stale => "stale_allow".to_string(),
        MatchStatus::Ambiguous => "ambiguous_selector".to_string(),
        MatchStatus::EvidenceMissing
            if finding
                .map(|finding| finding.kind == FindingKind::Unsafe)
                .or_else(|| entry.map(|entry| entry.kind == FindingKind::Unsafe))
                .unwrap_or(false) =>
        {
            "unsafe_missing_evidence".to_string()
        }
        MatchStatus::EvidenceMissing => "missing_evidence".to_string(),
        MatchStatus::MissingRequiredField => "missing_required_field".to_string(),
        MatchStatus::InvalidSelector => "invalid_selector".to_string(),
        MatchStatus::BaselineDebt => "baseline_debt".to_string(),
        MatchStatus::ReviewDue => "review_due".to_string(),
        MatchStatus::Matched => "matched".to_string(),
    }
}

fn work_item_risk(
    kind: &str,
    status: MatchStatus,
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
) -> &'static str {
    let exception_kind = finding
        .map(|finding| finding.kind)
        .or_else(|| entry.map(|entry| entry.kind));
    let family = exception_family(finding, entry);
    if matches!(status, MatchStatus::Stale) {
        return "low";
    }
    if matches!(
        (exception_kind, family),
        (
            Some(FindingKind::PolicyException),
            Some("process_spawn" | "network_destination")
        )
    ) {
        return "high";
    }
    if matches!(exception_kind, Some(FindingKind::Unsafe)) {
        return "high";
    }
    match (kind, status) {
        ("ambiguous_selector", _) | (_, MatchStatus::Expired) => "high",
        ("new_unreceipted_finding", _) | ("occurrence_limit_exceeded", _) => "medium",
        ("missing_evidence", _) | ("missing_required_field", _) | ("invalid_selector", _) => {
            "medium"
        }
        ("baseline_debt", _) | ("review_due", _) => "medium",
        ("stale_allow", _) => "low",
        _ => "medium",
    }
}

fn work_item_difficulty(
    kind: &str,
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
) -> &'static str {
    let exception_kind = finding
        .map(|finding| finding.kind)
        .or_else(|| entry.map(|entry| entry.kind));
    match kind {
        "stale_allow" => "small",
        "ambiguous_selector" | "invalid_selector" => "small",
        "missing_required_field" | "missing_evidence" => "small",
        "review_due" | "baseline_debt" => "medium",
        "unsafe_missing_evidence" => "medium",
        "new_unreceipted_finding"
            if matches!(
                exception_kind,
                Some(FindingKind::NonRustFile | FindingKind::GeneratedCode)
            ) =>
        {
            "small"
        }
        "new_unreceipted_finding" | "occurrence_limit_exceeded" => "medium",
        _ => "medium",
    }
}

fn exception_family<'a>(
    finding: Option<&'a Finding>,
    entry: Option<&'a AllowEntry>,
) -> Option<&'a str> {
    finding
        .and_then(|finding| finding.family.as_deref())
        .or_else(|| entry.and_then(|entry| entry.family.as_deref()))
}

fn suggested_actions(kind: &str) -> Vec<String> {
    match kind {
        "new_unreceipted_finding" => vec![
            "remove the new source exception if it is accidental".to_string(),
            "or add a reviewed allow entry with owner, reason, scope, evidence, and lifecycle"
                .to_string(),
        ],
        "occurrence_limit_exceeded" => vec![
            "reduce the current findings back to the baseline count".to_string(),
            "or split the added occurrence into a reviewed allow entry".to_string(),
        ],
        "expired_allow" => vec![
            "remove the expired allow if the exception is gone".to_string(),
            "or re-review with fresh evidence before changing lifecycle dates".to_string(),
        ],
        "stale_allow" => vec![
            "remove the stale allow entry if the exception no longer exists".to_string(),
            "or narrow/update the selector if the code moved without broadening scope".to_string(),
        ],
        "ambiguous_selector" => vec![
            "narrow selectors so each finding matches exactly one allow entry".to_string(),
            "prefer structural fields such as container, callee, lint, and snippet hash"
                .to_string(),
        ],
        "unsafe_missing_evidence" => vec![
            "add unsafe-review, test, spec, or boundary evidence for the unsafe exception"
                .to_string(),
            "keep the selector scoped to the reviewed unsafe boundary".to_string(),
        ],
        "missing_evidence" => {
            vec!["add evidence that supports the exception reason".to_string()]
        }
        "missing_required_field" => vec![
            "fill the required owner, reason, classification, lifecycle, or evidence field"
                .to_string(),
        ],
        "invalid_selector" => {
            vec!["replace line-only or invalid selector data with structural identity".to_string()]
        }
        "baseline_debt" => vec![
            "replace generated baseline debt with a reviewed allow entry".to_string(),
            "or remove the underlying exception".to_string(),
        ],
        "review_due" => {
            vec!["review the retained exception and update evidence or remove it".to_string()]
        }
        "broad_scope" => vec![
            "replace the broad glob with exact paths or a narrower glob where practical"
                .to_string(),
            "keep broad source-tree scope intentional, reviewed, and evidenced".to_string(),
        ],
        _ => vec!["inspect the outcome and update policy or source accordingly".to_string()],
    }
}

fn proof_commands(
    kind: &str,
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
) -> Vec<String> {
    let mut commands = Vec::new();
    if let Some(allow_id) = entry.map(|entry| entry.id.as_str()) {
        commands.push(format!("cargo-allow explain {allow_id}"));
    }
    if let Some(kind_arg) = worklist_kind_arg(finding, entry) {
        commands.push(format!("cargo-allow check --kind {kind_arg} --mode no-new"));
        if let Some(shortcut_arg) = worklist_shortcut_arg(kind) {
            commands.push(format!(
                "cargo-allow worklist --{shortcut_arg} --format json"
            ));
        }
        commands.push(format!(
            "cargo-allow worklist --kind {kind_arg} --format json"
        ));
    } else {
        commands.push("cargo-allow check --mode no-new".to_string());
        commands.push("cargo-allow worklist --format json".to_string());
    }
    if kind == "unsafe_missing_evidence" && !commands.iter().any(|cmd| cmd.contains("unsafe")) {
        commands.push("cargo-allow check --kind unsafe --mode no-new".to_string());
    }
    commands
}

fn worklist_shortcut_arg(kind: &str) -> Option<&'static str> {
    match kind {
        "baseline_debt" => Some("baseline-debt"),
        "broad_scope" => Some("broad-scope"),
        _ => None,
    }
}

fn worklist_kind_arg(
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
) -> Option<&'static str> {
    let exception_kind = finding
        .map(|finding| finding.kind)
        .or_else(|| entry.map(|entry| entry.kind))?;
    match exception_kind {
        FindingKind::Panic => Some("panic"),
        FindingKind::Unsafe => Some("unsafe"),
        FindingKind::LintException => Some("lint-exception"),
        FindingKind::NonRustFile => Some("non-rust"),
        FindingKind::GeneratedCode => Some("generated"),
        FindingKind::PolicyException => policy_exception_kind_arg(
            finding
                .and_then(|finding| finding.family.as_deref())
                .or_else(|| entry.and_then(|entry| entry.family.as_deref())),
        ),
    }
}

fn policy_exception_kind_arg(family: Option<&str>) -> Option<&'static str> {
    match family {
        Some("executable_file") => Some("executable"),
        Some("github_workflow" | "workflow_external_action") => Some("workflow"),
        Some("dependency_surface") => Some("dependency-surface"),
        Some("process_spawn") => Some("process"),
        Some("network_destination") => Some("network"),
        _ => None,
    }
}

fn render_worklist_json_with_context(items: &[WorkItem], context: WorklistContext<'_>) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": {},\n",
        allow_report::WORKLIST_SCHEMA_VERSION
    ));
    out.push_str(&format!(
        "  \"schema_id\": \"{}\",\n",
        allow_report::WORKLIST_SCHEMA_ID
    ));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str("  \"command\": \"worklist\",\n");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        json_string_array(allow_report::CLAIM_BOUNDARY)
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        json_string_array(allow_report::SCANNER_LIMITATIONS)
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&worklist_inventory_json(context, "  "));
    out.push_str(",\n");
    out.push_str("  \"filters\": ");
    out.push_str(&worklist_filters_json(context.filters, "  "));
    out.push_str(",\n");
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!("    \"work_items\": {},\n", items.len()));
    out.push_str(&format!("    \"high\": {},\n", risk_count(items, "high")));
    out.push_str(&format!(
        "    \"medium\": {},\n",
        risk_count(items, "medium")
    ));
    out.push_str(&format!("    \"low\": {},\n", risk_count(items, "low")));
    out.push_str(&format!(
        "    \"small_difficulty\": {},\n",
        difficulty_count(items, "small")
    ));
    out.push_str(&format!(
        "    \"medium_difficulty\": {}\n",
        difficulty_count(items, "medium")
    ));
    out.push_str("  },\n");
    out.push_str("  \"work_items\": [\n");
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&render_work_item_json(item));
    }
    out.push_str("\n  ]\n");
    out.push_str("}\n");
    out
}

fn render_work_item_json(item: &WorkItem) -> String {
    let mut out = String::new();
    out.push_str("    {\n");
    out.push_str(&format!("      \"id\": \"{}\",\n", json_escape(&item.id)));
    out.push_str(&format!(
        "      \"kind\": \"{}\",\n",
        json_escape(&item.kind)
    ));
    out.push_str(&format!(
        "      \"exception_kind\": {},\n",
        option_json_string(item.exception_kind.as_deref())
    ));
    out.push_str(&format!(
        "      \"family\": {},\n",
        option_json_string(item.family.as_deref())
    ));
    out.push_str(&format!(
        "      \"owner\": {},\n",
        option_json_string(item.owner.as_deref())
    ));
    out.push_str(&format!(
        "      \"classification\": {},\n",
        option_json_string(item.classification.as_deref())
    ));
    out.push_str(&format!(
        "      \"reason\": {},\n",
        option_json_string(item.reason.as_deref())
    ));
    out.push_str(&format!(
        "      \"created\": {},\n",
        option_json_string(item.created.as_deref())
    ));
    out.push_str(&format!(
        "      \"review_after\": {},\n",
        option_json_string(item.review_after.as_deref())
    ));
    out.push_str(&format!(
        "      \"expires\": {},\n",
        option_json_string(item.expires.as_deref())
    ));
    out.push_str(&format!(
        "      \"evidence_count\": {},\n",
        option_usize_json(item.evidence_count)
    ));
    out.push_str(&format!("      \"risk\": \"{}\",\n", item.risk));
    out.push_str(&format!("      \"difficulty\": \"{}\",\n", item.difficulty));
    out.push_str(&format!(
        "      \"status\": \"{}\",\n",
        item.status.as_str()
    ));
    out.push_str(&format!(
        "      \"allow_id\": {},\n",
        option_json_string(item.allow_id.as_deref())
    ));
    out.push_str(&format!(
        "      \"finding_index\": {},\n",
        item.finding_index
            .map(|index| index.to_string())
            .unwrap_or_else(|| "null".to_string())
    ));
    out.push_str(&format!(
        "      \"path\": {},\n",
        option_json_string(item.path.as_deref())
    ));
    out.push_str(&format!(
        "      \"source_package\": {},\n",
        option_json_string(item.source_package.as_deref())
    ));
    out.push_str(&format!(
        "      \"message\": \"{}\",\n",
        json_escape(&item.message)
    ));
    out.push_str(&format!(
        "      \"suggested_actions\": {},\n",
        json_string_array(&item.suggested_actions)
    ));
    out.push_str(&format!(
        "      \"proof_commands\": {}\n",
        json_string_array(&item.proof_commands)
    ));
    out.push_str("    }");
    out
}

fn render_worklist_human_with_context(items: &[WorkItem], context: WorklistContext<'_>) -> String {
    let mut out = String::new();
    out.push_str("cargo-allow worklist\n\n");
    out.push_str(&format!(
        "Inventory: source_tree/source_syntax via {}{}\n",
        context.inventory_source,
        worklist_inventory_files_suffix(context)
    ));
    if let Some(root) = context.source_tree_root {
        out.push_str(&format!("Source tree root: {root}\n"));
    }
    out.push_str(&worklist_filters_human(context.filters));
    out.push_str(&format!("Work items: {}\n", items.len()));
    out.push_str("Risk:\n");
    out.push_str(&format!("  high      {}\n", risk_count(items, "high")));
    out.push_str(&format!("  medium    {}\n", risk_count(items, "medium")));
    out.push_str(&format!("  low       {}\n", risk_count(items, "low")));
    out.push_str("Difficulty:\n");
    out.push_str(&format!(
        "  small     {}\n",
        difficulty_count(items, "small")
    ));
    out.push_str(&format!(
        "  medium    {}\n",
        difficulty_count(items, "medium")
    ));
    for item in items.iter().take(80) {
        out.push_str(&format!(
            "\n{} ({}, {}) {}\n",
            item.id, item.risk, item.difficulty, item.kind
        ));
        if let Some(path) = &item.path {
            out.push_str(&format!("  path: {path}\n"));
        }
        if let Some(package) = &item.source_package {
            out.push_str(&format!("  source package: {package}\n"));
        }
        if let Some(allow_id) = &item.allow_id {
            out.push_str(&format!("  allow: {allow_id}\n"));
        }
        if let Some(owner) = &item.owner {
            out.push_str(&format!("  owner: {owner}\n"));
        }
        if let Some(classification) = &item.classification {
            out.push_str(&format!("  classification: {classification}\n"));
        }
        if let Some(reason) = &item.reason {
            out.push_str(&format!("  reason: {reason}\n"));
        }
        if let Some(created) = &item.created {
            out.push_str(&format!("  created: {created}\n"));
        }
        if let Some(review_after) = &item.review_after {
            out.push_str(&format!("  review_after: {review_after}\n"));
        }
        if let Some(expires) = &item.expires {
            out.push_str(&format!("  expires: {expires}\n"));
        }
        if let Some(evidence_count) = item.evidence_count {
            out.push_str(&format!("  evidence: {evidence_count} reference(s)\n"));
        }
        if let Some(exception_kind) = &item.exception_kind {
            out.push_str(&format!("  exception: {exception_kind}"));
            if let Some(family) = &item.family {
                out.push_str(&format!(".{family}"));
            }
            out.push('\n');
        }
        out.push_str(&format!("  status: {}\n", item.status.as_str()));
        out.push_str(&format!("  message: {}\n", item.message));
        for action in item.suggested_actions.iter().take(2) {
            out.push_str(&format!("  action: {action}\n"));
        }
        for command in item.proof_commands.iter().take(3) {
            out.push_str(&format!("  proof: {command}\n"));
        }
    }
    if items.len() > 80 {
        out.push_str(&format!(
            "\n{} additional work items omitted from human output; use `cargo-allow worklist --format json` for the full queue.\n",
            items.len() - 80
        ));
    }
    out.push('\n');
    out.push_str(allow_report::CLAIM_BOUNDARY_TEXT);
    out.push('\n');
    out
}

fn worklist_inventory_json(context: WorklistContext<'_>, indent: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("{indent}  \"scope\": \"source_tree\",\n"));
    out.push_str(&format!("{indent}  \"scanner\": \"source_syntax\",\n"));
    out.push_str(&format!(
        "{indent}  \"source\": \"{}\"",
        json_escape(context.inventory_source)
    ));
    if let Some(root) = context.source_tree_root {
        out.push_str(&format!(",\n{indent}  \"root\": \"{}\"", json_escape(root)));
    }
    if let Some(files) = context.inventory_files {
        out.push_str(&format!(",\n{indent}  \"files_scanned\": {files}"));
    }
    out.push_str(&format!("\n{indent}}}"));
    out
}

fn worklist_inventory_files_suffix(context: WorklistContext<'_>) -> String {
    context
        .inventory_files
        .map(|files| format!("; files scanned: {files}"))
        .unwrap_or_default()
}

fn worklist_filters_json(filters: WorklistFilters<'_>, indent: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "{indent}  \"kind\": {},\n",
        option_json_string(filters.kind)
    ));
    out.push_str(&format!(
        "{indent}  \"family\": {},\n",
        option_json_string(filters.family)
    ));
    out.push_str(&format!(
        "{indent}  \"item_kind\": {},\n",
        option_json_string(filters.item_kind)
    ));
    out.push_str(&format!(
        "{indent}  \"status\": {},\n",
        option_json_string(filters.status)
    ));
    out.push_str(&format!(
        "{indent}  \"allow_id\": {},\n",
        option_json_string(filters.allow_id)
    ));
    out.push_str(&format!(
        "{indent}  \"path\": {},\n",
        option_json_string(filters.path)
    ));
    out.push_str(&format!(
        "{indent}  \"source_package\": {},\n",
        option_json_string(filters.source_package)
    ));
    out.push_str(&format!(
        "{indent}  \"owner\": {},\n",
        option_json_string(filters.owner)
    ));
    out.push_str(&format!(
        "{indent}  \"classification\": {},\n",
        option_json_string(filters.classification)
    ));
    out.push_str(&format!(
        "{indent}  \"baseline_debt\": {},\n",
        filters.baseline_debt
    ));
    out.push_str(&format!(
        "{indent}  \"broad_scope\": {},\n",
        filters.broad_scope
    ));
    out.push_str(&format!(
        "{indent}  \"risk\": {},\n",
        option_json_string(filters.risk)
    ));
    out.push_str(&format!(
        "{indent}  \"difficulty\": {},\n",
        option_json_string(filters.difficulty)
    ));
    out.push_str(&format!(
        "{indent}  \"missing_evidence\": {}\n",
        filters.missing_evidence
    ));
    out.push_str(&format!("{indent}}}"));
    out
}

fn worklist_filters_human(filters: WorklistFilters<'_>) -> String {
    let mut parts = Vec::new();
    if let Some(kind) = filters.kind {
        parts.push(format!("kind={kind}"));
    }
    if let Some(family) = filters.family {
        parts.push(format!("family={family}"));
    }
    if let Some(item_kind) = filters.item_kind {
        parts.push(format!("item_kind={item_kind}"));
    }
    if let Some(status) = filters.status {
        parts.push(format!("status={status}"));
    }
    if let Some(allow_id) = filters.allow_id {
        parts.push(format!("allow_id={allow_id}"));
    }
    if let Some(path) = filters.path {
        parts.push(format!("path={path}"));
    }
    if let Some(source_package) = filters.source_package {
        parts.push(format!("source_package={source_package}"));
    }
    if let Some(owner) = filters.owner {
        parts.push(format!("owner={owner}"));
    }
    if let Some(classification) = filters.classification {
        parts.push(format!("classification={classification}"));
    }
    if filters.baseline_debt {
        parts.push("baseline_debt=true".to_string());
    }
    if filters.broad_scope {
        parts.push("broad_scope=true".to_string());
    }
    if let Some(risk) = filters.risk {
        parts.push(format!("risk={risk}"));
    }
    if let Some(difficulty) = filters.difficulty {
        parts.push(format!("difficulty={difficulty}"));
    }
    if filters.missing_evidence {
        parts.push("missing_evidence=true".to_string());
    }
    if parts.is_empty() {
        "Filters: none\n".to_string()
    } else {
        format!("Filters: {}\n", parts.join(", "))
    }
}

fn risk_count(items: &[WorkItem], risk: &str) -> usize {
    items.iter().filter(|item| item.risk == risk).count()
}

fn difficulty_count(items: &[WorkItem], difficulty: &str) -> usize {
    items
        .iter()
        .filter(|item| item.difficulty == difficulty)
        .count()
}

fn option_json_string(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", json_escape(value)))
        .unwrap_or_else(|| "null".to_string())
}

fn option_usize_json(value: Option<usize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn option_u32_json(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn json_string_array<T: AsRef<str>>(values: &[T]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("\"{}\"", json_escape(value.as_ref())))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn cmd_migrate(args: &MigrateArgs) -> CargoAllowResult<()> {
    let cfg = match (&args.from, &args.repo_policy) {
        (Some(from), None) => allow_policy_legacy::load_legacy_or_canonical(from)?,
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
    validate_policy(&cfg)?;
    write_file_no_overwrite(&args.out, &render_policy(&cfg), args.force)?;
    eprintln!("{}", allow_policy_legacy::migration_notes());
    Ok(())
}

fn load_repo_policy_migration_config(
    explicit_root: Option<&Path>,
    repo_policy: &Path,
) -> CargoAllowResult<AllowConfig> {
    let root = repo_policy_source_tree_root(explicit_root, repo_policy)?;
    let repo_policy = root_relative_path(&root, repo_policy);
    let files = inventory_files(&root, &InventoryOptions::default())?;
    let findings = allow_files::scan_files(&files)
        .into_iter()
        .filter(|finding| finding.kind == FindingKind::NonRustFile)
        .collect::<Vec<_>>();
    allow_policy_legacy::load_legacy_policy_dir_with_non_rust_findings(&repo_policy, &findings)
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

fn cmd_prune(args: &PruneArgs) -> CargoAllowResult<()> {
    if !args.stale {
        return Err(CargoAllowError::new(
            "prune currently supports only --stale",
        ));
    }
    if args.dry_run && args.write {
        return Err(CargoAllowError::new(
            "pass either --dry-run or --write, not both",
        ));
    }
    let (root, cfg, findings, inventory_facts) = load_world(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        None,
        args.include_untracked,
    )?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);
    let candidates = prune_stale_candidates(&cfg, &outcomes);
    let written_path = if args.write && !candidates.is_empty() {
        let path = config_path(&root, args.config.as_deref()).ok_or_else(|| {
            CargoAllowError::new("no policy config found; run `cargo-allow init` or pass --config")
        })?;
        let pruned = config_without_prune_candidates(&cfg, &candidates);
        validate_policy(&pruned)?;
        write_file(&path, &render_policy(&pruned))?;
        Some(path)
    } else {
        None
    };
    let root_text = source_tree_root_text(&root);
    let context = PruneContext {
        inventory_source: inventory_facts.source.as_str(),
        source_tree_root: Some(&root_text),
        inventory_files: inventory_facts.files_scanned,
    };
    let text = match args.format {
        PruneFormat::Human => render_prune_stale_result(
            &candidates,
            args.dry_run,
            args.write,
            written_path.as_deref(),
        ),
        PruneFormat::Json => render_prune_stale_json(
            &candidates,
            args.dry_run,
            args.write,
            written_path.as_deref(),
            context,
        ),
    };
    if let Some(path) = &args.output {
        write_file(path, &text)?;
    } else {
        println!("{text}");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct PruneContext<'a> {
    inventory_source: &'a str,
    source_tree_root: Option<&'a str>,
    inventory_files: Option<usize>,
}

impl Default for PruneContext<'static> {
    fn default() -> Self {
        Self {
            inventory_source: "unknown",
            source_tree_root: None,
            inventory_files: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PruneCandidate {
    id: String,
    kind: FindingKind,
    family: Option<String>,
    owner: String,
    classification: String,
    scope: String,
    reason: String,
}

fn prune_stale_candidates(cfg: &AllowConfig, outcomes: &[MatchOutcome]) -> Vec<PruneCandidate> {
    outcomes
        .iter()
        .filter(|outcome| outcome.status == MatchStatus::Stale)
        .filter_map(|outcome| {
            let id = outcome.allow_id.as_deref()?;
            let entry = cfg.allow.iter().find(|entry| entry.id == id)?;
            Some(PruneCandidate {
                id: entry.id.clone(),
                kind: entry.kind,
                family: entry.family.clone(),
                owner: entry.owner.clone(),
                classification: entry.classification.clone(),
                scope: entry.path_or_glob(),
                reason: entry.reason.clone(),
            })
        })
        .collect()
}

fn config_without_prune_candidates(
    cfg: &AllowConfig,
    candidates: &[PruneCandidate],
) -> AllowConfig {
    let mut pruned = cfg.clone();
    pruned
        .allow
        .retain(|entry| !candidates.iter().any(|candidate| candidate.id == entry.id));
    pruned
}

fn render_prune_stale_result(
    candidates: &[PruneCandidate],
    explicit_dry_run: bool,
    write_requested: bool,
    written_path: Option<&Path>,
) -> String {
    let mut out = String::new();
    out.push_str("cargo-allow prune\n\n");
    if write_requested {
        out.push_str("mode: write\n");
    } else {
        out.push_str("mode: dry-run\n");
    }
    if explicit_dry_run {
        out.push_str("requested: --dry-run\n");
    }
    out.push_str(&format!("stale entries: {}\n\n", candidates.len()));
    if candidates.is_empty() {
        out.push_str("No stale allow entries found.\n");
        return out;
    }
    out.push_str("| Allow ID | Kind | Family | Owner | Classification | Scope | Reason |\n");
    out.push_str("|---|---|---|---|---|---|---|\n");
    for candidate in candidates {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | {} |\n",
            markdown_cell(&candidate.id),
            candidate.kind,
            markdown_cell(candidate.family.as_deref().unwrap_or("-")),
            markdown_cell(&candidate.owner),
            markdown_cell(&candidate.classification),
            markdown_cell(&candidate.scope),
            markdown_cell(&candidate.reason)
        ));
    }
    if let Some(path) = written_path {
        out.push_str(&format!(
            "\nRemoved stale entries from `{}`.\n",
            markdown_cell(&path.display().to_string())
        ));
    } else {
        out.push_str(
            "\nNo files were changed. Remove these entries only after confirming the exception is gone.\n",
        );
    }
    out
}

fn render_prune_stale_json(
    candidates: &[PruneCandidate],
    explicit_dry_run: bool,
    write_requested: bool,
    written_path: Option<&Path>,
    context: PruneContext<'_>,
) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": {},\n",
        allow_report::PRUNE_SCHEMA_VERSION
    ));
    out.push_str(&format!(
        "  \"schema_id\": \"{}\",\n",
        allow_report::PRUNE_SCHEMA_ID
    ));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str("  \"command\": \"prune\",\n");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        json_string_array(allow_report::CLAIM_BOUNDARY)
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        json_string_array(allow_report::SCANNER_LIMITATIONS)
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&prune_inventory_json(context, "  "));
    out.push_str(",\n");
    out.push_str("  \"mode\": {\n");
    out.push_str(&format!("    \"dry_run\": {},\n", !write_requested));
    out.push_str(&format!("    \"write_requested\": {},\n", write_requested));
    out.push_str(&format!(
        "    \"explicit_dry_run\": {},\n",
        explicit_dry_run
    ));
    let written = written_path.map(|path| path.display().to_string());
    out.push_str(&format!(
        "    \"written_path\": {}\n",
        option_json_string(written.as_deref())
    ));
    out.push_str("  },\n");
    out.push_str(&format!(
        "  \"summary\": {{\n    \"stale_entries\": {}\n  }},\n",
        candidates.len()
    ));
    out.push_str("  \"stale_entries\": [\n");
    for (index, candidate) in candidates.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&prune_candidate_json(candidate, "  "));
    }
    out.push_str("\n  ]\n");
    out.push_str("}\n");
    out
}

fn prune_inventory_json(context: PruneContext<'_>, indent: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("{indent}  \"scope\": \"source_tree\",\n"));
    out.push_str(&format!("{indent}  \"scanner\": \"source_syntax\",\n"));
    out.push_str(&format!(
        "{indent}  \"source\": \"{}\"",
        json_escape(context.inventory_source)
    ));
    if let Some(root) = context.source_tree_root {
        out.push_str(&format!(",\n{indent}  \"root\": \"{}\"", json_escape(root)));
    }
    if let Some(files) = context.inventory_files {
        out.push_str(&format!(",\n{indent}  \"files_scanned\": {files}"));
    }
    out.push_str(&format!("\n{indent}}}"));
    out
}

fn prune_candidate_json(candidate: &PruneCandidate, indent: &str) -> String {
    format!(
        "{indent}  {{\n{indent}    \"id\": \"{}\",\n{indent}    \"kind\": \"{}\",\n{indent}    \"family\": {},\n{indent}    \"owner\": \"{}\",\n{indent}    \"classification\": \"{}\",\n{indent}    \"scope\": \"{}\",\n{indent}    \"reason\": \"{}\"\n{indent}  }}",
        json_escape(&candidate.id),
        candidate.kind,
        option_json_string(candidate.family.as_deref()),
        json_escape(&candidate.owner),
        json_escape(&candidate.classification),
        json_escape(&candidate.scope),
        json_escape(&candidate.reason)
    )
}

fn cmd_doctor(args: &ConfigArgs) -> CargoAllowResult<()> {
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    let root = resolve_source_tree_root(args.root.root.as_deref(), &cwd)?;
    let root_discovery = root_discovery_kind(args.root.root.as_deref(), &root);
    let config = config_path(&root, args.config.as_deref());
    let opts = InventoryOptions::default();
    let inventory = inventory(&root, &opts)?;
    let root_text = source_tree_root_text(&root);
    let config_text = config.as_ref().map(|path| source_tree_root_text(path));
    let facts = DoctorFacts {
        source_tree_root: &root_text,
        root_discovery,
        config_path: config_text.as_deref(),
        inventory_source: inventory.source.as_str(),
        files_scanned: inventory.files.len(),
    };
    let text = match args.format {
        DoctorFormat::Human => render_doctor_human(facts),
        DoctorFormat::Json => render_doctor_json(facts),
    };
    if let Some(path) = &args.output {
        write_file(path, &text)?;
    } else {
        println!("{text}");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct DoctorFacts<'a> {
    source_tree_root: &'a str,
    root_discovery: &'a str,
    config_path: Option<&'a str>,
    inventory_source: &'a str,
    files_scanned: usize,
}

fn root_discovery_kind(explicit_root: Option<&Path>, root: &Path) -> &'static str {
    if explicit_root.is_some() {
        "explicit_root"
    } else if root.join(".git").exists() {
        "nearest_git_root"
    } else {
        "current_directory_fallback"
    }
}

fn render_doctor_human(facts: DoctorFacts<'_>) -> String {
    let mut out = String::new();
    out.push_str(&format!("source tree root: {}\n", facts.source_tree_root));
    out.push_str(&format!("root discovery: {}\n", facts.root_discovery));
    match facts.config_path {
        Some(path) => out.push_str(&format!("config: {path}\n")),
        None => out.push_str("config: not found; run `cargo-allow init`\n"),
    }
    out.push_str(&format!(
        "inventory: source_tree/source_syntax via {}; files scanned: {}\n",
        facts.inventory_source, facts.files_scanned
    ));
    out.push_str(allow_report::CLAIM_BOUNDARY_TEXT);
    out
}

fn render_doctor_json(facts: DoctorFacts<'_>) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": {},\n",
        allow_report::DOCTOR_SCHEMA_VERSION
    ));
    out.push_str(&format!(
        "  \"schema_id\": \"{}\",\n",
        allow_report::DOCTOR_SCHEMA_ID
    ));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str("  \"command\": \"doctor\",\n");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        json_string_array(allow_report::CLAIM_BOUNDARY)
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        json_string_array(allow_report::SCANNER_LIMITATIONS)
    ));
    out.push_str("  \"root\": {\n");
    out.push_str(&format!(
        "    \"path\": \"{}\",\n",
        json_escape(facts.source_tree_root)
    ));
    out.push_str(&format!(
        "    \"discovery\": \"{}\"\n",
        json_escape(facts.root_discovery)
    ));
    out.push_str("  },\n");
    out.push_str("  \"config\": {\n");
    out.push_str(&format!(
        "    \"found\": {},\n",
        facts.config_path.is_some()
    ));
    out.push_str(&format!(
        "    \"path\": {}\n",
        option_json_string(facts.config_path)
    ));
    out.push_str("  },\n");
    out.push_str("  \"inventory\": {\n");
    out.push_str("    \"scope\": \"source_tree\",\n");
    out.push_str("    \"scanner\": \"source_syntax\",\n");
    out.push_str(&format!(
        "    \"source\": \"{}\",\n",
        json_escape(facts.inventory_source)
    ));
    out.push_str(&format!("    \"files_scanned\": {}\n", facts.files_scanned));
    out.push_str("  }\n");
    out.push_str("}\n");
    out
}

fn load_world(
    explicit_root: Option<&Path>,
    config: Option<&Path>,
    require_config: bool,
    kind_filter: Option<&str>,
    include_untracked: bool,
) -> CargoAllowResult<(PathBuf, AllowConfig, Vec<Finding>, InventoryFacts)> {
    load_world_with_evidence_validation(
        explicit_root,
        config,
        require_config,
        kind_filter,
        include_untracked,
        true,
    )
}

fn load_world_with_evidence_validation(
    explicit_root: Option<&Path>,
    config: Option<&Path>,
    require_config: bool,
    kind_filter: Option<&str>,
    include_untracked: bool,
    validate_local_evidence: bool,
) -> CargoAllowResult<(PathBuf, AllowConfig, Vec<Finding>, InventoryFacts)> {
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    let root = resolve_source_tree_root(explicit_root, cwd)?;
    let cfg = if require_config {
        load_config_required(&root, config, validate_local_evidence)?
    } else {
        load_config_optional(&root, config, validate_local_evidence)?
            .unwrap_or_else(AllowConfig::empty)
    };
    let opts = InventoryOptions {
        ignored: cfg.workspace.ignored.clone(),
        generated: cfg.workspace.generated.clone(),
        include_untracked,
    };
    let inventory = inventory(&root, &opts)?;
    let inventory_facts = InventoryFacts::scanned(inventory.source, inventory.files.len());
    let files = inventory.files;
    let mut findings = Vec::new();
    findings.extend(allow_rust::scan_rust_files(&root, &files)?);
    findings.extend(allow_files::scan_files_with_options(
        &files,
        &allow_files::FileScanOptions {
            generated: opts.generated.clone(),
        },
    ));
    let companion_findings = canonical_companion_findings(&root, &cfg)?;
    extend_unique_findings(&mut findings, companion_findings);
    if let Some(kind) = kind_filter {
        let parsed = parse_kind_filter(kind)?;
        findings.retain(|f| parsed.matches_finding(f));
    }
    Ok((root, cfg, findings, inventory_facts))
}

fn canonical_companion_findings(root: &Path, cfg: &AllowConfig) -> CargoAllowResult<Vec<Finding>> {
    let mut findings = Vec::new();
    if has_allow_family(cfg, FindingKind::GeneratedCode, "generated_code") {
        findings.extend(allow_policy_legacy::generated_findings_from_gitattributes(
            root,
        )?);
    }
    if has_allow_family(cfg, FindingKind::PolicyException, "executable_file") {
        findings.extend(allow_policy_legacy::executable_findings_from_git(root)?);
    }
    if has_policy_family(cfg, &["github_workflow", "workflow_external_action"]) {
        findings.extend(allow_policy_legacy::workflow_findings_from_files(root)?);
    }
    if has_allow_family(cfg, FindingKind::PolicyException, "dependency_surface") {
        findings.extend(allow_policy_legacy::dependency_surface_findings_from_git(
            root, cfg,
        )?);
    }
    if has_allow_family(cfg, FindingKind::PolicyException, "process_spawn") {
        findings.extend(allow_policy_legacy::process_findings_from_config(cfg));
    }
    if has_allow_family(cfg, FindingKind::PolicyException, "network_destination") {
        findings.extend(allow_policy_legacy::network_findings_from_config(cfg));
    }
    Ok(findings)
}

fn has_policy_family(cfg: &AllowConfig, families: &[&str]) -> bool {
    cfg.allow.iter().any(|entry| {
        entry.kind == FindingKind::PolicyException
            && entry
                .family
                .as_deref()
                .is_some_and(|family| families.contains(&family))
    })
}

fn has_allow_family(cfg: &AllowConfig, kind: FindingKind, family: &str) -> bool {
    cfg.allow
        .iter()
        .any(|entry| entry.kind == kind && entry.family.as_deref() == Some(family))
}

fn extend_unique_findings(findings: &mut Vec<Finding>, additional: Vec<Finding>) {
    for finding in additional {
        if !findings
            .iter()
            .any(|existing| same_finding_identity(existing, &finding))
        {
            findings.push(finding);
        }
    }
}

fn same_finding_identity(left: &Finding, right: &Finding) -> bool {
    allow_core::finding_identity_key(left) == allow_core::finding_identity_key(right)
}

fn load_compat_world(
    explicit_root: Option<&Path>,
    config: Option<&Path>,
    kind_filter: Option<&str>,
    include_untracked: bool,
) -> CargoAllowResult<(PathBuf, AllowConfig, Vec<Finding>, InventoryFacts)> {
    let compat_kind = kind_filter.unwrap_or("non-rust");
    let parsed_filter = kind_filter
        .map(parse_kind_filter)
        .transpose()?
        .unwrap_or(KindFilter {
            kind: FindingKind::NonRustFile,
            family: FamilyFilter::Any,
        });
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    let root = resolve_source_tree_root(explicit_root, cwd)?;
    if is_no_panic_allowlist_compat_kind(compat_kind) {
        let policy_path = config
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("policy/no-panic-allowlist.toml"));
        let cfg = allow_policy_legacy::load_no_panic_allowlist_compat_config(policy_path)?;
        let opts = InventoryOptions {
            ignored: cfg.workspace.ignored.clone(),
            generated: cfg.workspace.generated.clone(),
            include_untracked,
        };
        let inventory = inventory(&root, &opts)?;
        let inventory_facts = InventoryFacts::scanned(inventory.source, inventory.files.len());
        let mut findings = allow_rust::scan_rust_files(&root, &inventory.files)?;
        findings.retain(|finding| finding.kind == FindingKind::Panic);
        return Ok((root, cfg, findings, inventory_facts));
    }
    if is_panic_compat_kind(compat_kind) {
        let policy_path = config
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("policy/no-panic-baseline.toml"));
        let cfg = allow_policy_legacy::load_no_panic_baseline_compat_config(policy_path)?;
        let opts = InventoryOptions {
            ignored: cfg.workspace.ignored.clone(),
            generated: cfg.workspace.generated.clone(),
            include_untracked,
        };
        let inventory = inventory(&root, &opts)?;
        let inventory_facts = InventoryFacts::scanned(inventory.source, inventory.files.len());
        let mut findings = allow_rust::scan_rust_files(&root, &inventory.files)?;
        findings.retain(|finding| finding.kind == FindingKind::Panic);
        return Ok((root, cfg, findings, inventory_facts));
    }
    if is_clippy_compat_kind(compat_kind) {
        let policy_path = config
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("policy/clippy-exceptions.toml"));
        let cfg = allow_policy_legacy::load_clippy_exceptions_compat_config(policy_path)?;
        let opts = InventoryOptions {
            ignored: cfg.workspace.ignored.clone(),
            generated: cfg.workspace.generated.clone(),
            include_untracked,
        };
        let inventory = inventory(&root, &opts)?;
        let inventory_facts = InventoryFacts::scanned(inventory.source, inventory.files.len());
        let mut findings = allow_rust::scan_rust_files(&root, &inventory.files)?;
        findings.retain(|finding| finding.kind == FindingKind::LintException);
        return Ok((root, cfg, findings, inventory_facts));
    }
    if is_unsafe_compat_kind(compat_kind) {
        let policy_path = config
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("policy/unsafe-allowlist.toml"));
        let cfg = allow_policy_legacy::load_unsafe_allowlist_compat_config(policy_path)?;
        let opts = InventoryOptions {
            ignored: cfg.workspace.ignored.clone(),
            generated: cfg.workspace.generated.clone(),
            include_untracked,
        };
        let inventory = inventory(&root, &opts)?;
        let inventory_facts = InventoryFacts::scanned(inventory.source, inventory.files.len());
        let mut findings = allow_rust::scan_rust_files(&root, &inventory.files)?;
        findings.retain(|finding| finding.kind == FindingKind::Unsafe);
        return Ok((root, cfg, findings, inventory_facts));
    }
    if is_executable_compat_kind(compat_kind) {
        let policy_path = config
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("policy/executable-allowlist.toml"));
        let cfg = allow_policy_legacy::load_executable_compat_config(policy_path)?;
        let findings = allow_policy_legacy::executable_findings_from_git(&root)?;
        return Ok((
            root,
            cfg,
            findings,
            InventoryFacts::source_only(InventorySource::FilesystemFallback),
        ));
    }
    if is_workflow_compat_kind(compat_kind) {
        let policy_path = config
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("policy/workflow-allowlist.toml"));
        let cfg = allow_policy_legacy::load_workflow_compat_config(policy_path)?;
        let findings = allow_policy_legacy::workflow_findings_from_files(&root)?;
        return Ok((
            root,
            cfg,
            findings,
            InventoryFacts::source_only(InventorySource::GitTracked),
        ));
    }
    if is_dependency_surface_compat_kind(compat_kind) {
        let policy_path = config
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("policy/dependency-surface-allowlist.toml"));
        let cfg = allow_policy_legacy::load_dependency_surface_compat_config(policy_path)?;
        let findings = allow_policy_legacy::dependency_surface_findings_from_git(&root, &cfg)?;
        return Ok((
            root,
            cfg,
            findings,
            InventoryFacts::source_only(InventorySource::GitTracked),
        ));
    }
    if is_process_compat_kind(compat_kind) {
        let policy_path = config
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("policy/process-allowlist.toml"));
        let cfg = allow_policy_legacy::load_process_compat_config(policy_path)?;
        let findings = allow_policy_legacy::process_findings_from_config(&cfg);
        return Ok((
            root,
            cfg,
            findings,
            InventoryFacts::source_only(InventorySource::FilesystemFallback),
        ));
    }
    if is_network_compat_kind(compat_kind) {
        let policy_path = config
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("policy/network-allowlist.toml"));
        let cfg = allow_policy_legacy::load_network_compat_config(policy_path)?;
        let findings = allow_policy_legacy::network_findings_from_config(&cfg);
        return Ok((
            root,
            cfg,
            findings,
            InventoryFacts::source_only(InventorySource::FilesystemFallback),
        ));
    }
    if parsed_filter.kind == FindingKind::GeneratedCode {
        let policy_path = config
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("policy/generated-allowlist.toml"));
        let cfg = allow_policy_legacy::load_generated_compat_config(policy_path)?;
        let findings = allow_policy_legacy::generated_findings_from_gitattributes(&root)?;
        return Ok((
            root,
            cfg,
            findings,
            InventoryFacts::source_only(InventorySource::FilesystemFallback),
        ));
    }
    if parsed_filter.kind != FindingKind::NonRustFile {
        return Err(CargoAllowError::new(
            "--compat currently supports only --kind non-rust, --kind generated, --kind panic, --kind no-panic-allowlist, --kind lint-exception, --kind unsafe, --kind executable, --kind workflow, --kind dependency-surface, --kind process, or --kind network",
        ));
    }
    let opts = InventoryOptions {
        include_untracked,
        ..InventoryOptions::default()
    };
    let inventory = inventory(&root, &opts)?;
    let inventory_facts = InventoryFacts::scanned(inventory.source, inventory.files.len());
    let findings = allow_files::scan_files(&inventory.files)
        .into_iter()
        .filter(|finding| finding.kind == FindingKind::NonRustFile)
        .collect::<Vec<_>>();
    let policy_path = config
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("policy/non-rust-allowlist.toml"));
    let cfg = allow_policy_legacy::load_non_rust_compat_config(policy_path, &findings)?;
    Ok((root, cfg, findings, inventory_facts))
}

#[derive(Debug, Clone, Copy)]
struct KindFilter {
    kind: FindingKind,
    family: FamilyFilter,
}

#[derive(Debug, Clone, Copy)]
enum FamilyFilter {
    Any,
    Exact(&'static str),
    Workflow,
}

impl KindFilter {
    fn matches_finding(self, finding: &Finding) -> bool {
        finding.kind == self.kind && self.family.matches(finding.family.as_deref())
    }

    fn matches_entry(self, entry: &AllowEntry) -> bool {
        entry.kind == self.kind && self.family.matches(entry.family.as_deref())
    }
}

impl FamilyFilter {
    fn matches(self, family: Option<&str>) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(expected) => family == Some(expected),
            Self::Workflow => {
                matches!(family, Some("github_workflow" | "workflow_external_action"))
            }
        }
    }
}

fn parse_kind_filter(kind: &str) -> CargoAllowResult<KindFilter> {
    if is_panic_compat_kind(kind) {
        return Ok(KindFilter {
            kind: FindingKind::Panic,
            family: FamilyFilter::Any,
        });
    }
    if is_no_panic_allowlist_compat_kind(kind) {
        return Ok(KindFilter {
            kind: FindingKind::Panic,
            family: FamilyFilter::Any,
        });
    }
    if is_clippy_compat_kind(kind) {
        return Ok(KindFilter {
            kind: FindingKind::LintException,
            family: FamilyFilter::Any,
        });
    }
    if is_unsafe_compat_kind(kind) {
        return Ok(KindFilter {
            kind: FindingKind::Unsafe,
            family: FamilyFilter::Any,
        });
    }
    if is_executable_compat_kind(kind) {
        return Ok(KindFilter {
            kind: FindingKind::PolicyException,
            family: FamilyFilter::Exact("executable_file"),
        });
    }
    if is_workflow_compat_kind(kind) {
        return Ok(KindFilter {
            kind: FindingKind::PolicyException,
            family: FamilyFilter::Workflow,
        });
    }
    if is_dependency_surface_compat_kind(kind) {
        return Ok(KindFilter {
            kind: FindingKind::PolicyException,
            family: FamilyFilter::Exact("dependency_surface"),
        });
    }
    if is_process_compat_kind(kind) {
        return Ok(KindFilter {
            kind: FindingKind::PolicyException,
            family: FamilyFilter::Exact("process_spawn"),
        });
    }
    if is_network_compat_kind(kind) {
        return Ok(KindFilter {
            kind: FindingKind::PolicyException,
            family: FamilyFilter::Exact("network_destination"),
        });
    }
    Ok(KindFilter {
        kind: FindingKind::from_str(kind)?,
        family: FamilyFilter::Any,
    })
}

fn is_panic_compat_kind(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "panic"
            | "panic-family"
            | "panic_family"
            | "no-panic"
            | "no_panic"
            | "no-panic-baseline"
            | "no_panic_baseline"
    )
}

fn is_no_panic_allowlist_compat_kind(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "no-panic-allowlist" | "no_panic_allowlist" | "panic-allowlist" | "panic_allowlist"
    )
}

fn is_clippy_compat_kind(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "clippy"
            | "clippy-exception"
            | "clippy-exceptions"
            | "clippy_exception"
            | "clippy_exceptions"
            | "lint"
            | "lint-exception"
            | "lint_exception"
            | "lint-suppression"
            | "lint_suppression"
    )
}

fn is_unsafe_compat_kind(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "unsafe" | "unsafe-allowlist" | "unsafe_allowlist" | "unsafe-policy" | "unsafe_policy"
    )
}

fn is_executable_compat_kind(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "executable" | "executable_file" | "executable-file" | "executable-bit" | "exec"
    )
}

fn is_workflow_compat_kind(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "workflow" | "workflows" | "github_workflow" | "github-workflow" | "workflow-action"
    )
}

fn is_dependency_surface_compat_kind(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "dependency"
            | "dependencies"
            | "dependency_surface"
            | "dependency-surface"
            | "dependency-surfaces"
            | "dep-surface"
            | "dep"
    )
}

fn is_process_compat_kind(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "process" | "processes" | "process-policy" | "process_spawn" | "process-spawn" | "proc"
    )
}

fn is_network_compat_kind(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "network" | "net" | "network-policy" | "network_destination" | "network-destination"
    )
}

fn report_config(cfg: &AllowConfig, kind_filter: Option<&str>) -> CargoAllowResult<AllowConfig> {
    let Some(kind) = kind_filter else {
        return Ok(cfg.clone());
    };
    let parsed = parse_kind_filter(kind)?;
    let mut filtered = cfg.clone();
    filtered.allow.retain(|entry| parsed.matches_entry(entry));
    Ok(filtered)
}

fn policy_baseline_debt_entries(cfg: &AllowConfig) -> usize {
    cfg.allow
        .iter()
        .filter(|entry| entry.classification == "baseline_debt")
        .count()
}

fn load_config_required(
    root: &Path,
    config: Option<&Path>,
    validate_local_evidence: bool,
) -> CargoAllowResult<AllowConfig> {
    let path = config_path(root, config).ok_or_else(|| {
        CargoAllowError::new("no policy config found; run `cargo-allow init` or pass --config")
    })?;
    load_policy_for_root(root, path, validate_local_evidence)
}

fn load_config_optional(
    root: &Path,
    config: Option<&Path>,
    validate_local_evidence: bool,
) -> CargoAllowResult<Option<AllowConfig>> {
    match config_path(root, config) {
        Some(path) => Ok(Some(load_policy_for_root(
            root,
            path,
            validate_local_evidence,
        )?)),
        None => Ok(None),
    }
}

fn load_policy_for_root(
    root: &Path,
    path: PathBuf,
    validate_local_evidence: bool,
) -> CargoAllowResult<AllowConfig> {
    let cfg = load_policy(path)?;
    if validate_local_evidence {
        validate_local_evidence_references(root, &cfg)?;
    }
    Ok(cfg)
}

fn config_path(root: &Path, config: Option<&Path>) -> Option<PathBuf> {
    config
        .map(|path| root_relative_path(root, path))
        .or_else(|| find_config(root))
}

fn root_relative_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn git_relative_config_path(root: &Path, config: Option<&Path>) -> CargoAllowResult<PathBuf> {
    let path = config_path(root, config).ok_or_else(|| {
        CargoAllowError::new("no policy config found; run `cargo-allow init` or pass --config")
    })?;
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

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('`', "\\`")
}

fn source_tree_root_text(root: &Path) -> String {
    let text = root.to_string_lossy().replace('\\', "/");
    if let Some(stripped) = text.strip_prefix("//?/UNC/") {
        return format!("//{stripped}");
    }
    if let Some(stripped) = text.strip_prefix("//?/") {
        return stripped.to_string();
    }
    if let Some(stripped) = text.strip_prefix("/?/") {
        return stripped.to_string();
    }
    normalize_path(root)
}

struct ReportRenderArgs<'a> {
    command: &'a str,
    format: OutputFormat,
    baseline_debt_entries: usize,
    findings: &'a [Finding],
    outcomes: &'a [allow_core::MatchOutcome],
    failed: bool,
    output: Option<&'a Path>,
    root: &'a Path,
    inventory_facts: InventoryFacts,
}

fn print_report(args: ReportRenderArgs<'_>) -> CargoAllowResult<()> {
    let root_text = source_tree_root_text(args.root);
    let context = allow_report::ReportContext {
        inventory_source: args.inventory_facts.source.as_str(),
        source_tree_root: Some(&root_text),
        inventory_files: args.inventory_facts.files_scanned,
        baseline_debt_entries: Some(args.baseline_debt_entries),
    };
    let text = match args.format {
        OutputFormat::Human => allow_report::render_human_with_context(
            args.command,
            args.findings,
            args.outcomes,
            args.failed,
            context,
        ),
        OutputFormat::Json => allow_report::render_json_with_context(
            args.command,
            args.findings,
            args.outcomes,
            args.failed,
            context,
        ),
        OutputFormat::Html => allow_report::render_html_with_context(
            args.command,
            args.findings,
            args.outcomes,
            args.failed,
            context,
        ),
        OutputFormat::Sarif => allow_report::render_sarif_with_context(
            args.command,
            args.findings,
            args.outcomes,
            args.failed,
            context,
        ),
        OutputFormat::Markdown => allow_report::render_markdown_with_context(
            args.command,
            args.findings,
            args.outcomes,
            args.failed,
            context,
        ),
    };
    if let Some(path) = args.output {
        write_file(path, &text)?;
    } else {
        println!("{text}");
    }
    Ok(())
}

fn entry_from_finding(finding: &Finding, index: usize, expires: &str) -> AllowEntry {
    AllowEntry {
        id: format!("allow-{index:04}"),
        kind: finding.kind,
        family: finding.family.clone(),
        path: Some(finding.path.clone()),
        glob: None,
        owner: "unowned".to_string(),
        classification: "baseline_debt".to_string(),
        reason: "Generated by cargo-allow propose; requires human review.".to_string(),
        evidence: if finding.kind == FindingKind::Unsafe {
            vec!["TODO: add unsafe-review or boundary-test evidence".to_string()]
        } else {
            Vec::new()
        },
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: None,
            expires: Some(expires.to_string()),
        },
        selector: selector_from_finding(finding),
        last_seen: finding.span.as_ref().map(|s| allow_core::LastSeen {
            line: s.line,
            column: s.column,
        }),
    }
}

fn selector_from_finding(finding: &Finding) -> Selector {
    Selector {
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

fn write_file_no_overwrite(
    path: impl AsRef<Path>,
    contents: &str,
    force: bool,
) -> CargoAllowResult<()> {
    let path = path.as_ref();
    if path.exists() && !force {
        return Err(CargoAllowError::new(format!(
            "{} already exists; use --force to overwrite",
            path.display()
        )));
    }
    write_file(path, contents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{Span, StructuralIdentity};
    use serde_json::Value;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn normalized_args_accepts_cargo_subcommand_prefix() {
        let normalized = normalized_args(argv(vec!["cargo-allow", "allow", "audit"]));
        let expected = argv(vec!["cargo-allow", "audit"]);
        assert_eq!(normalized, expected);
    }

    #[test]
    fn source_tree_root_text_strips_windows_verbatim_prefix() {
        assert_eq!(
            source_tree_root_text(Path::new(r"\\?\H:\Code\Rust\cargo-allow")),
            "H:/Code/Rust/cargo-allow"
        );
        assert_eq!(
            source_tree_root_text(Path::new(r"\\?\UNC\server\share\repo")),
            "//server/share/repo"
        );
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
    fn clap_parses_doctor_json_output() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "doctor",
            "--root",
            ".",
            "--config",
            "policy/custom.toml",
            "--format",
            "json",
            "--output",
            "target/doctor.json",
        ]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Doctor(ConfigArgs {
                root: RootArgs { root: Some(root) },
                config: Some(config),
                format: DoctorFormat::Json,
                output: Some(output),
            })) if root == Path::new(".")
                && config == Path::new("policy/custom.toml")
                && output == Path::new("target/doctor.json")
        ));
    }

    #[test]
    fn clap_parses_lint_exception_compat_check() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "check",
            "--compat",
            "--kind",
            "lint-exception",
        ]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Check(CheckArgs {
                compat: true,
                kind: Some(kind),
                ..
            })) if kind == "lint-exception"
        ));
    }

    #[test]
    fn clap_parses_no_panic_allowlist_compat_check() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "check",
            "--compat",
            "--kind",
            "no-panic-allowlist",
        ]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Check(CheckArgs {
                compat: true,
                kind: Some(kind),
                ..
            })) if kind == "no-panic-allowlist"
        ));
    }

    #[test]
    fn clap_parses_unsafe_compat_check() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "check",
            "--compat",
            "--kind",
            "unsafe",
        ]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Check(CheckArgs {
                compat: true,
                kind: Some(kind),
                ..
            })) if kind == "unsafe"
        ));
    }

    #[test]
    fn clap_requires_diff_base() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "diff"]));

        assert!(parsed.is_err());
    }

    #[test]
    fn clap_parses_list_filters() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "list",
            "--kind",
            "unsafe",
            "--family",
            "unsafe_fn",
            "--owner",
            "runtime",
            "--classification",
            "baseline_debt",
            "--path",
            "crates/allow-core",
            "--source-package",
            "allow-core",
            "--status",
            "baseline_debt",
            "--expired",
            "--review-due",
            "--stale",
            "--baseline-debt",
            "--broad-scope",
            "--missing-evidence",
            "--format",
            "json",
            "--output",
            "target/list.json",
            "--include-untracked",
        ]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::List(ListArgs {
                kind: Some(kind),
                family: Some(family),
                owner: Some(owner),
                classification: Some(classification),
                path: Some(path),
                source_package: Some(source_package),
                status: Some(status),
                expired: true,
                review_due: true,
                stale: true,
                baseline_debt: true,
                broad_scope: true,
                missing_evidence: true,
                format: ListFormat::Json,
                output: Some(output),
                include_untracked: true,
                ..
            })) if kind == "unsafe"
                && family == "unsafe_fn"
                && owner == "runtime"
                && classification == "baseline_debt"
                && path == "crates/allow-core"
                && source_package == "allow-core"
                && status == "baseline_debt"
                && output == Path::new("target/list.json")
        ));
    }

    #[test]
    fn clap_parses_propose_force() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "propose",
            "--write",
            "target/proposed.toml",
            "--force",
            "--summary-format",
            "json",
            "--summary-output",
            "target/propose-summary.json",
        ]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Propose(ProposeArgs {
                write: Some(path),
                force: true,
                summary_format: ProposeSummaryFormat::Json,
                summary_output: Some(summary_output),
                ..
            })) if path == Path::new("target/proposed.toml")
                && summary_output == Path::new("target/propose-summary.json")
        ));
    }

    #[test]
    fn propose_summary_reports_generated_baseline_boundary() {
        let text = render_propose_summary(
            12,
            3,
            "2026-08-01",
            Some(Path::new("policy/allow.proposed.toml")),
        );

        assert!(text.contains("findings scanned: 12"));
        assert!(text.contains("baseline_debt entries proposed: 3"));
        assert!(text.contains("owner: unowned"));
        assert!(text.contains("classification: baseline_debt"));
        assert!(text.contains("output: policy/allow.proposed.toml"));
        assert!(text.contains("generated debt still requires human review"));
    }

    #[test]
    fn render_propose_summary_json_records_generated_baseline_boundary() {
        let json = render_propose_summary_json(
            12,
            3,
            "2026-08-01",
            Some(Path::new("policy/allow.proposed.toml")),
            true,
            ProposeContext {
                inventory_source: "git_tracked",
                source_tree_root: Some("H:/Code/Rust/cargo-allow"),
                inventory_files: Some(51),
                kind_filter: Some("panic"),
            },
        );

        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains(&format!(
            "\"schema_id\": \"{}\"",
            allow_report::PROPOSE_SCHEMA_ID
        )));
        assert!(json.contains("\"command\": \"propose\""));
        assert!(json.contains("\"claim_boundary\""));
        assert!(json.contains("\"scanner_limitations\""));
        assert!(json.contains("\"source\": \"git_tracked\""));
        assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
        assert!(json.contains("\"files_scanned\": 51"));
        assert!(json.contains("\"kind\": \"panic\""));
        assert!(json.contains("\"policy_output\": \"policy/allow.proposed.toml\""));
        assert!(json.contains("\"force\": true"));
        assert!(json.contains("\"findings_scanned\": 12"));
        assert!(json.contains("\"baseline_debt_entries_proposed\": 3"));
        assert!(json.contains("\"owner\": \"unowned\""));
        assert!(json.contains("\"classification\": \"baseline_debt\""));
        assert!(json.contains("\"repository_code_not_executed\""));
    }

    #[test]
    fn render_doctor_json_records_setup_context() {
        let json = render_doctor_json(DoctorFacts {
            source_tree_root: "H:/Code/Rust/cargo-allow",
            root_discovery: "nearest_git_root",
            config_path: Some("H:/Code/Rust/cargo-allow/policy/allow.toml"),
            inventory_source: "git_tracked",
            files_scanned: 50,
        });

        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains(&format!(
            "\"schema_id\": \"{}\"",
            allow_report::DOCTOR_SCHEMA_ID
        )));
        assert!(json.contains("\"command\": \"doctor\""));
        assert!(json.contains("\"claim_boundary\""));
        assert!(json.contains("\"scanner_limitations\""));
        assert!(json.contains("\"path\": \"H:/Code/Rust/cargo-allow\""));
        assert!(json.contains("\"discovery\": \"nearest_git_root\""));
        assert!(json.contains("\"found\": true"));
        assert!(json.contains("policy/allow.toml"));
        assert!(json.contains("\"source\": \"git_tracked\""));
        assert!(json.contains("\"files_scanned\": 50"));
        assert!(json.contains("\"repository_code_not_executed\""));
    }

    #[test]
    fn json_artifact_renderers_emit_parseable_v1_contracts() {
        let report_json = allow_report::render_json_with_context(
            "audit",
            &[],
            &[],
            false,
            allow_report::ReportContext {
                inventory_source: "filesystem_fallback",
                source_tree_root: Some("fixtures/source-snapshot"),
                inventory_files: Some(7),
                ..allow_report::ReportContext::default()
            },
        );
        let report = parse_json_artifact(
            "report",
            &report_json,
            allow_report::REPORT_SCHEMA_ID,
            "audit",
        );
        assert_inventory_contract(
            "report",
            &report,
            "filesystem_fallback",
            Some("fixtures/source-snapshot"),
            Some(7),
        );

        let receipt_json = allow_report::render_receipt_with_context(
            "check",
            &[],
            false,
            allow_report::ReportContext {
                inventory_source: "git_tracked",
                source_tree_root: Some("H:/Code/Rust/cargo-allow"),
                inventory_files: Some(42),
                ..allow_report::ReportContext::default()
            },
        );
        let receipt = parse_json_artifact(
            "receipt",
            &receipt_json,
            allow_report::RECEIPT_SCHEMA_ID,
            "check",
        );
        assert_inventory_contract(
            "receipt",
            &receipt,
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(42),
        );

        let diff_base_json = allow_report::render_json_with_context(
            "diff",
            &[],
            &[],
            false,
            allow_report::ReportContext {
                inventory_source: "git_tracked",
                source_tree_root: Some("H:/Code/Rust/cargo-allow"),
                inventory_files: Some(8),
                ..allow_report::ReportContext::default()
            },
        );
        let diff_json = render_diff_json_with_posture(diff_base_json, &[], &[], &[]);
        let diff = parse_json_artifact("diff", &diff_json, allow_report::REPORT_SCHEMA_ID, "diff");
        assert_eq!(
            diff.pointer("/diff/net_posture").and_then(Value::as_str),
            Some("unchanged"),
            "diff net posture"
        );

        let list_filters = empty_list_filters();
        let list_json = render_list_rows_json(
            &[],
            &list_filters,
            ListContext {
                inventory_source: "git_tracked",
                source_tree_root: Some("H:/Code/Rust/cargo-allow"),
                inventory_files: Some(46),
                kind_arg: None,
            },
        );
        let list = parse_json_artifact("list", &list_json, allow_report::LIST_SCHEMA_ID, "list");
        assert_eq!(
            list.pointer("/summary/allow_entries")
                .and_then(Value::as_u64),
            Some(0),
            "list allow_entries"
        );

        let mut cfg = AllowConfig::empty();
        let entry = test_entry("allow-json", FindingKind::NonRustFile);
        cfg.allow.push(entry.clone());
        let finding = test_finding(
            FindingKind::NonRustFile,
            None,
            "tracked.file",
            "tracked_file",
        );
        let explain_json = explain_entry_json(
            Path::new("."),
            &cfg,
            &entry,
            &[finding],
            ExplainContext {
                inventory_source: "git_tracked",
                source_tree_root: Some("H:/Code/Rust/cargo-allow"),
                inventory_files: Some(47),
            },
        );
        let explain = parse_json_artifact(
            "explain",
            &explain_json,
            allow_report::EXPLAIN_SCHEMA_ID,
            "explain",
        );
        assert_eq!(
            explain.pointer("/allow_entry/id").and_then(Value::as_str),
            Some("allow-json"),
            "explain allow id"
        );

        let add_finding = test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "method_call",
        );
        let add_entry = allow_entry_from_finding(AddEntryRequest {
            finding: &add_finding,
            id: "allow-add-json".to_string(),
            owner: "parser".to_string(),
            classification: "reviewed_exception".to_string(),
            reason: "Parser validates the input before unwrapping.".to_string(),
            evidence: vec!["test:parser_validates_input".to_string()],
            review_after: "2026-11-01".to_string(),
            expires: None,
        });
        let add_json = render_add_summary_json(
            &add_entry,
            &add_finding,
            Some(Path::new("policy/allow.proposed.toml")),
            false,
            AddContext {
                inventory_source: "git_tracked",
                source_tree_root: Some("H:/Code/Rust/cargo-allow"),
                inventory_files: Some(48),
            },
        );
        let add = parse_json_artifact("add", &add_json, allow_report::ADD_SCHEMA_ID, "add");
        assert_eq!(
            add.pointer("/allow_entry/id").and_then(Value::as_str),
            Some("allow-add-json"),
            "add allow id"
        );

        let work_items = Vec::new();
        let worklist_json = render_worklist_json_with_context(
            &work_items,
            WorklistContext {
                inventory_source: "filesystem_fallback",
                source_tree_root: Some("fixtures/source-snapshot"),
                inventory_files: Some(5),
                filters: WorklistFilters::default(),
            },
        );
        let worklist = parse_json_artifact(
            "worklist",
            &worklist_json,
            allow_report::WORKLIST_SCHEMA_ID,
            "worklist",
        );
        assert_eq!(
            worklist
                .pointer("/summary/work_items")
                .and_then(Value::as_u64),
            Some(0),
            "worklist work_items"
        );

        let prune_candidates = Vec::new();
        let prune_json = render_prune_stale_json(
            &prune_candidates,
            true,
            false,
            None,
            PruneContext {
                inventory_source: "git_tracked",
                source_tree_root: Some("H:/Code/Rust/cargo-allow"),
                inventory_files: Some(49),
            },
        );
        let prune =
            parse_json_artifact("prune", &prune_json, allow_report::PRUNE_SCHEMA_ID, "prune");
        assert_eq!(
            prune
                .pointer("/summary/stale_entries")
                .and_then(Value::as_u64),
            Some(0),
            "prune stale_entries"
        );

        let propose_json = render_propose_summary_json(
            12,
            3,
            "2026-08-01",
            Some(Path::new("policy/allow.proposed.toml")),
            true,
            ProposeContext {
                inventory_source: "git_tracked",
                source_tree_root: Some("H:/Code/Rust/cargo-allow"),
                inventory_files: Some(51),
                kind_filter: Some("panic"),
            },
        );
        let propose = parse_json_artifact(
            "propose",
            &propose_json,
            allow_report::PROPOSE_SCHEMA_ID,
            "propose",
        );
        assert_eq!(
            propose
                .pointer("/summary/baseline_debt_entries_proposed")
                .and_then(Value::as_u64),
            Some(3),
            "propose baseline_debt_entries_proposed"
        );

        let doctor_json = render_doctor_json(DoctorFacts {
            source_tree_root: "H:/Code/Rust/cargo-allow",
            root_discovery: "nearest_git_root",
            config_path: Some("H:/Code/Rust/cargo-allow/policy/allow.toml"),
            inventory_source: "git_tracked",
            files_scanned: 50,
        });
        let doctor = parse_json_artifact(
            "doctor",
            &doctor_json,
            allow_report::DOCTOR_SCHEMA_ID,
            "doctor",
        );
        assert_eq!(
            doctor.pointer("/root/discovery").and_then(Value::as_str),
            Some("nearest_git_root"),
            "doctor root discovery"
        );
    }

    #[test]
    fn clap_parses_add_from_finding() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "add",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "42",
            "--owner",
            "parser",
            "--reason",
            "validated invariant",
            "--evidence",
            "test:parser_invariant",
            "--write",
            "policy/allow.proposed.toml",
            "--force",
            "--summary-format",
            "json",
            "--summary-output",
            "target/add-summary.json",
        ]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Add(AddArgs {
                kind,
                path,
                line: 42,
                owner,
                reason,
                evidence,
                write: Some(write),
                force: true,
                summary_format: AddSummaryFormat::Json,
                summary_output: Some(summary_output),
                ..
            })) if kind == "panic"
                && path == Path::new("src/lib.rs")
                && owner == "parser"
                && reason == "validated invariant"
                && evidence == vec!["test:parser_invariant".to_string()]
                && write == Path::new("policy/allow.proposed.toml")
                && summary_output == Path::new("target/add-summary.json")
        ));
    }

    #[test]
    fn select_add_finding_picks_nearest_path_and_kind() {
        let findings = vec![
            test_finding_at_line(
                FindingKind::Panic,
                Some("unwrap"),
                "src/lib.rs",
                "method_call",
                10,
            ),
            test_finding_at_line(
                FindingKind::Panic,
                Some("expect"),
                "src/lib.rs",
                "method_call",
                40,
            ),
            test_finding_at_line(
                FindingKind::Unsafe,
                Some("unsafe_fn"),
                "src/lib.rs",
                "unsafe_fn",
                39,
            ),
        ];
        let kind = parse_kind_filter("panic")
            .unwrap_or_else(|err| std::panic::panic_any(format!("kind should parse: {err}")));

        let (_index, selected) = select_add_finding(&findings, kind, Path::new("src/lib.rs"), 39)
            .unwrap_or_else(|err| std::panic::panic_any(format!("finding should select: {err}")));

        assert_eq!(selected.family.as_deref(), Some("expect"));
        assert_eq!(selected.span.as_ref().map(|span| span.line), Some(40));
    }

    #[test]
    fn select_add_finding_fails_closed_on_equal_nearest_findings() {
        let findings = vec![
            test_finding_at_line(
                FindingKind::Panic,
                Some("unwrap"),
                "src/lib.rs",
                "method_call",
                40,
            ),
            test_finding_at_line(
                FindingKind::Panic,
                Some("expect"),
                "src/lib.rs",
                "method_call",
                42,
            ),
        ];
        let kind = parse_kind_filter("panic")
            .unwrap_or_else(|err| std::panic::panic_any(format!("kind should parse: {err}")));

        let err = select_add_finding(&findings, kind, Path::new("src/lib.rs"), 41)
            .expect_err("equally near findings should be ambiguous");

        assert!(err.to_string().contains("ambiguous add request"));
    }

    #[test]
    fn ensure_addable_outcome_rejects_already_matched_findings() {
        assert!(ensure_addable_outcome(MatchStatus::New).is_ok());

        let err = ensure_addable_outcome(MatchStatus::Matched)
            .expect_err("matched finding should not be addable");

        assert!(err.to_string().contains("already receipted"));
    }

    #[test]
    fn allow_entry_from_finding_uses_structural_selector_and_review_metadata() {
        let mut finding = test_finding_at_line(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "method_call",
            42,
        );
        finding.identity.container = Some("parse_span".to_string());
        finding.identity.callee = Some("unwrap".to_string());
        finding.identity.normalized_snippet_hash = Some("fnv1a64:1234".to_string());

        let entry = allow_entry_from_finding(AddEntryRequest {
            finding: &finding,
            id: "allow-0099".to_string(),
            owner: "parser".to_string(),
            classification: "validated_invariant".to_string(),
            reason: "Parser validates the span before unwrapping.".to_string(),
            evidence: vec!["test:parser_validates_span".to_string()],
            review_after: "2026-11-01".to_string(),
            expires: None,
        });

        assert_eq!(entry.id, "allow-0099");
        assert_eq!(entry.owner, "parser");
        assert_eq!(entry.selector.container.as_deref(), Some("parse_span"));
        assert_eq!(entry.selector.callee.as_deref(), Some("unwrap"));
        assert_eq!(
            entry.selector.normalized_snippet_hash.as_deref(),
            Some("fnv1a64:1234")
        );
        assert_eq!(entry.last_seen.as_ref().map(|last| last.line), Some(42));
    }

    #[test]
    fn render_add_summary_json_records_entry_and_selected_finding() {
        let mut finding = test_finding_at_line(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "method_call",
            42,
        );
        finding.identity.crate_name = Some("parser".to_string());
        finding.identity.container = Some("parse_span".to_string());
        finding.identity.callee = Some("unwrap".to_string());
        let mut entry = allow_entry_from_finding(AddEntryRequest {
            finding: &finding,
            id: "allow-0101".to_string(),
            owner: "parser".to_string(),
            classification: "validated_invariant".to_string(),
            reason: "Parser validates the span before unwrapping.".to_string(),
            evidence: vec!["test:parser_validates_span".to_string()],
            review_after: "2026-11-01".to_string(),
            expires: Some("2027-01-01".to_string()),
        });
        entry.selector.normalized_snippet_hash = Some("fnv1a64:1234".to_string());

        let json = render_add_summary_json(
            &entry,
            &finding,
            Some(Path::new("policy/allow.proposed.toml")),
            true,
            AddContext {
                inventory_source: "git_tracked",
                source_tree_root: Some("H:/Code/Rust/cargo-allow"),
                inventory_files: Some(52),
            },
        );
        let value = parse_json_artifact("add", &json, allow_report::ADD_SCHEMA_ID, "add");

        assert_inventory_contract(
            "add",
            &value,
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(52),
        );
        assert_eq!(
            value
                .pointer("/options/policy_output")
                .and_then(Value::as_str),
            Some("policy/allow.proposed.toml"),
            "add policy output"
        );
        assert_eq!(
            value.pointer("/options/force").and_then(Value::as_bool),
            Some(true),
            "add force"
        );
        assert_eq!(
            value.pointer("/summary/entry_id").and_then(Value::as_str),
            Some("allow-0101"),
            "add summary entry id"
        );
        assert_eq!(
            value
                .pointer("/summary/human_review_required")
                .and_then(Value::as_bool),
            Some(true),
            "add human_review_required"
        );
        assert_eq!(
            value.pointer("/allow_entry/id").and_then(Value::as_str),
            Some("allow-0101"),
            "add allow id"
        );
        assert_eq!(
            value
                .pointer("/allow_entry/evidence_count")
                .and_then(Value::as_u64),
            Some(1),
            "add evidence count"
        );
        assert_eq!(
            value
                .pointer("/selected_finding/source_package")
                .and_then(Value::as_str),
            Some("parser"),
            "add selected finding source package"
        );
    }

    #[test]
    fn list_rows_report_lifecycle_stale_and_baseline_status() {
        let mut cfg = AllowConfig::empty();
        let mut expired = test_entry("allow-expired", FindingKind::Panic);
        expired.lifecycle.expires = Some("2000-01-01".to_string());
        let mut review_due = test_entry("allow-review", FindingKind::Panic);
        review_due.lifecycle.review_after = Some("2000-01-01".to_string());
        let mut baseline = test_entry("allow-baseline", FindingKind::Panic);
        baseline.classification = "baseline_debt".to_string();
        let stale = test_entry("allow-stale", FindingKind::Panic);
        cfg.allow = vec![expired, review_due, baseline, stale];
        let outcomes = vec![
            test_outcome(
                MatchStatus::Matched,
                Some("allow-expired"),
                Some(0),
                "matched",
            ),
            test_outcome(
                MatchStatus::Matched,
                Some("allow-review"),
                Some(1),
                "matched",
            ),
            test_outcome(
                MatchStatus::Matched,
                Some("allow-baseline"),
                Some(2),
                "matched",
            ),
            test_outcome(MatchStatus::Stale, Some("allow-stale"), None, "stale"),
        ];
        let expired_finding = test_finding(
            FindingKind::NonRustFile,
            None,
            "tracked-expired.file",
            "tracked_file",
        );
        let mut review_finding = test_finding(
            FindingKind::NonRustFile,
            None,
            "tracked-review.file",
            "tracked_file",
        );
        review_finding.identity.crate_name = Some("review-package".to_string());
        let stale_finding = test_finding(
            FindingKind::NonRustFile,
            None,
            "tracked-stale.file",
            "tracked_file",
        );
        let findings = vec![expired_finding, review_finding, stale_finding];

        let rows = list_rows(&cfg, &findings, &outcomes);

        assert_eq!(row_status(&rows, "allow-expired"), MatchStatus::Expired);
        assert_eq!(row_status(&rows, "allow-review"), MatchStatus::ReviewDue);
        assert_eq!(
            rows.iter()
                .find(|row| row.id == "allow-review")
                .and_then(|row| row.source_package.as_deref()),
            Some("review-package")
        );
        assert_eq!(
            row_status(&rows, "allow-baseline"),
            MatchStatus::BaselineDebt
        );
        assert_eq!(row_status(&rows, "allow-stale"), MatchStatus::Stale);
    }

    #[test]
    fn render_list_rows_filters_owner_kind_classification_and_baseline_debt() {
        let rows = vec![
            list_row(
                "allow-runtime",
                FindingKind::Unsafe,
                "runtime",
                "baseline_debt",
            ),
            list_row(
                "allow-parser",
                FindingKind::Panic,
                "parser",
                "reviewed_exception",
            ),
        ];
        let filters = ListFilters {
            kind: Some(parse_kind_filter("unsafe").unwrap_or_else(|err| {
                std::panic::panic_any(format!("kind filter should parse: {err}"))
            })),
            family: None,
            owner: Some("runtime"),
            classification: Some("baseline_debt"),
            path: None,
            source_package: None,
            status: None,
            expired: false,
            review_due: false,
            stale: false,
            baseline_debt: true,
            broad_scope: false,
            missing_evidence: false,
        };

        let text = render_list_rows(&rows, &filters);

        assert!(text.contains("allow-runtime"));
        assert!(!text.contains("allow-parser"));
        assert!(text.contains("baseline_debt"));
    }

    #[test]
    fn render_list_rows_filters_classification_without_baseline_shortcut() {
        let rows = vec![
            list_row(
                "allow-runtime",
                FindingKind::Unsafe,
                "runtime",
                "baseline_debt",
            ),
            list_row(
                "allow-parser",
                FindingKind::Panic,
                "parser",
                "reviewed_exception",
            ),
        ];
        let filters = ListFilters {
            kind: None,
            family: None,
            owner: None,
            classification: Some("reviewed_exception"),
            path: None,
            source_package: None,
            status: None,
            expired: false,
            review_due: false,
            stale: false,
            baseline_debt: false,
            broad_scope: false,
            missing_evidence: false,
        };

        let text = render_list_rows(&rows, &filters);

        assert!(!text.contains("allow-runtime"));
        assert!(text.contains("allow-parser"));
        assert!(text.contains("reviewed_exception"));
    }

    #[test]
    fn render_list_rows_filters_source_package() {
        let mut allow_core = list_row("allow-core", FindingKind::Panic, "parser", "baseline_debt");
        allow_core.source_package = Some("allow-core".to_string());
        let mut allow_rust = list_row("allow-rust", FindingKind::Panic, "scanner", "baseline_debt");
        allow_rust.source_package = Some("allow-rust".to_string());
        let rows = vec![allow_core, allow_rust];
        let filters = ListFilters {
            kind: None,
            family: None,
            owner: None,
            classification: None,
            path: None,
            source_package: Some("allow-core"),
            status: None,
            expired: false,
            review_due: false,
            stale: false,
            baseline_debt: false,
            broad_scope: false,
            missing_evidence: false,
        };

        let text = render_list_rows(&rows, &filters);

        assert!(text.contains("allow-core"));
        assert!(!text.contains("allow-rust"));
    }

    #[test]
    fn render_list_rows_filters_family() {
        let mut indexing = list_row("allow-index", FindingKind::Panic, "parser", "baseline_debt");
        indexing.family = Some("indexing".to_string());
        let mut unwrap = list_row(
            "allow-unwrap",
            FindingKind::Panic,
            "parser",
            "baseline_debt",
        );
        unwrap.family = Some("unwrap".to_string());
        let rows = vec![indexing, unwrap];
        let filters = ListFilters {
            kind: None,
            family: Some("unwrap"),
            owner: None,
            classification: None,
            path: None,
            source_package: None,
            status: None,
            expired: false,
            review_due: false,
            stale: false,
            baseline_debt: false,
            broad_scope: false,
            missing_evidence: false,
        };

        let text = render_list_rows(&rows, &filters);

        assert!(!text.contains("allow-index"));
        assert!(text.contains("allow-unwrap"));
    }

    #[test]
    fn render_list_rows_filters_status() {
        let mut baseline = list_row(
            "allow-baseline",
            FindingKind::Panic,
            "parser",
            "baseline_debt",
        );
        baseline.status = MatchStatus::BaselineDebt;
        let mut stale = list_row(
            "allow-stale",
            FindingKind::Panic,
            "parser",
            "reviewed_exception",
        );
        stale.status = MatchStatus::Stale;
        let rows = vec![baseline, stale];
        let filters = ListFilters {
            kind: None,
            family: None,
            owner: None,
            classification: None,
            path: None,
            source_package: None,
            status: Some("stale"),
            expired: false,
            review_due: false,
            stale: false,
            baseline_debt: false,
            broad_scope: false,
            missing_evidence: false,
        };

        let text = render_list_rows(&rows, &filters);

        assert!(!text.contains("allow-baseline"));
        assert!(text.contains("allow-stale"));
    }

    #[test]
    fn render_list_rows_filters_path_prefix_and_covering_glob() {
        let mut allow_core = list_row("allow-core", FindingKind::Panic, "parser", "baseline_debt");
        allow_core.scope = "crates/allow-core/src/lib.rs".to_string();
        let mut broad = list_row(
            "allow-broad",
            FindingKind::NonRustFile,
            "tools",
            "baseline_debt",
        );
        broad.scope = "crates/allow-core/**".to_string();
        let mut allow_rust = list_row("allow-rust", FindingKind::Panic, "scanner", "baseline_debt");
        allow_rust.scope = "crates/allow-rust/src/lib.rs".to_string();
        let rows = vec![allow_core, broad, allow_rust];
        let filters = ListFilters {
            kind: None,
            family: None,
            owner: None,
            classification: None,
            path: Some(r"crates\allow-core"),
            source_package: None,
            status: None,
            expired: false,
            review_due: false,
            stale: false,
            baseline_debt: false,
            broad_scope: false,
            missing_evidence: false,
        };

        let text = render_list_rows(&rows, &filters);

        assert!(text.contains("allow-core"));
        assert!(text.contains("allow-broad"));
        assert!(!text.contains("allow-rust"));
    }

    #[test]
    fn render_list_rows_filters_broad_scope() {
        let mut exact = list_row("allow-exact", FindingKind::Panic, "parser", "baseline_debt");
        exact.scope = "crates/allow-core/src/lib.rs".to_string();
        let mut broad = list_row(
            "allow-broad",
            FindingKind::NonRustFile,
            "tools",
            "baseline_debt",
        );
        broad.scope = "crates/allow-core/**".to_string();
        let rows = vec![exact, broad];
        let filters = ListFilters {
            kind: None,
            family: None,
            owner: None,
            classification: None,
            path: None,
            source_package: None,
            status: None,
            expired: false,
            review_due: false,
            stale: false,
            baseline_debt: false,
            broad_scope: true,
            missing_evidence: false,
        };

        let text = render_list_rows(&rows, &filters);

        assert!(!text.contains("allow-exact"));
        assert!(text.contains("allow-broad"));
    }

    #[test]
    fn render_list_rows_reports_and_filters_missing_evidence() {
        let mut missing = list_row(
            "allow-missing",
            FindingKind::Panic,
            "parser",
            "baseline_debt",
        );
        missing.evidence_count = 0;
        let mut evidenced = list_row(
            "allow-evidenced",
            FindingKind::Unsafe,
            "runtime",
            "reviewed_exception",
        );
        evidenced.evidence_count = 2;
        let rows = vec![missing, evidenced];
        let filters = ListFilters {
            kind: None,
            family: None,
            owner: None,
            classification: None,
            path: None,
            source_package: None,
            status: None,
            expired: false,
            review_due: false,
            stale: false,
            baseline_debt: false,
            broad_scope: false,
            missing_evidence: true,
        };

        let text = render_list_rows(&rows, &filters);

        assert!(text.contains("evidence_count"));
        assert!(text.contains("allow-missing"));
        assert!(!text.contains("allow-evidenced"));
    }

    #[test]
    fn render_list_rows_json_records_context_filters_and_rows() {
        let mut row = list_row("allow-json", FindingKind::Panic, "parser", "baseline_debt");
        row.family = Some("unwrap".to_string());
        row.source_package = Some("allow-core".to_string());
        row.evidence_count = 2;
        row.review_after = "2026-09-01".to_string();
        row.expires = "2026-12-01".to_string();
        let filters = ListFilters {
            kind: Some(
                parse_kind_filter("panic")
                    .unwrap_or_else(|err| std::panic::panic_any(format!("kind filter: {err}"))),
            ),
            family: Some("unwrap"),
            owner: Some("parser"),
            classification: Some("baseline_debt"),
            path: Some("src/lib.rs"),
            source_package: Some("allow-core"),
            status: Some("baseline_debt"),
            expired: false,
            review_due: false,
            stale: false,
            baseline_debt: true,
            broad_scope: false,
            missing_evidence: false,
        };
        let context = ListContext {
            inventory_source: "git_tracked",
            source_tree_root: Some("H:/Code/Rust/cargo-allow"),
            inventory_files: Some(46),
            kind_arg: Some("panic"),
        };

        let json = render_list_rows_json(&[row], &filters, context);

        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains(&format!(
            "\"schema_id\": \"{}\"",
            allow_report::LIST_SCHEMA_ID
        )));
        assert!(json.contains("\"command\": \"list\""));
        assert!(json.contains("\"claim_boundary\""));
        assert!(json.contains("\"scanner_limitations\""));
        assert!(json.contains("\"source\": \"git_tracked\""));
        assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
        assert!(json.contains("\"files_scanned\": 46"));
        assert!(json.contains("\"kind\": \"panic\""));
        assert!(json.contains("\"family\": \"unwrap\""));
        assert!(json.contains("\"baseline_debt\": true"));
        assert!(json.contains("\"allow_entries\": 1"));
        assert!(json.contains("\"id\": \"allow-json\""));
        assert!(json.contains("\"source_package\": \"allow-core\""));
        assert!(json.contains("\"evidence_count\": 2"));
    }

    #[test]
    fn clap_parses_prune_stale_dry_run() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "prune",
            "--stale",
            "--dry-run",
            "--include-untracked",
            "--format",
            "json",
            "--output",
            "target/prune.json",
        ]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Prune(PruneArgs {
                stale: true,
                dry_run: true,
                include_untracked: true,
                format: PruneFormat::Json,
                output: Some(path),
                ..
            })) if path == Path::new("target/prune.json")
        ));
    }

    #[test]
    fn clap_parses_prune_stale_write() {
        let parsed =
            CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "prune", "--stale", "--write"]))
                .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Prune(PruneArgs {
                stale: true,
                write: true,
                ..
            }))
        ));
    }

    #[test]
    fn clap_rejects_prune_dry_run_and_write_together() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "prune",
            "--stale",
            "--dry-run",
            "--write",
        ]));

        assert!(parsed.is_err());
    }

    #[test]
    fn prune_stale_candidates_only_include_stale_entries() {
        let mut cfg = AllowConfig::empty();
        cfg.allow
            .push(test_entry("allow-stale", FindingKind::Panic));
        cfg.allow.push(test_entry("allow-live", FindingKind::Panic));
        let outcomes = vec![
            test_outcome(MatchStatus::Stale, Some("allow-stale"), None, "stale"),
            test_outcome(MatchStatus::Matched, Some("allow-live"), Some(0), "matched"),
        ];

        let candidates = prune_stale_candidates(&cfg, &outcomes);

        assert_eq!(candidates.len(), 1);
        let candidate = candidates
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected one prune candidate"));
        assert_eq!(candidate.id, "allow-stale");
    }

    #[test]
    fn config_without_prune_candidates_removes_only_stale_entries() {
        let mut cfg = AllowConfig::empty();
        cfg.allow
            .push(test_entry("allow-stale", FindingKind::Panic));
        cfg.allow.push(test_entry("allow-live", FindingKind::Panic));
        let candidates = vec![PruneCandidate {
            id: "allow-stale".to_string(),
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            owner: "parser".to_string(),
            classification: "reviewed_exception".to_string(),
            scope: "src/lib.rs".to_string(),
            reason: "The old exception is gone.".to_string(),
        }];

        let pruned = config_without_prune_candidates(&cfg, &candidates);

        assert_eq!(pruned.allow.len(), 1);
        assert!(pruned.allow.iter().any(|entry| entry.id == "allow-live"));
        assert!(!pruned.allow.iter().any(|entry| entry.id == "allow-stale"));
    }

    #[test]
    fn render_prune_stale_preview_is_dry_run_first() {
        let candidates = vec![PruneCandidate {
            id: "allow-stale".to_string(),
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            owner: "parser".to_string(),
            classification: "reviewed_exception".to_string(),
            scope: "src/lib.rs".to_string(),
            reason: "The old exception is gone.".to_string(),
        }];

        let text = render_prune_stale_result(&candidates, true, false, None);

        assert!(text.contains("mode: dry-run"));
        assert!(text.contains("requested: --dry-run"));
        assert!(text.contains("stale entries: 1"));
        assert!(text.contains("allow-stale"));
        assert!(text.contains("No files were changed"));
    }

    #[test]
    fn render_prune_stale_json_records_context_and_candidates() {
        let candidates = vec![PruneCandidate {
            id: "allow-stale".to_string(),
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            owner: "parser".to_string(),
            classification: "baseline_debt".to_string(),
            scope: "crates/parser/src/lib.rs".to_string(),
            reason: "old baseline entry".to_string(),
        }];

        let json = render_prune_stale_json(
            &candidates,
            true,
            false,
            None,
            PruneContext {
                inventory_source: "git_tracked",
                source_tree_root: Some("H:/Code/Rust/cargo-allow"),
                inventory_files: Some(49),
            },
        );

        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains(&format!(
            "\"schema_id\": \"{}\"",
            allow_report::PRUNE_SCHEMA_ID
        )));
        assert!(json.contains("\"command\": \"prune\""));
        assert!(json.contains("\"claim_boundary\""));
        assert!(json.contains("\"scanner_limitations\""));
        assert!(json.contains("\"source\": \"git_tracked\""));
        assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
        assert!(json.contains("\"files_scanned\": 49"));
        assert!(json.contains("\"dry_run\": true"));
        assert!(json.contains("\"write_requested\": false"));
        assert!(json.contains("\"explicit_dry_run\": true"));
        assert!(json.contains("\"written_path\": null"));
        assert!(json.contains("\"stale_entries\": 1"));
        assert!(json.contains("\"id\": \"allow-stale\""));
        assert!(json.contains("\"kind\": \"panic\""));
        assert!(json.contains("\"family\": \"unwrap\""));
    }

    #[test]
    fn render_prune_stale_result_reports_written_policy() {
        let candidates = vec![PruneCandidate {
            id: "allow-stale".to_string(),
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            owner: "parser".to_string(),
            classification: "reviewed_exception".to_string(),
            scope: "src/lib.rs".to_string(),
            reason: "The old exception is gone.".to_string(),
        }];

        let text = render_prune_stale_result(
            &candidates,
            false,
            true,
            Some(Path::new("policy/allow.toml")),
        );

        assert!(text.contains("mode: write"));
        assert!(text.contains("Removed stale entries from `policy/allow.toml`"));
        assert!(!text.contains("No files were changed"));
    }

    #[test]
    fn render_prune_stale_result_reports_write_mode_with_no_candidates() {
        let text = render_prune_stale_result(&[], false, true, None);

        assert!(text.contains("mode: write"));
        assert!(text.contains("No stale allow entries found."));
    }

    #[test]
    fn cmd_prune_write_removes_only_stale_entries_from_policy_file() {
        let root = migrate_fixture_dir();
        let policy_dir = root.join("policy");
        let docs_dir = root.join("docs");
        fs::create_dir_all(&policy_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
        fs::create_dir_all(&docs_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("docs dir: {err}")));
        fs::write(docs_dir.join("live.md"), "# live\n")
            .unwrap_or_else(|err| std::panic::panic_any(format!("live doc: {err}")));

        let mut cfg = AllowConfig::empty();
        cfg.allow
            .push(non_rust_prune_fixture_entry("allow-live", "docs/live.md"));
        cfg.allow
            .push(non_rust_prune_fixture_entry("allow-stale", "docs/stale.md"));
        let policy_path = policy_dir.join("allow.toml");
        fs::write(&policy_path, render_policy(&cfg))
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy write: {err}")));

        cmd_prune(&PruneArgs {
            root: RootArgs {
                root: Some(root.clone()),
            },
            config: Some(policy_path.clone()),
            stale: true,
            dry_run: false,
            write: true,
            include_untracked: false,
            format: PruneFormat::Human,
            output: None,
        })
        .unwrap_or_else(|err| std::panic::panic_any(format!("prune write: {err}")));

        let rendered = fs::read_to_string(&policy_path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy read: {err}")));
        let loaded = load_policy(&policy_path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy reload: {err}")));

        assert!(rendered.contains("allow-live"));
        assert!(!rendered.contains("allow-stale"));
        assert_eq!(loaded.allow.len(), 1);
        assert!(loaded.allow.iter().any(|entry| entry.id == "allow-live"));

        fs::remove_dir_all(&root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
    }

    #[test]
    fn diff_markdown_pr_summary_reports_unchanged_posture() {
        let text = render_diff_pr_summary_markdown(&[], &[], &[]);

        assert!(text.contains("**Net posture:** `unchanged`"));
        assert!(text.contains("| Current no-new failures | 0 |"));
        assert!(text.contains("no source exception posture change detected"));
    }

    #[test]
    fn diff_markdown_pr_summary_reports_review_required_for_new_source_finding() {
        let changes = vec![finding_posture_change(
            allow_diff::FindingPostureKind::New,
            "panic",
            Some("unwrap"),
            "src/lib.rs",
        )];

        let text = render_diff_pr_summary_markdown(&[], &changes, &[]);

        assert!(text.contains("**Net posture:** `review-required`"));
        assert!(text.contains("| New source findings | 1 |"));
        assert!(text.contains("review the source exception posture change"));
    }

    #[test]
    fn diff_markdown_pr_summary_reports_worse_for_policy_failure() {
        let changes = vec![policy_change(
            allow_diff::PolicyChangeSeverity::Fail,
            allow_diff::PolicyChangeKind::ScopeBroadened,
        )];

        let text = render_diff_pr_summary_markdown(&[], &[], &changes);

        assert!(text.contains("**Net posture:** `worse`"));
        assert!(text.contains("| Policy failures | 1 |"));
        assert!(text.contains("block until failing source exception changes"));
    }

    #[test]
    fn diff_markdown_pr_summary_reports_improved_for_removed_source_finding() {
        let changes = vec![finding_posture_change(
            allow_diff::FindingPostureKind::Removed,
            "panic",
            Some("unwrap"),
            "src/lib.rs",
        )];

        let text = render_diff_pr_summary_markdown(&[], &changes, &[]);

        assert!(text.contains("**Net posture:** `improved`"));
        assert!(text.contains("| Removed source findings | 1 |"));
        assert!(text.contains("keep the narrower posture"));
    }

    #[test]
    fn diff_markdown_pr_summary_reports_improved_for_removed_policy_entry() {
        let changes = vec![policy_change(
            allow_diff::PolicyChangeSeverity::Improvement,
            allow_diff::PolicyChangeKind::RemovedAllow,
        )];

        let text = render_diff_pr_summary_markdown(&[], &[], &changes);

        assert!(text.contains("**Net posture:** `improved`"));
        assert!(text.contains("| Policy improvements | 1 |"));
        assert!(text.contains("keep the narrower posture"));
    }

    #[test]
    fn diff_json_report_includes_structured_posture_changes() {
        let outcomes = vec![test_outcome(
            MatchStatus::New,
            None,
            Some(0),
            "unreceipted panic.unwrap at src/lib.rs:1:1",
        )];
        let finding_changes = vec![finding_posture_change(
            allow_diff::FindingPostureKind::New,
            "panic",
            Some("unwrap"),
            "src/lib.rs",
        )];
        let policy_changes = vec![policy_change(
            allow_diff::PolicyChangeSeverity::Fail,
            allow_diff::PolicyChangeKind::ScopeBroadened,
        )];

        let json = render_diff_json_with_posture(
            "{\n  \"schema_id\": \"cargo-allow.report.v1\"\n}".to_string(),
            &outcomes,
            &finding_changes,
            &policy_changes,
        );

        assert!(json.contains("\"diff\""));
        assert!(json.contains("\"net_posture\": \"worse\""));
        assert!(json.contains("\"current_failures\": 1"));
        assert!(json.contains("\"new_findings\": 1"));
        assert!(json.contains("\"policy_failures\": 1"));
        assert!(json.contains("\"policy_improvements\": 0"));
        assert!(json.contains("\"finding_changes\""));
        assert!(json.contains("\"change\": \"new\""));
        assert!(json.contains("\"family\": \"unwrap\""));
        assert!(json.contains("\"policy_changes\""));
        assert!(json.contains("\"severity\": \"fail\""));
        assert!(json.contains("\"kind\": \"scope_broadened\""));
        assert!(json.ends_with("}\n"));
    }

    #[test]
    fn diff_json_report_keeps_base_report_when_append_fails() {
        let base = "not json".to_string();

        let json = render_diff_json_with_posture(base.clone(), &[], &[], &[]);

        assert_eq!(json, base);
    }

    #[test]
    fn report_schema_documents_diff_posture_contract() {
        let schema = include_str!("../../../docs/schemas/report.schema.json");

        assert!(schema.contains("\"diff\""));
        assert!(schema.contains("\"net_posture\""));
        assert!(schema.contains("\"finding_changes\""));
        assert!(schema.contains("\"policy_changes\""));
        assert!(schema.contains("\"scope_broadened\""));
        assert!(schema.contains("\"scope_narrowed\""));
        assert!(schema.contains("\"removed_allow\""));
        assert!(schema.contains("\"selector_precision_increased\""));
        assert!(schema.contains("\"evidence_added\""));
        assert!(schema.contains("\"expiry_shortened\""));
        assert!(schema.contains("\"review_after_shortened\""));
        assert!(schema.contains("\"owner_added\""));
        assert!(schema.contains("\"reason_added\""));
        assert!(schema.contains("\"classification_added\""));
        assert!(schema.contains("\"occurrence_limit_tightened\""));
        assert!(schema.contains("\"policy_improvements\""));
    }

    #[test]
    fn clap_parses_explain_id_and_config() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "explain",
            "allow-0001",
            "--config",
            "policy/custom.toml",
            "--include-untracked",
            "--format",
            "json",
            "--output",
            "target/explain.json",
        ]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Explain(ExplainArgs {
                id,
                config,
                include_untracked: true,
                format: ExplainFormat::Json,
                output,
                ..
            })) if id == "allow-0001"
                && config.as_deref() == Some(Path::new("policy/custom.toml"))
                && output.as_deref() == Some(Path::new("target/explain.json"))
        ));
    }

    #[test]
    fn clap_parses_worklist_json_output() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "worklist",
            "--kind",
            "unsafe",
            "--family",
            "unsafe_fn",
            "--item-kind",
            "baseline_debt",
            "--status",
            "baseline_debt",
            "--allow-id",
            "allow-0001",
            "--path",
            "crates/allow-core",
            "--source-package",
            "allow-core",
            "--owner",
            "runtime",
            "--classification",
            "baseline_debt",
            "--baseline-debt",
            "--broad-scope",
            "--risk",
            "medium",
            "--difficulty",
            "small",
            "--missing-evidence",
            "--format",
            "json",
            "--output",
            "target/worklist.json",
        ]))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("CLI should parse worklist args: {err}"))
        });

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Worklist(WorklistArgs {
                kind: Some(kind),
                family: Some(family),
                item_kind: Some(item_kind),
                status: Some(status),
                allow_id: Some(allow_id),
                path: Some(path_filter),
                source_package: Some(source_package),
                owner: Some(owner),
                classification: Some(classification),
                baseline_debt: true,
                broad_scope: true,
                risk: Some(risk),
                difficulty: Some(difficulty),
                missing_evidence: true,
                format: WorklistFormat::Json,
                output: Some(path),
                ..
            })) if kind == "unsafe"
                && family == "unsafe_fn"
                && item_kind == "baseline_debt"
                && status == "baseline_debt"
                && allow_id == "allow-0001"
                && path_filter == "crates/allow-core"
                && source_package == "allow-core"
                && owner == "runtime"
                && classification == "baseline_debt"
                && risk == "medium"
                && difficulty == "small"
                && path == Path::new("target/worklist.json")
        ));
    }

    #[test]
    fn explain_entry_text_reports_live_match_status() {
        let mut cfg = AllowConfig::empty();
        let entry = test_entry("allow-file", FindingKind::NonRustFile);
        cfg.allow.push(entry.clone());
        let mut finding = test_finding(
            FindingKind::NonRustFile,
            None,
            "tracked.file",
            "tracked_file",
        );
        finding.identity.crate_name = Some("fixture-package".to_string());
        let findings = vec![finding];

        let text = explain_entry_text(Path::new("."), &cfg, &entry, &findings);

        assert!(text.contains("current_status: matched"));
        assert!(text.contains("current_matches: 1"));
        assert!(text.contains("match_outcomes: matched=1"));
        assert!(text.contains("matched: tracked.file:1:1"));
        assert!(text.contains("source_package=fixture-package"));
        assert!(text.contains("Claim boundary: scanned source-tree/source syntax only"));
        assert!(text.contains("did not invoke Cargo metadata"));
    }

    #[test]
    fn explain_entry_json_records_context_and_live_status() {
        let mut cfg = AllowConfig::empty();
        let mut entry = test_entry("allow-json", FindingKind::NonRustFile);
        entry.family = Some("documentation".to_string());
        entry.evidence = vec!["test:explain_fixture".to_string()];
        entry.lifecycle.created = Some("2026-05-27".to_string());
        entry.lifecycle.review_after = Some("2026-11-01".to_string());
        cfg.allow.push(entry.clone());
        let mut finding = test_finding(
            FindingKind::NonRustFile,
            Some("documentation"),
            "tracked.file",
            "tracked_file",
        );
        finding.identity.crate_name = Some("allow-core".to_string());

        let json = explain_entry_json(
            Path::new("."),
            &cfg,
            &entry,
            &[finding],
            ExplainContext {
                inventory_source: "git_tracked",
                source_tree_root: Some("H:/Code/Rust/cargo-allow"),
                inventory_files: Some(47),
            },
        );

        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains(&format!(
            "\"schema_id\": \"{}\"",
            allow_report::EXPLAIN_SCHEMA_ID
        )));
        assert!(json.contains("\"command\": \"explain\""));
        assert!(json.contains("\"claim_boundary\""));
        assert!(json.contains("\"scanner_limitations\""));
        assert!(json.contains("\"cargo_metadata_not_invoked\""));
        assert!(json.contains("\"repository_code_not_executed\""));
        assert!(json.contains("\"source\": \"git_tracked\""));
        assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
        assert!(json.contains("\"files_scanned\": 47"));
        assert!(json.contains("\"id\": \"allow-json\""));
        assert!(json.contains("\"current_status\": \"matched\""));
        assert!(json.contains("\"current_matches\": 1"));
        assert!(json.contains("\"path\": \"tracked.file\""));
        assert!(json.contains("\"source_package\": \"allow-core\""));
        assert!(json.contains("\"status\": \"traceability_only\""));
        assert!(json.contains("\"suggested_actions\": []"));
        assert!(json.contains("\"proof_commands\": []"));
    }

    #[test]
    fn explain_entry_text_reports_baseline_debt_next_actions() {
        let mut cfg = AllowConfig::empty();
        let mut entry = test_entry("allow-baseline", FindingKind::Panic);
        entry.classification = "baseline_debt".to_string();
        entry.family = Some("unwrap".to_string());
        cfg.allow.push(entry.clone());
        let finding = test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "tracked.file",
            "tracked_file",
        );

        let text = explain_entry_text(Path::new("."), &cfg, &entry, &[finding]);

        assert!(text.contains("current_status: matched"));
        assert!(text.contains("baseline_debt and still needs human review"));
        assert!(text.contains("next:"));
        assert!(text.contains("action: replace generated baseline debt"));
        assert!(text.contains("proof: cargo-allow explain allow-baseline"));
        assert!(text.contains("proof: cargo-allow check --kind panic --mode no-new"));
    }

    #[test]
    fn explain_entry_text_reports_evidence_reference_status() {
        let root = migrate_fixture_dir();
        fs::create_dir_all(root.join("docs"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
        fs::write(root.join("docs/safety.md"), "review notes")
            .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence: {err}")));
        let mut cfg = AllowConfig::empty();
        let mut entry = test_entry("allow-file", FindingKind::NonRustFile);
        entry.evidence = vec![
            "doc:docs/safety.md".to_string(),
            "spec:docs/missing.md".to_string(),
            "test:file_policy_fixture".to_string(),
        ];
        cfg.allow.push(entry.clone());

        let text = explain_entry_text(&root, &cfg, &entry, &[]);

        assert!(text.contains("evidence references:"));
        assert!(text.contains("doc:docs/safety.md"));
        assert!(text.contains("status=local_file_present"));
        assert!(text.contains("spec:docs/missing.md"));
        assert!(text.contains("status=local_file_missing"));
        assert!(text.contains("test:file_policy_fixture"));
        assert!(text.contains("status=traceability_only"));
        fs::remove_dir_all(root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
    }

    #[test]
    fn explain_entry_text_reports_stale_entry() {
        let mut cfg = AllowConfig::empty();
        let entry = test_entry("allow-file", FindingKind::NonRustFile);
        cfg.allow.push(entry.clone());

        let text = explain_entry_text(Path::new("."), &cfg, &entry, &[]);

        assert!(text.contains("current_status: stale"));
        assert!(text.contains("current_matches: 0"));
        assert!(text.contains("match_outcomes: stale=1"));
        assert!(text.contains("allow-file is stale"));
        assert!(text.contains("next:"));
        assert!(text.contains("action: remove the stale allow entry"));
        assert!(text.contains("proof: cargo-allow explain allow-file"));
    }

    #[test]
    fn explain_entry_text_reports_occurrence_limit_exceeded() {
        let mut cfg = AllowConfig::empty();
        let mut entry = test_entry("allow-file", FindingKind::NonRustFile);
        entry.occurrence_limit = Some(1);
        cfg.allow.push(entry.clone());
        let finding = test_finding(
            FindingKind::NonRustFile,
            None,
            "tracked.file",
            "tracked_file",
        );
        let findings = vec![finding.clone(), finding];

        let text = explain_entry_text(Path::new("."), &cfg, &entry, &findings);

        assert!(text.contains("occurrence_limit: 1"));
        assert!(text.contains("current_status: new"));
        assert!(text.contains("current_matches: 2"));
        assert!(text.contains("match_outcomes: matched=1, new=1"));
        assert!(text.contains("occurrence_limit exceeded"));
    }

    #[test]
    fn worklist_json_emits_stale_allow_actions() {
        let mut cfg = AllowConfig::empty();
        let mut entry = test_entry("allow-file", FindingKind::NonRustFile);
        entry.lifecycle.created = Some("2026-05-01".to_string());
        entry.lifecycle.review_after = Some("2026-06-01".to_string());
        entry.lifecycle.expires = Some("2026-08-01".to_string());
        entry.evidence = vec!["doc:docs/policy/file.md".to_string()];
        cfg.allow.push(entry);
        let outcomes = vec![test_outcome(
            MatchStatus::Stale,
            Some("allow-file"),
            None,
            "allow-file is stale: no current finding matched tracked.file",
        )];

        let items = work_items_from_outcomes(&cfg, &[], &outcomes);
        let json = render_worklist_json_with_context(&items, WorklistContext::default());
        let human = render_worklist_human_with_context(&items, WorklistContext::default());

        assert_eq!(items.len(), 1);
        assert!(json.contains(&format!(
            "\"schema_id\": \"{}\"",
            allow_report::WORKLIST_SCHEMA_ID
        )));
        assert!(json.contains("\"source_tree_inventory\""));
        assert!(json.contains("\"cargo_commands_not_invoked\""));
        assert!(json.contains("\"repository_code_not_executed\""));
        assert!(json.contains("\"scanner_limitations\""));
        assert!(json.contains("\"inventory\""));
        assert!(json.contains("\"source\": \"unknown\""));
        assert!(json.contains("\"kind\": \"stale_allow\""));
        assert!(json.contains("\"exception_kind\": \"non_rust_file\""));
        assert!(json.contains("\"family\": null"));
        assert!(json.contains("\"owner\": \"owner\""));
        assert!(json.contains("\"classification\": \"classification\""));
        assert!(json.contains("\"reason\": \"reason\""));
        assert!(json.contains("\"created\": \"2026-05-01\""));
        assert!(json.contains("\"review_after\": \"2026-06-01\""));
        assert!(json.contains("\"expires\": \"2026-08-01\""));
        assert!(json.contains("\"evidence_count\": 1"));
        assert!(json.contains("\"risk\": \"low\""));
        assert!(json.contains("\"small_difficulty\": 1"));
        assert!(json.contains("\"medium_difficulty\": 0"));
        assert!(json.contains("\"source_package\": null"));
        assert!(json.contains("\"cargo-allow explain allow-file\""));
        assert!(json.contains("\"cargo-allow check --kind non-rust --mode no-new\""));
        assert!(human.contains("owner: owner"));
        assert!(human.contains("classification: classification"));
        assert!(human.contains("reason: reason"));
        assert!(human.contains("created: 2026-05-01"));
        assert!(human.contains("review_after: 2026-06-01"));
        assert!(human.contains("expires: 2026-08-01"));
        assert!(human.contains("evidence: 1 reference(s)"));
    }

    #[test]
    fn worklist_schema_documents_current_contract() {
        let schema = include_str!("../../../docs/schemas/worklist.schema.json");

        assert!(schema.contains(allow_report::WORKLIST_SCHEMA_ID));
        assert!(schema.contains("\"exception_kind\""));
        assert!(schema.contains("\"family\""));
        assert!(schema.contains("\"owner\""));
        assert!(schema.contains("\"classification\""));
        assert!(schema.contains("\"reason\""));
        assert!(schema.contains("\"created\""));
        assert!(schema.contains("\"review_after\""));
        assert!(schema.contains("\"expires\""));
        assert!(schema.contains("\"evidence_count\""));
        assert!(schema.contains("\"source_package\""));
        assert!(schema.contains("\"proof_commands\""));
        assert!(schema.contains("\"scanner_limitations\""));
        assert!(schema.contains("\"scanner_limitation\""));
        assert!(schema.contains("\"macro_expansion_not_analyzed\""));
        assert!(schema.contains("\"small_difficulty\""));
        assert!(schema.contains("\"medium_difficulty\""));
        assert!(schema.contains("\"filters\""));
        assert!(schema.contains("\"family\""));
        assert!(schema.contains("\"item_kind\""));
        assert!(schema.contains("\"status\""));
        assert!(schema.contains("\"allow_id\""));
        assert!(schema.contains("\"path\""));
        assert!(schema.contains("\"source_package\""));
        assert!(schema.contains("\"baseline_debt\""));
        assert!(schema.contains("\"broad_scope\""));
        assert!(schema.contains("\"missing_evidence\""));
        assert!(schema.contains("\"inventory\""));
        assert!(schema.contains("\"git_tracked\""));
        assert!(schema.contains("\"source_tree_inventory\""));
    }

    #[test]
    fn explain_schema_documents_current_contract() {
        let schema = include_str!("../../../docs/schemas/explain.schema.json");

        assert!(schema.contains(allow_report::EXPLAIN_SCHEMA_ID));
        assert!(schema.contains("\"allow_entry\""));
        assert!(schema.contains("\"evidence_references\""));
        assert!(schema.contains("\"current_findings\""));
        assert!(schema.contains("\"match_outcomes\""));
        assert!(schema.contains("\"next\""));
        assert!(schema.contains("\"scanner_limitations\""));
        assert!(schema.contains("\"scanner_limitation\""));
        assert!(schema.contains("\"source_package\""));
        assert!(schema.contains("\"cargo_metadata_not_invoked\""));
        assert!(schema.contains("\"repository_code_not_executed\""));
    }

    #[test]
    fn prune_schema_documents_current_contract() {
        let schema = include_str!("../../../docs/schemas/prune.schema.json");

        assert!(schema.contains(allow_report::PRUNE_SCHEMA_ID));
        assert!(schema.contains("\"mode\""));
        assert!(schema.contains("\"dry_run\""));
        assert!(schema.contains("\"written_path\""));
        assert!(schema.contains("\"stale_entries\""));
        assert!(schema.contains("\"scanner_limitations\""));
        assert!(schema.contains("\"scanner_limitation\""));
        assert!(schema.contains("\"cargo_metadata_not_invoked\""));
        assert!(schema.contains("\"repository_code_not_executed\""));
    }

    #[test]
    fn doctor_schema_documents_current_contract() {
        let schema = include_str!("../../../docs/schemas/doctor.schema.json");

        assert!(schema.contains(allow_report::DOCTOR_SCHEMA_ID));
        assert!(schema.contains("\"root\""));
        assert!(schema.contains("\"discovery\""));
        assert!(schema.contains("\"config\""));
        assert!(schema.contains("\"inventory\""));
        assert!(schema.contains("\"files_scanned\""));
        assert!(schema.contains("\"scanner_limitations\""));
        assert!(schema.contains("\"scanner_limitation\""));
        assert!(schema.contains("\"cargo_metadata_not_invoked\""));
        assert!(schema.contains("\"repository_code_not_executed\""));
    }

    #[test]
    fn propose_schema_documents_current_contract() {
        let schema = include_str!("../../../docs/schemas/propose.schema.json");

        assert!(schema.contains(allow_report::PROPOSE_SCHEMA_ID));
        assert!(schema.contains("\"options\""));
        assert!(schema.contains("\"policy_output\""));
        assert!(schema.contains("\"baseline_debt_entries_proposed\""));
        assert!(schema.contains("\"generated_entry_defaults\""));
        assert!(schema.contains("\"scanner_limitations\""));
        assert!(schema.contains("\"scanner_limitation\""));
        assert!(schema.contains("\"cargo_metadata_not_invoked\""));
        assert!(schema.contains("\"repository_code_not_executed\""));
    }

    #[test]
    fn add_schema_documents_current_contract() {
        let schema = include_str!("../../../docs/schemas/add.schema.json");

        assert!(schema.contains(allow_report::ADD_SCHEMA_ID));
        assert!(schema.contains("\"options\""));
        assert!(schema.contains("\"policy_output\""));
        assert!(schema.contains("\"allow_entry\""));
        assert!(schema.contains("\"selected_finding\""));
        assert!(schema.contains("\"human_review_required\""));
        assert!(schema.contains("\"scanner_limitations\""));
        assert!(schema.contains("\"scanner_limitation\""));
        assert!(schema.contains("\"cargo_metadata_not_invoked\""));
        assert!(schema.contains("\"repository_code_not_executed\""));
    }

    #[test]
    fn worklist_renderers_include_inventory_context() {
        let items = Vec::new();
        let context = WorklistContext {
            inventory_source: "git_tracked",
            source_tree_root: Some("H:/Code/Rust/cargo-allow"),
            inventory_files: Some(46),
            filters: WorklistFilters::default(),
        };

        let json = render_worklist_json_with_context(&items, context);
        let human = render_worklist_human_with_context(&items, context);

        assert!(json.contains("\"scope\": \"source_tree\""));
        assert!(json.contains("\"scanner\": \"source_syntax\""));
        assert!(json.contains("\"source\": \"git_tracked\""));
        assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
        assert!(json.contains("\"files_scanned\": 46"));
        assert!(json.contains("\"filters\""));
        assert!(json.contains("\"risk\": null"));
        assert!(
            human.contains(
                "Inventory: source_tree/source_syntax via git_tracked; files scanned: 46"
            )
        );
        assert!(human.contains("Source tree root: H:/Code/Rust/cargo-allow"));
        assert!(human.contains("Filters: none"));
    }

    #[test]
    fn worklist_renderers_include_applied_filters() {
        let items = Vec::new();
        let context = WorklistContext {
            inventory_source: "git_tracked",
            source_tree_root: None,
            inventory_files: Some(46),
            filters: WorklistFilters {
                kind: Some("unsafe"),
                family: Some("unsafe_fn"),
                item_kind: Some("baseline_debt"),
                status: Some("baseline_debt"),
                allow_id: Some("allow-0001"),
                path: Some("crates/allow-core"),
                source_package: Some("allow-core"),
                owner: Some("runtime"),
                classification: Some("baseline_debt"),
                baseline_debt: true,
                broad_scope: true,
                risk: Some("high"),
                difficulty: Some("medium"),
                missing_evidence: true,
            },
        };

        let json = render_worklist_json_with_context(&items, context);
        let human = render_worklist_human_with_context(&items, context);

        assert!(json.contains("\"filters\""));
        assert!(json.contains("\"kind\": \"unsafe\""));
        assert!(json.contains("\"family\": \"unsafe_fn\""));
        assert!(json.contains("\"item_kind\": \"baseline_debt\""));
        assert!(json.contains("\"status\": \"baseline_debt\""));
        assert!(json.contains("\"allow_id\": \"allow-0001\""));
        assert!(json.contains("\"path\": \"crates/allow-core\""));
        assert!(json.contains("\"source_package\": \"allow-core\""));
        assert!(json.contains("\"owner\": \"runtime\""));
        assert!(json.contains("\"classification\": \"baseline_debt\""));
        assert!(json.contains("\"baseline_debt\": true"));
        assert!(json.contains("\"broad_scope\": true"));
        assert!(json.contains("\"risk\": \"high\""));
        assert!(json.contains("\"difficulty\": \"medium\""));
        assert!(json.contains("\"missing_evidence\": true"));
        assert!(human.contains(
            "Filters: kind=unsafe, family=unsafe_fn, item_kind=baseline_debt, status=baseline_debt, allow_id=allow-0001, path=crates/allow-core, source_package=allow-core, owner=runtime, classification=baseline_debt, baseline_debt=true, broad_scope=true, risk=high, difficulty=medium, missing_evidence=true"
        ));
    }

    #[test]
    fn worklist_human_output_reports_truncated_items() {
        let cfg = AllowConfig::empty();
        let findings = (0..81)
            .map(|index| {
                test_finding(
                    FindingKind::Panic,
                    Some("unwrap"),
                    &format!("src/file_{index}.rs"),
                    "method_call",
                )
            })
            .collect::<Vec<_>>();
        let outcomes = (0..81)
            .map(|index| {
                test_outcome(
                    MatchStatus::New,
                    None,
                    Some(index),
                    &format!("unreceipted panic.unwrap at src/file_{index}.rs:1:1"),
                )
            })
            .collect::<Vec<_>>();

        let items = work_items_from_outcomes(&cfg, &findings, &outcomes);
        let human = render_worklist_human_with_context(&items, WorklistContext::default());

        assert!(human.contains("work-new-unreceipted-finding-0080"));
        assert!(!human.contains("work-new-unreceipted-finding-0081"));
        assert!(human.contains("1 additional work items omitted from human output"));
        assert!(human.contains("cargo-allow worklist --format json"));
    }

    #[test]
    fn worklist_items_prioritize_unsafe_new_findings() {
        let cfg = AllowConfig::empty();
        let findings = vec![test_finding(
            FindingKind::Unsafe,
            Some("unsafe_fn"),
            "src/lib.rs",
            "unsafe_fn",
        )];
        let outcomes = vec![test_outcome(
            MatchStatus::New,
            None,
            Some(0),
            "unreceipted unsafe.unsafe_fn at src/lib.rs:1:1",
        )];

        let items = work_items_from_outcomes(&cfg, &findings, &outcomes);
        let text = render_worklist_human_with_context(&items, WorklistContext::default());

        let item = items
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
        assert_eq!(item.kind, "new_unreceipted_finding");
        assert_eq!(item.exception_kind.as_deref(), Some("unsafe"));
        assert_eq!(item.family.as_deref(), Some("unsafe_fn"));
        assert_eq!(item.risk, "high");
        assert!(
            item.proof_commands
                .iter()
                .any(|command| { command == "cargo-allow check --kind unsafe --mode no-new" })
        );
        assert!(
            item.proof_commands
                .iter()
                .any(|command| command == "cargo-allow worklist --kind unsafe --format json")
        );
        assert!(
            item.proof_commands
                .iter()
                .all(|command| command.starts_with("cargo-allow "))
        );
        assert!(text.contains("work-new-unreceipted-finding-0001"));
        assert!(text.contains("exception: unsafe.unsafe_fn"));
        assert!(text.contains("action: remove the new source exception if it is accidental"));
        assert!(text.contains("proof: cargo-allow check --kind unsafe --mode no-new"));
        assert!(text.contains("proof: cargo-allow worklist --kind unsafe --format json"));
        assert!(text.contains("Difficulty:"));
        assert!(text.contains("  medium    1"));
    }

    #[test]
    fn worklist_items_include_explicit_source_package_context() {
        let cfg = AllowConfig::empty();
        let mut finding = test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "crates/parser/src/lib.rs",
            "method_call",
        );
        finding.identity.crate_name = Some("parser".to_string());
        let outcomes = vec![test_outcome(
            MatchStatus::New,
            None,
            Some(0),
            "unreceipted panic.unwrap at crates/parser/src/lib.rs:1:1",
        )];

        let items = work_items_from_outcomes(&cfg, &[finding], &outcomes);
        let json = render_worklist_json_with_context(&items, WorklistContext::default());
        let human = render_worklist_human_with_context(&items, WorklistContext::default());
        let item = items
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected one work item"));

        assert_eq!(item.source_package.as_deref(), Some("parser"));
        assert_eq!(item.exception_kind.as_deref(), Some("panic"));
        assert_eq!(item.family.as_deref(), Some("unwrap"));
        assert!(
            item.suggested_actions
                .iter()
                .any(|action| action.contains("package `parser`"))
        );
        assert!(json.contains("\"source_package\": \"parser\""));
        assert!(json.contains("\"exception_kind\": \"panic\""));
        assert!(json.contains("\"family\": \"unwrap\""));
        assert!(human.contains("source package: parser"));
        assert!(human.contains("exception: panic.unwrap"));
        assert!(
            item.proof_commands
                .iter()
                .all(|command| command.starts_with("cargo-allow "))
        );
    }

    #[test]
    fn worklist_items_prioritize_process_policy_findings() {
        let cfg = AllowConfig::empty();
        let findings = vec![test_finding(
            FindingKind::PolicyException,
            Some("process_spawn"),
            ".github/workflows/ci.yml",
            "process_spawn",
        )];
        let outcomes = vec![test_outcome(
            MatchStatus::New,
            None,
            Some(0),
            "unreceipted policy_exception.process_spawn at .github/workflows/ci.yml:1:1",
        )];

        let items = work_items_from_outcomes(&cfg, &findings, &outcomes);

        let item = items
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
        assert_eq!(item.kind, "new_unreceipted_finding");
        assert_eq!(item.exception_kind.as_deref(), Some("policy_exception"));
        assert_eq!(item.family.as_deref(), Some("process_spawn"));
        assert_eq!(item.risk, "high");
        assert_eq!(item.difficulty, "medium");
        assert!(
            item.proof_commands
                .iter()
                .any(|command| command == "cargo-allow check --kind process --mode no-new")
        );
        assert!(
            item.proof_commands
                .iter()
                .any(|command| command == "cargo-allow worklist --kind process --format json")
        );
    }

    #[test]
    fn worklist_items_treat_new_non_rust_files_as_small() {
        let cfg = AllowConfig::empty();
        let findings = vec![test_finding(
            FindingKind::NonRustFile,
            Some("shell_script"),
            "scripts/new.sh",
            "tracked_file",
        )];
        let outcomes = vec![test_outcome(
            MatchStatus::New,
            None,
            Some(0),
            "unreceipted non_rust_file.shell_script at scripts/new.sh:1:1",
        )];

        let items = work_items_from_outcomes(&cfg, &findings, &outcomes);

        let item = items
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
        assert_eq!(item.kind, "new_unreceipted_finding");
        assert_eq!(item.exception_kind.as_deref(), Some("non_rust_file"));
        assert_eq!(item.family.as_deref(), Some("shell_script"));
        assert_eq!(item.risk, "medium");
        assert_eq!(item.difficulty, "small");
        assert!(
            item.proof_commands
                .iter()
                .any(|command| command == "cargo-allow check --kind non-rust --mode no-new")
        );
    }

    #[test]
    fn worklist_items_keep_stale_allows_low_risk_even_for_unsafe() {
        let mut cfg = AllowConfig::empty();
        cfg.allow
            .push(test_entry("allow-unsafe", FindingKind::Unsafe));
        let outcomes = vec![test_outcome(
            MatchStatus::Stale,
            Some("allow-unsafe"),
            None,
            "allow-unsafe is stale: no current finding matched src/lib.rs",
        )];

        let items = work_items_from_outcomes(&cfg, &[], &outcomes);

        let item = items
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
        assert_eq!(item.kind, "stale_allow");
        assert_eq!(item.exception_kind.as_deref(), Some("unsafe"));
        assert_eq!(item.risk, "low");
        assert_eq!(item.difficulty, "small");
    }

    #[test]
    fn worklist_filters_by_risk_and_difficulty() {
        let mut cfg = AllowConfig::empty();
        cfg.allow
            .push(test_entry("allow-stale", FindingKind::NonRustFile));
        let findings = vec![
            test_finding(
                FindingKind::PolicyException,
                Some("process_spawn"),
                ".github/workflows/ci.yml",
                "process_spawn",
            ),
            test_finding(
                FindingKind::NonRustFile,
                Some("shell_script"),
                "scripts/new.sh",
                "tracked_file",
            ),
        ];
        let outcomes = vec![
            test_outcome(
                MatchStatus::New,
                None,
                Some(0),
                "unreceipted process policy exception",
            ),
            test_outcome(MatchStatus::New, None, Some(1), "unreceipted shell script"),
            test_outcome(
                MatchStatus::Stale,
                Some("allow-stale"),
                None,
                "allow-stale is stale",
            ),
        ];

        let items = work_items_from_outcomes(&cfg, &findings, &outcomes);
        let filtered = filter_work_items(
            items,
            WorklistFilters {
                risk: Some("medium"),
                difficulty: Some("small"),
                ..WorklistFilters::default()
            },
        );

        assert_eq!(filtered.len(), 1);
        let item = filtered
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
        assert_eq!(item.kind, "new_unreceipted_finding");
        assert_eq!(item.exception_kind.as_deref(), Some("non_rust_file"));
        assert_eq!(item.risk, "medium");
        assert_eq!(item.difficulty, "small");
        assert_eq!(item.path.as_deref(), Some("scripts/new.sh"));
    }

    #[test]
    fn worklist_filters_by_owner_and_classification() {
        let mut cfg = AllowConfig::empty();
        let mut first = test_entry("allow-first", FindingKind::NonRustFile);
        first.owner = "team-a".to_string();
        first.classification = "baseline_debt".to_string();
        let mut second = test_entry("allow-second", FindingKind::NonRustFile);
        second.owner = "team-b".to_string();
        second.classification = "reviewed_exception".to_string();
        cfg.allow.push(first);
        cfg.allow.push(second);
        let outcomes = vec![
            test_outcome(
                MatchStatus::Stale,
                Some("allow-first"),
                None,
                "allow-first is stale",
            ),
            test_outcome(
                MatchStatus::Stale,
                Some("allow-second"),
                None,
                "allow-second is stale",
            ),
        ];

        let items = work_items_from_outcomes(&cfg, &[], &outcomes);
        let filtered = filter_work_items(
            items,
            WorklistFilters {
                owner: Some("team-a"),
                classification: Some("baseline_debt"),
                ..WorklistFilters::default()
            },
        );

        assert_eq!(filtered.len(), 1);
        let item = filtered
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
        assert_eq!(item.allow_id.as_deref(), Some("allow-first"));
        assert_eq!(item.owner.as_deref(), Some("team-a"));
        assert_eq!(item.classification.as_deref(), Some("baseline_debt"));
    }

    #[test]
    fn worklist_filters_by_item_kind() {
        let mut cfg = AllowConfig::empty();
        cfg.allow
            .push(test_entry("allow-stale", FindingKind::NonRustFile));
        let findings = vec![test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "method_call",
        )];
        let outcomes = vec![
            test_outcome(MatchStatus::New, None, Some(0), "unreceipted panic.unwrap"),
            test_outcome(
                MatchStatus::Stale,
                Some("allow-stale"),
                None,
                "allow-stale is stale",
            ),
        ];

        let items = work_items_from_outcomes(&cfg, &findings, &outcomes);
        let filtered = filter_work_items(
            items,
            WorklistFilters {
                item_kind: Some("stale_allow"),
                ..WorklistFilters::default()
            },
        );

        assert_eq!(filtered.len(), 1);
        let item = filtered
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
        assert_eq!(item.kind, "stale_allow");
        assert_eq!(item.allow_id.as_deref(), Some("allow-stale"));
    }

    #[test]
    fn worklist_filters_by_status() {
        let mut cfg = AllowConfig::empty();
        cfg.allow
            .push(test_entry("allow-stale", FindingKind::NonRustFile));
        let findings = vec![test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "method_call",
        )];
        let outcomes = vec![
            test_outcome(MatchStatus::New, None, Some(0), "unreceipted panic.unwrap"),
            test_outcome(
                MatchStatus::Stale,
                Some("allow-stale"),
                None,
                "allow-stale is stale",
            ),
        ];

        let items = work_items_from_outcomes(&cfg, &findings, &outcomes);
        let filtered = filter_work_items(
            items,
            WorklistFilters {
                status: Some("stale"),
                ..WorklistFilters::default()
            },
        );

        assert_eq!(filtered.len(), 1);
        let item = filtered
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
        assert_eq!(item.status, MatchStatus::Stale);
        assert_eq!(item.allow_id.as_deref(), Some("allow-stale"));
    }

    #[test]
    fn worklist_filters_by_allow_id() {
        let mut cfg = AllowConfig::empty();
        cfg.allow
            .push(test_entry("allow-first", FindingKind::NonRustFile));
        cfg.allow
            .push(test_entry("allow-second", FindingKind::NonRustFile));
        let outcomes = vec![
            test_outcome(
                MatchStatus::Stale,
                Some("allow-first"),
                None,
                "allow-first is stale",
            ),
            test_outcome(
                MatchStatus::Stale,
                Some("allow-second"),
                None,
                "allow-second is stale",
            ),
        ];

        let items = work_items_from_outcomes(&cfg, &[], &outcomes);
        let filtered = filter_work_items(
            items,
            WorklistFilters {
                allow_id: Some("allow-second"),
                ..WorklistFilters::default()
            },
        );

        assert_eq!(filtered.len(), 1);
        let item = filtered
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
        assert_eq!(item.allow_id.as_deref(), Some("allow-second"));
    }

    #[test]
    fn worklist_filters_by_advisory_shortcuts() {
        let baseline = WorkItem {
            id: "work-baseline-debt-0001".to_string(),
            kind: "baseline_debt".to_string(),
            exception_kind: Some("panic".to_string()),
            family: Some("unwrap".to_string()),
            owner: Some("runtime".to_string()),
            classification: Some("baseline_debt".to_string()),
            reason: Some("fixture".to_string()),
            created: None,
            review_after: None,
            expires: Some("2026-08-01".to_string()),
            evidence_count: Some(0),
            risk: "medium",
            difficulty: "medium",
            status: MatchStatus::BaselineDebt,
            allow_id: Some("allow-baseline".to_string()),
            finding_index: None,
            path: Some("src/lib.rs".to_string()),
            source_package: None,
            message: "baseline debt".to_string(),
            suggested_actions: Vec::new(),
            proof_commands: Vec::new(),
        };
        let mut broad = baseline.clone();
        broad.id = "work-broad-scope-0002".to_string();
        broad.kind = "broad_scope".to_string();
        broad.classification = Some("reviewed_exception".to_string());
        broad.status = MatchStatus::Matched;
        broad.allow_id = Some("allow-broad".to_string());
        let mut stale = broad.clone();
        stale.id = "work-stale-0003".to_string();
        stale.kind = "stale_allow".to_string();
        stale.status = MatchStatus::Stale;
        stale.allow_id = Some("allow-stale".to_string());

        let baseline_filtered = filter_work_items(
            vec![baseline.clone(), broad.clone(), stale.clone()],
            WorklistFilters {
                baseline_debt: true,
                ..WorklistFilters::default()
            },
        );
        let broad_filtered = filter_work_items(
            vec![baseline, broad, stale],
            WorklistFilters {
                broad_scope: true,
                ..WorklistFilters::default()
            },
        );

        assert_eq!(baseline_filtered.len(), 1);
        assert_eq!(
            baseline_filtered[0].allow_id.as_deref(),
            Some("allow-baseline")
        );
        assert_eq!(broad_filtered.len(), 1);
        assert_eq!(broad_filtered[0].allow_id.as_deref(), Some("allow-broad"));
    }

    #[test]
    fn worklist_filters_by_missing_evidence() {
        let missing = WorkItem {
            id: "work-missing-evidence-0001".to_string(),
            kind: "missing_evidence".to_string(),
            exception_kind: Some("unsafe".to_string()),
            family: Some("unsafe_block".to_string()),
            owner: Some("runtime".to_string()),
            classification: Some("reviewed_unsafe_boundary".to_string()),
            reason: Some("fixture".to_string()),
            created: None,
            review_after: None,
            expires: None,
            evidence_count: Some(0),
            risk: "high",
            difficulty: "small",
            status: MatchStatus::EvidenceMissing,
            allow_id: Some("allow-missing".to_string()),
            finding_index: None,
            path: Some("src/lib.rs".to_string()),
            source_package: None,
            message: "allow-missing requires evidence".to_string(),
            suggested_actions: Vec::new(),
            proof_commands: Vec::new(),
        };
        let mut evidenced = missing.clone();
        evidenced.id = "work-review-due-0002".to_string();
        evidenced.kind = "review_due".to_string();
        evidenced.evidence_count = Some(2);
        evidenced.status = MatchStatus::ReviewDue;
        evidenced.allow_id = Some("allow-evidenced".to_string());
        let mut new_finding = missing.clone();
        new_finding.id = "work-new-unreceipted-finding-0003".to_string();
        new_finding.kind = "new_unreceipted_finding".to_string();
        new_finding.evidence_count = None;
        new_finding.status = MatchStatus::New;
        new_finding.allow_id = None;

        let filtered = filter_work_items(
            vec![missing, evidenced, new_finding],
            WorklistFilters {
                missing_evidence: true,
                ..WorklistFilters::default()
            },
        );

        assert_eq!(filtered.len(), 1);
        let item = filtered
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected missing evidence work item"));
        assert_eq!(item.allow_id.as_deref(), Some("allow-missing"));
        assert_eq!(item.evidence_count, Some(0));
    }

    #[test]
    fn worklist_filters_by_path_prefix() {
        let cfg = AllowConfig::empty();
        let findings = vec![
            test_finding(
                FindingKind::Panic,
                Some("unwrap"),
                "crates/allow-core/src/lib.rs",
                "method_call",
            ),
            test_finding(
                FindingKind::Panic,
                Some("expect"),
                "crates/allow-rust/src/lib.rs",
                "method_call",
            ),
        ];
        let outcomes = vec![
            test_outcome(MatchStatus::New, None, Some(0), "unreceipted unwrap"),
            test_outcome(MatchStatus::New, None, Some(1), "unreceipted expect"),
        ];

        let items = work_items_from_outcomes(&cfg, &findings, &outcomes);
        let filtered = filter_work_items(
            items,
            WorklistFilters {
                path: Some(r"crates\allow-core"),
                ..WorklistFilters::default()
            },
        );

        assert_eq!(filtered.len(), 1);
        let item = filtered
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
        assert_eq!(item.path.as_deref(), Some("crates/allow-core/src/lib.rs"));
    }

    #[test]
    fn worklist_filters_by_source_package() {
        let cfg = AllowConfig::empty();
        let mut first = test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "crates/allow-core/src/lib.rs",
            "method_call",
        );
        first.identity.crate_name = Some("allow-core".to_string());
        let mut second = test_finding(
            FindingKind::Panic,
            Some("expect"),
            "crates/allow-rust/src/lib.rs",
            "method_call",
        );
        second.identity.crate_name = Some("allow-rust".to_string());
        let findings = vec![first, second];
        let outcomes = vec![
            test_outcome(MatchStatus::New, None, Some(0), "unreceipted unwrap"),
            test_outcome(MatchStatus::New, None, Some(1), "unreceipted expect"),
        ];

        let items = work_items_from_outcomes(&cfg, &findings, &outcomes);
        let filtered = filter_work_items(
            items,
            WorklistFilters {
                source_package: Some("allow-core"),
                ..WorklistFilters::default()
            },
        );

        assert_eq!(filtered.len(), 1);
        let item = filtered
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
        assert_eq!(item.source_package.as_deref(), Some("allow-core"));
        assert_eq!(item.path.as_deref(), Some("crates/allow-core/src/lib.rs"));
    }

    #[test]
    fn worklist_filters_by_family() {
        let cfg = AllowConfig::empty();
        let findings = vec![
            test_finding(
                FindingKind::Panic,
                Some("unwrap"),
                "src/unwrap.rs",
                "method_call",
            ),
            test_finding(
                FindingKind::Panic,
                Some("expect"),
                "src/expect.rs",
                "method_call",
            ),
        ];
        let outcomes = vec![
            test_outcome(MatchStatus::New, None, Some(0), "unreceipted unwrap"),
            test_outcome(MatchStatus::New, None, Some(1), "unreceipted expect"),
        ];

        let items = work_items_from_outcomes(&cfg, &findings, &outcomes);
        let filtered = filter_work_items(
            items,
            WorklistFilters {
                family: Some("unwrap"),
                ..WorklistFilters::default()
            },
        );

        assert_eq!(filtered.len(), 1);
        let item = filtered
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected filtered work item"));
        assert_eq!(item.family.as_deref(), Some("unwrap"));
        assert_eq!(item.path.as_deref(), Some("src/unwrap.rs"));
    }

    #[test]
    fn worklist_sort_prioritizes_risk_then_difficulty() {
        let mut cfg = AllowConfig::empty();
        cfg.allow
            .push(test_entry("allow-stale", FindingKind::NonRustFile));
        let findings = vec![
            test_finding(
                FindingKind::Panic,
                Some("unwrap"),
                "src/panic.rs",
                "method_call",
            ),
            test_finding(
                FindingKind::PolicyException,
                Some("process_spawn"),
                ".github/workflows/ci.yml",
                "process_spawn",
            ),
            test_finding(
                FindingKind::NonRustFile,
                Some("shell_script"),
                "scripts/new.sh",
                "tracked_file",
            ),
        ];
        let outcomes = vec![
            test_outcome(MatchStatus::New, None, Some(0), "unreceipted panic.unwrap"),
            test_outcome(
                MatchStatus::New,
                None,
                Some(1),
                "unreceipted process policy exception",
            ),
            test_outcome(MatchStatus::New, None, Some(2), "unreceipted shell script"),
            test_outcome(
                MatchStatus::Stale,
                Some("allow-stale"),
                None,
                "allow-stale is stale",
            ),
        ];

        let mut items = work_items_from_outcomes(&cfg, &findings, &outcomes);
        sort_work_items(&mut items);
        renumber_work_items(&mut items);

        assert_eq!(items[0].risk, "high");
        assert_eq!(items[0].family.as_deref(), Some("process_spawn"));
        assert_eq!(items[0].id, "work-new-unreceipted-finding-0001");
        assert_eq!(items[1].risk, "medium");
        assert_eq!(items[1].difficulty, "small");
        assert_eq!(items[1].family.as_deref(), Some("shell_script"));
        assert_eq!(items[1].id, "work-new-unreceipted-finding-0002");
        assert_eq!(items[2].risk, "medium");
        assert_eq!(items[2].difficulty, "medium");
        assert_eq!(items[2].family.as_deref(), Some("unwrap"));
        assert_eq!(items[2].id, "work-new-unreceipted-finding-0003");
        assert_eq!(items[3].risk, "low");
        assert_eq!(items[3].kind, "stale_allow");
        assert_eq!(items[3].id, "work-stale-allow-0004");
    }

    #[test]
    fn worklist_items_report_occurrence_limit_overrun() {
        let mut cfg = AllowConfig::empty();
        cfg.allow
            .push(test_entry("allow-file", FindingKind::NonRustFile));
        let finding = test_finding(
            FindingKind::NonRustFile,
            None,
            "tracked.file",
            "tracked_file",
        );
        let outcomes = vec![test_outcome(
            MatchStatus::New,
            Some("allow-file"),
            Some(0),
            "allow-file occurrence_limit exceeded at tracked.file:1:1",
        )];

        let items = work_items_from_outcomes(&cfg, &[finding], &outcomes);

        let item = items
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
        assert_eq!(item.kind, "occurrence_limit_exceeded");
        assert_eq!(item.exception_kind.as_deref(), Some("non_rust_file"));
        assert_eq!(item.risk, "medium");
        assert!(
            item.suggested_actions
                .iter()
                .any(|action| action.contains("baseline count"))
        );
    }

    #[test]
    fn worklist_items_report_broad_scope_advisories() {
        let mut cfg = AllowConfig::empty();
        let mut entry = test_entry("allow-scripts", FindingKind::NonRustFile);
        entry.path = None;
        entry.glob = Some("scripts/**".to_string());
        entry.selector.glob = Some("scripts/**".to_string());
        entry.family = Some("shell_script".to_string());
        cfg.allow.push(entry);
        let outcomes = vec![MatchOutcome {
            status: MatchStatus::Matched,
            allow_id: Some("allow-scripts".to_string()),
            finding_index: Some(0),
            message: "matched".to_string(),
            score: 100,
        }];

        let items = work_items_from_policy_advisories(&cfg, &[], &outcomes, 1);
        let json = render_worklist_json_with_context(&items, WorklistContext::default());
        let human = render_worklist_human_with_context(&items, WorklistContext::default());

        let item = items
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
        assert_eq!(item.kind, "broad_scope");
        assert_eq!(item.status, MatchStatus::Matched);
        assert_eq!(item.risk, "medium");
        assert_eq!(item.difficulty, "small");
        assert_eq!(item.allow_id.as_deref(), Some("allow-scripts"));
        assert_eq!(item.path.as_deref(), Some("scripts/**"));
        assert_eq!(item.exception_kind.as_deref(), Some("non_rust_file"));
        assert_eq!(item.family.as_deref(), Some("shell_script"));
        assert!(
            item.suggested_actions
                .iter()
                .any(|action| action.contains("narrower glob"))
        );
        assert!(
            item.proof_commands
                .iter()
                .any(|command| command == "cargo-allow worklist --broad-scope --format json")
        );
        assert!(json.contains("\"kind\": \"broad_scope\""));
        assert!(json.contains("\"status\": \"matched\""));
        assert!(human.contains("proof: cargo-allow worklist --broad-scope --format json"));
        assert!(human.contains("exception: non_rust_file.shell_script"));
    }

    #[test]
    fn worklist_items_report_matched_baseline_debt_advisories() {
        let mut cfg = AllowConfig::empty();
        let mut entry = test_entry("allow-baseline", FindingKind::Panic);
        entry.classification = "baseline_debt".to_string();
        entry.family = Some("unwrap".to_string());
        cfg.allow.push(entry);
        let mut finding = test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "crates/parser/src/lib.rs",
            "method_call",
        );
        finding.identity.crate_name = Some("parser".to_string());
        let outcomes = vec![MatchOutcome {
            status: MatchStatus::Matched,
            allow_id: Some("allow-baseline".to_string()),
            finding_index: Some(0),
            message: "matched".to_string(),
            score: 100,
        }];

        let items = work_items_from_policy_advisories(&cfg, &[finding], &outcomes, 1);
        let json = render_worklist_json_with_context(&items, WorklistContext::default());
        let human = render_worklist_human_with_context(&items, WorklistContext::default());

        let item = items
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
        assert_eq!(item.kind, "baseline_debt");
        assert_eq!(item.status, MatchStatus::BaselineDebt);
        assert_eq!(item.risk, "medium");
        assert_eq!(item.difficulty, "medium");
        assert_eq!(item.allow_id.as_deref(), Some("allow-baseline"));
        assert_eq!(item.finding_index, Some(0));
        assert_eq!(item.exception_kind.as_deref(), Some("panic"));
        assert_eq!(item.family.as_deref(), Some("unwrap"));
        assert_eq!(item.source_package.as_deref(), Some("parser"));
        assert!(item.message.contains("still needs human review"));
        assert!(
            item.suggested_actions
                .iter()
                .any(|action| action.contains("reviewed allow entry"))
        );
        assert!(
            item.proof_commands
                .iter()
                .any(|command| command == "cargo-allow worklist --baseline-debt --format json")
        );
        assert!(json.contains("\"kind\": \"baseline_debt\""));
        assert!(json.contains("\"status\": \"baseline_debt\""));
        assert!(human.contains("proof: cargo-allow worklist --baseline-debt --format json"));
        assert!(human.contains("source package: parser"));
        assert!(human.contains("exception: panic.unwrap"));
    }

    #[test]
    fn worklist_policy_advisories_ignore_exact_selector_globs() {
        let mut cfg = AllowConfig::empty();
        let mut entry = test_entry("allow-doc", FindingKind::NonRustFile);
        entry.selector.glob = Some("docs/README.md".to_string());
        cfg.allow.push(entry);
        let outcomes = vec![MatchOutcome {
            status: MatchStatus::Matched,
            allow_id: Some("allow-doc".to_string()),
            finding_index: Some(0),
            message: "matched".to_string(),
            score: 100,
        }];

        let items = work_items_from_policy_advisories(&cfg, &[], &outcomes, 1);

        assert!(items.is_empty());
    }

    #[test]
    fn worklist_policy_advisories_ignore_unmatched_broad_scopes() {
        let mut cfg = AllowConfig::empty();
        let mut entry = test_entry("allow-scripts", FindingKind::NonRustFile);
        entry.glob = Some("scripts/**".to_string());
        cfg.allow.push(entry);
        let outcomes = vec![MatchOutcome {
            status: MatchStatus::Stale,
            allow_id: Some("allow-scripts".to_string()),
            finding_index: None,
            message: "stale".to_string(),
            score: 0,
        }];

        let items = work_items_from_policy_advisories(&cfg, &[], &outcomes, 1);

        assert!(items.is_empty());
    }

    #[test]
    fn worklist_items_report_broken_evidence_links() {
        let root = migrate_fixture_dir();
        let mut cfg = AllowConfig::empty();
        let mut entry = test_entry("allow-unsafe", FindingKind::Unsafe);
        entry.evidence = vec!["doc:docs/missing.md".to_string()];
        cfg.allow.push(entry);

        let items = work_items_from_evidence_diagnostics(&root, &cfg, 1);
        let json = render_worklist_json_with_context(&items, WorklistContext::default());

        let item = items
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected one work item"));
        assert_eq!(item.kind, "broken_evidence_link");
        assert_eq!(item.exception_kind.as_deref(), Some("unsafe"));
        assert_eq!(item.risk, "high");
        assert_eq!(item.difficulty, "small");
        assert_eq!(item.status, MatchStatus::EvidenceMissing);
        assert_eq!(item.allow_id.as_deref(), Some("allow-unsafe"));
        assert_eq!(item.path.as_deref(), Some("docs/missing.md"));
        assert!(item.message.contains("local evidence file is missing"));
        assert!(json.contains("\"kind\": \"broken_evidence_link\""));
        assert!(json.contains("\"exception_kind\": \"unsafe\""));
        assert!(json.contains("\"cargo-allow explain allow-unsafe\""));
        fs::remove_dir_all(root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture dir: {err}")));
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
    fn clap_parses_source_tree_root_for_check() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "check",
            "--root",
            "fixtures/source-snapshot",
        ]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse --root: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Check(CheckArgs {
                root: RootArgs { root: Some(root) },
                ..
            })) if root == Path::new("fixtures/source-snapshot")
        ));
    }

    #[test]
    fn clap_parses_non_rust_compat_check() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "check",
            "--compat",
            "--kind",
            "non-rust",
        ]))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("CLI should parse compat check: {err}"))
        });

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Check(CheckArgs {
                compat: true,
                kind: Some(kind),
                ..
            })) if kind == "non-rust"
        ));
    }

    #[test]
    fn clap_parses_generated_compat_check() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "check",
            "--compat",
            "--kind",
            "generated",
        ]))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("CLI should parse generated compat check: {err}"))
        });

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Check(CheckArgs {
                compat: true,
                kind: Some(kind),
                ..
            })) if kind == "generated"
        ));
    }

    #[test]
    fn clap_parses_panic_compat_check() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "check",
            "--compat",
            "--kind",
            "panic",
        ]))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("CLI should parse panic compat check: {err}"))
        });

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Check(CheckArgs {
                compat: true,
                kind: Some(kind),
                ..
            })) if kind == "panic"
        ));
    }

    #[test]
    fn clap_parses_executable_compat_check() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "check",
            "--compat",
            "--kind",
            "executable",
        ]))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("CLI should parse executable compat check: {err}"))
        });

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Check(CheckArgs {
                compat: true,
                kind: Some(kind),
                ..
            })) if kind == "executable"
        ));
    }

    #[test]
    fn clap_parses_workflow_compat_check() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "check",
            "--compat",
            "--kind",
            "workflow",
        ]))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("CLI should parse workflow compat check: {err}"))
        });

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Check(CheckArgs {
                compat: true,
                kind: Some(kind),
                ..
            })) if kind == "workflow"
        ));
    }

    #[test]
    fn clap_parses_dependency_surface_compat_check() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "check",
            "--compat",
            "--kind",
            "dependency-surface",
        ]))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!(
                "CLI should parse dependency-surface compat check: {err}"
            ))
        });

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Check(CheckArgs {
                compat: true,
                kind: Some(kind),
                ..
            })) if kind == "dependency-surface"
        ));
    }

    #[test]
    fn clap_parses_process_compat_check() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "check",
            "--compat",
            "--kind",
            "process",
        ]))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("CLI should parse process compat check: {err}"))
        });

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Check(CheckArgs {
                compat: true,
                kind: Some(kind),
                ..
            })) if kind == "process"
        ));
    }

    #[test]
    fn clap_parses_network_compat_check() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "check",
            "--compat",
            "--kind",
            "network",
        ]))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("CLI should parse network compat check: {err}"))
        });

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Check(CheckArgs {
                compat: true,
                kind: Some(kind),
                ..
            })) if kind == "network"
        ));
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
                ..
            })) if dir == Path::new("policy") && out == Path::new("target/allow.toml")
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
        })
        .unwrap_or_else(|err| std::panic::panic_any(format!("repo-policy migrate: {err}")));

        let rendered = fs::read_to_string(&out)
            .unwrap_or_else(|err| std::panic::panic_any(format!("read migrated policy: {err}")));
        assert!(rendered.contains("process_spawn"));
        assert!(rendered.contains("network_destination"));
    }

    #[test]
    fn canonical_companion_findings_match_migrated_policy_entries() {
        let dir = migrate_fixture_dir();
        let workflows_dir = dir.join(".github").join("workflows");
        fs::create_dir_all(&workflows_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("workflow dir: {err}")));
        fs::write(
            dir.join(".gitattributes"),
            "generated/schema.json linguist-generated=true\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("gitattributes write: {err}")));
        fs::write(
            workflows_dir.join("ci.yml"),
            "steps:\n  - uses: actions/checkout@v4\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("workflow write: {err}")));

        let mut cfg = AllowConfig::empty();
        cfg.allow.push(companion_entry(
            "generated-schema",
            FindingKind::GeneratedCode,
            "generated_code",
            "generated/schema.json",
            "tracked_file",
            "generated/schema.json",
            Some("json"),
        ));
        cfg.allow.push(companion_entry(
            "workflow-file-ci",
            FindingKind::PolicyException,
            "github_workflow",
            ".github/workflows/ci.yml",
            "github_workflow",
            ".github/workflows/ci.yml",
            None,
        ));
        cfg.allow.push(companion_entry(
            "workflow-action-ci-checkout",
            FindingKind::PolicyException,
            "workflow_external_action",
            ".github/workflows/ci.yml",
            "github_action_uses",
            ".github/workflows/ci.yml uses actions/checkout@v4",
            Some("action:actions/checkout@v4"),
        ));
        cfg.allow.push(companion_entry(
            "proc-cargo-test",
            FindingKind::PolicyException,
            "process_spawn",
            ".github/workflows/ci.yml",
            "process_spawn",
            "cargo test",
            Some("process:cargo test"),
        ));
        cfg.allow.push(companion_entry(
            "net-crates-io",
            FindingKind::PolicyException,
            "network_destination",
            "policy/network-allowlist.toml",
            "network_destination",
            "crates.io lane build",
            Some("network:crates.io:auth:false:lane:build"),
        ));

        let findings = canonical_companion_findings(&dir, &cfg).unwrap_or_else(|err| {
            std::panic::panic_any(format!("canonical companion findings: {err}"))
        });
        let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);

        assert_eq!(findings.len(), 5);
        assert!(
            outcomes
                .iter()
                .all(|outcome| outcome.status == MatchStatus::Matched),
            "expected every migrated companion entry to match current canonical findings: {outcomes:?}"
        );
    }

    #[test]
    fn panic_compat_loads_no_panic_baseline_and_scans_source_tree_findings() {
        let dir = migrate_fixture_dir();
        let policy_dir = dir.join("policy");
        let src_dir = dir.join("src");
        fs::create_dir_all(&policy_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
        fs::create_dir_all(&src_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
        let snippet = "let value = maybe.unwrap();";
        fs::write(
            policy_dir.join("no-panic-baseline.toml"),
            format!(
                r#"schema_version = 1
policy = "no-panic-baseline"
owner = "EffortlessMetrics"
status = "advisory"

[[entry]]
path = "src/lib.rs"
family = "unwrap"
selector_kind = "method-call"
selector_callee = "Option/Result::unwrap"
snippet = "{snippet}"
count = 1
"#
            ),
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("no-panic policy write: {err}")));
        fs::write(
            src_dir.join("lib.rs"),
            format!("fn load(maybe: Option<u8>) {{\n    {snippet}\n}}\n"),
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("rust fixture write: {err}")));

        let (_root, cfg, findings, inventory_facts) =
            load_compat_world(Some(&dir), None, Some("panic"), false).unwrap_or_else(|err| {
                std::panic::panic_any(format!("panic compat world loads: {err}"))
            });
        let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);

        assert_eq!(inventory_facts.source, InventorySource::FilesystemFallback);
        assert!(inventory_facts.files_scanned.is_some());
        assert!(
            cfg.allow
                .iter()
                .any(|entry| entry.classification == "baseline_debt"
                    && entry.occurrence_limit == Some(1))
        );
        assert!(findings.iter().any(|finding| {
            finding.kind == FindingKind::Panic && finding.family.as_deref() == Some("unwrap")
        }));
        assert!(
            outcomes
                .iter()
                .any(|outcome| outcome.status == MatchStatus::Matched)
        );
    }

    #[test]
    fn no_panic_allowlist_compat_loads_policy_and_scans_panic_findings() {
        let dir = migrate_fixture_dir();
        let policy_dir = dir.join("policy");
        let src_dir = dir.join("src");
        fs::create_dir_all(&policy_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
        fs::create_dir_all(&src_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
        fs::write(
            policy_dir.join("no-panic-allowlist.toml"),
            r#"schema_version = 1
policy = "no-panic-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "no-panic-unwrap"
path = "src/lib.rs"
family = "unwrap"
owner = "parser"
classification = "reviewed_panic_exception"
reason = "Parser validates the optional value."
created = "2026-05-09"
review_after = "2026-09-09"

[allow.selector]
kind = "method-call"
callee = "Option/Result::unwrap"
"#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("no-panic policy write: {err}")));
        fs::write(
            src_dir.join("lib.rs"),
            "fn load(maybe: Option<u8>) {\n    let value = maybe.unwrap();\n}\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("rust fixture write: {err}")));

        let (_root, cfg, findings, inventory_facts) =
            load_compat_world(Some(&dir), None, Some("no-panic-allowlist"), false).unwrap_or_else(
                |err| std::panic::panic_any(format!("no-panic allowlist world loads: {err}")),
            );
        let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);

        assert_eq!(inventory_facts.source, InventorySource::FilesystemFallback);
        assert!(inventory_facts.files_scanned.is_some());
        assert!(cfg.allow.iter().any(|entry| {
            entry.kind == FindingKind::Panic && entry.selector.callee.as_deref() == Some("unwrap")
        }));
        assert!(findings.iter().any(|finding| {
            finding.kind == FindingKind::Panic && finding.family.as_deref() == Some("unwrap")
        }));
        assert!(
            outcomes
                .iter()
                .any(|outcome| outcome.status == MatchStatus::Matched)
        );
    }

    #[test]
    fn clippy_compat_loads_legacy_policy_and_scans_lint_findings() {
        let dir = migrate_fixture_dir();
        let policy_dir = dir.join("policy");
        let src_dir = dir.join("src");
        fs::create_dir_all(&policy_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
        fs::create_dir_all(&src_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
        fs::write(
            policy_dir.join("clippy-exceptions.toml"),
            r#"schema_version = 1
policy = "clippy-exceptions"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "clippy-unwrap-policy"
path = "src/lib.rs"
lint = "clippy::unwrap_used"
family = "expect"
owner = "lint"
classification = "reviewed_lint_exception"
reason = "Fixture keeps an explicit lint suppression linked to policy."
policy_id = "clippy-unwrap-policy"
created = "2026-05-09"
review_after = "2026-09-09"
"#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("clippy policy write: {err}")));
        fs::write(
            src_dir.join("lib.rs"),
            r#"#[expect(clippy::unwrap_used, reason = "policy:clippy-unwrap-policy: fixture")]
fn load() {}
"#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("rust fixture write: {err}")));

        let (_root, cfg, findings, inventory_facts) =
            load_compat_world(Some(&dir), None, Some("lint-exception"), false).unwrap_or_else(
                |err| std::panic::panic_any(format!("clippy compat world loads: {err}")),
            );
        let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);

        assert_eq!(inventory_facts.source, InventorySource::FilesystemFallback);
        assert!(inventory_facts.files_scanned.is_some());
        assert!(cfg.allow.iter().any(|entry| {
            entry.kind == FindingKind::LintException
                && entry.selector.lint.as_deref() == Some("clippy::unwrap_used")
        }));
        assert!(findings.iter().any(|finding| {
            finding.kind == FindingKind::LintException
                && finding.family.as_deref() == Some("expect_attribute")
        }));
        assert!(
            outcomes
                .iter()
                .any(|outcome| outcome.status == MatchStatus::Matched)
        );
    }

    #[test]
    fn unsafe_compat_loads_legacy_policy_and_scans_unsafe_findings() {
        let dir = migrate_fixture_dir();
        let policy_dir = dir.join("policy");
        let src_dir = dir.join("src");
        fs::create_dir_all(&policy_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
        fs::create_dir_all(&src_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
        fs::write(
            policy_dir.join("unsafe-allowlist.toml"),
            r#"schema_version = 1
policy = "unsafe-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "unsafe-read"
path = "src/lib.rs"
family = "unsafe_block"
owner = "runtime"
classification = "reviewed_unsafe_boundary"
reason = "Caller validates pointer before read."
evidence = ["unsafe-review:docs/evidence/unsafe/read.json"]
created = "2026-05-09"
review_after = "2026-09-09"

[allow.selector]
kind = "unsafe-block"
container = "read"
"#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("unsafe policy write: {err}")));
        fs::write(
            src_dir.join("lib.rs"),
            "fn read(ptr: *const u8) -> u8 {\n    // SAFETY: fixture validates the policy match path.\n    unsafe { core::ptr::read(ptr) }\n}\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("rust fixture write: {err}")));

        let (_root, cfg, findings, inventory_facts) =
            load_compat_world(Some(&dir), None, Some("unsafe"), false).unwrap_or_else(|err| {
                std::panic::panic_any(format!("unsafe compat world loads: {err}"))
            });
        let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);

        assert_eq!(inventory_facts.source, InventorySource::FilesystemFallback);
        assert!(inventory_facts.files_scanned.is_some());
        assert!(cfg.allow.iter().any(|entry| {
            entry.kind == FindingKind::Unsafe
                && entry.selector.ast_kind.as_deref() == Some("unsafe_block")
        }));
        assert!(findings.iter().any(|finding| {
            finding.kind == FindingKind::Unsafe && finding.family.as_deref() == Some("unsafe_block")
        }));
        assert!(
            outcomes
                .iter()
                .any(|outcome| outcome.status == MatchStatus::Matched)
        );
    }

    #[test]
    fn dependency_surface_compat_reports_git_source_without_inventory_count() {
        let dir = migrate_fixture_dir();
        let policy_dir = dir.join("policy");
        let crate_dir = dir.join("crates").join("core");
        fs::create_dir_all(&policy_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
        fs::create_dir_all(&crate_dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("crate dir: {err}")));
        fs::write(
            policy_dir.join("dependency-surface-allowlist.toml"),
            dependency_surface_policy_fixture_text(),
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("dependency policy write: {err}")));
        fs::write(
            dir.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/core\"]\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("workspace manifest: {err}")));
        fs::write(crate_dir.join("Cargo.toml"), "[package]\nname = \"core\"\n")
            .unwrap_or_else(|err| std::panic::panic_any(format!("crate manifest: {err}")));
        run_git_for_test(&dir, &["init"]);
        run_git_for_test(&dir, &["add", "Cargo.toml", "crates/core/Cargo.toml"]);

        let (_root, _cfg, findings, inventory_facts) =
            load_compat_world(Some(&dir), None, Some("dependency-surface"), false).unwrap_or_else(
                |err| std::panic::panic_any(format!("dependency compat world loads: {err}")),
            );

        assert_eq!(inventory_facts.source, InventorySource::GitTracked);
        assert_eq!(inventory_facts.files_scanned, None);
        assert_eq!(findings.len(), 2);
    }

    #[test]
    fn extend_unique_findings_deduplicates_generated_companion_inventory() {
        let mut generated = test_finding(
            FindingKind::GeneratedCode,
            Some("generated_code"),
            "generated/schema.json",
            "tracked_file",
        );
        generated.identity.symbol = Some("generated/schema.json".to_string());
        generated.identity.target_fingerprint = Some("json".to_string());
        let duplicate = generated.clone();
        let mut existing = vec![generated];
        let distinct = test_finding(
            FindingKind::GeneratedCode,
            Some("generated_code"),
            "generated/other.json",
            "tracked_file",
        );

        extend_unique_findings(&mut existing, vec![duplicate, distinct]);

        assert_eq!(existing.len(), 2);
        assert!(
            existing
                .iter()
                .any(|finding| normalize_path(&finding.path) == "generated/other.json")
        );
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

    #[test]
    fn report_config_filters_executable_family() {
        let mut cfg = AllowConfig::empty();
        let mut executable = test_entry("allow-exec", FindingKind::PolicyException);
        executable.family = Some("executable_file".to_string());
        let mut other = test_entry("allow-other-policy", FindingKind::PolicyException);
        other.family = Some("workflow_permission".to_string());
        cfg.allow.push(executable);
        cfg.allow.push(other);

        let filtered = report_config(&cfg, Some("executable")).unwrap_or_else(|err| {
            std::panic::panic_any(format!("executable filter should parse: {err}"))
        });

        assert_eq!(filtered.allow.len(), 1);
        let entry = filtered
            .allow
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected executable entry"));
        assert_eq!(entry.id, "allow-exec");
    }

    #[test]
    fn report_config_filters_workflow_families() {
        let mut cfg = AllowConfig::empty();
        let mut workflow = test_entry("allow-workflow", FindingKind::PolicyException);
        workflow.family = Some("github_workflow".to_string());
        let mut action = test_entry("allow-workflow-action", FindingKind::PolicyException);
        action.family = Some("workflow_external_action".to_string());
        let mut other = test_entry("allow-other-policy", FindingKind::PolicyException);
        other.family = Some("executable_file".to_string());
        cfg.allow.push(workflow);
        cfg.allow.push(action);
        cfg.allow.push(other);

        let filtered = report_config(&cfg, Some("workflow")).unwrap_or_else(|err| {
            std::panic::panic_any(format!("workflow filter should parse: {err}"))
        });

        assert_eq!(filtered.allow.len(), 2);
        assert!(
            filtered
                .allow
                .iter()
                .any(|entry| entry.id == "allow-workflow")
        );
        assert!(
            filtered
                .allow
                .iter()
                .any(|entry| entry.id == "allow-workflow-action")
        );
    }

    #[test]
    fn report_config_filters_dependency_surface_family() {
        let mut cfg = AllowConfig::empty();
        let mut dependency = test_entry("allow-dep", FindingKind::PolicyException);
        dependency.family = Some("dependency_surface".to_string());
        let mut other = test_entry("allow-other-policy", FindingKind::PolicyException);
        other.family = Some("workflow_external_action".to_string());
        cfg.allow.push(dependency);
        cfg.allow.push(other);

        let filtered = report_config(&cfg, Some("dependency-surface")).unwrap_or_else(|err| {
            std::panic::panic_any(format!("dependency-surface filter should parse: {err}"))
        });

        assert_eq!(filtered.allow.len(), 1);
        let entry = filtered
            .allow
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected dependency entry"));
        assert_eq!(entry.id, "allow-dep");
    }

    #[test]
    fn report_config_filters_process_family() {
        let mut cfg = AllowConfig::empty();
        let mut process = test_entry("allow-process", FindingKind::PolicyException);
        process.family = Some("process_spawn".to_string());
        let mut other = test_entry("allow-other-policy", FindingKind::PolicyException);
        other.family = Some("dependency_surface".to_string());
        cfg.allow.push(process);
        cfg.allow.push(other);

        let filtered = report_config(&cfg, Some("process")).unwrap_or_else(|err| {
            std::panic::panic_any(format!("process filter should parse: {err}"))
        });

        assert_eq!(filtered.allow.len(), 1);
        let entry = filtered
            .allow
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected process entry"));
        assert_eq!(entry.id, "allow-process");
    }

    #[test]
    fn report_config_filters_network_family() {
        let mut cfg = AllowConfig::empty();
        let mut network = test_entry("allow-network", FindingKind::PolicyException);
        network.family = Some("network_destination".to_string());
        let mut other = test_entry("allow-other-policy", FindingKind::PolicyException);
        other.family = Some("process_spawn".to_string());
        cfg.allow.push(network);
        cfg.allow.push(other);

        let filtered = report_config(&cfg, Some("network")).unwrap_or_else(|err| {
            std::panic::panic_any(format!("network filter should parse: {err}"))
        });

        assert_eq!(filtered.allow.len(), 1);
        let entry = filtered
            .allow
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected network entry"));
        assert_eq!(entry.id, "allow-network");
    }

    fn argv(items: Vec<&str>) -> Vec<String> {
        items.into_iter().map(String::from).collect()
    }

    static NEXT_MIGRATE_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    fn migrate_fixture_dir() -> PathBuf {
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

    fn dependency_surface_policy_fixture_text() -> &'static str {
        r#"schema_version = 1
policy = "dependency-surface-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "dep-workspace-cargo-toml"
path = "Cargo.toml"
surface = "workspace_manifest"
owner = "release"
reason = "Workspace dependency block."
created = "2026-05-09"
expires = "permanent"

[[allow]]
id = "dep-crate-cargo-toml"
path = "crates/*/Cargo.toml"
surface = "crate_manifest"
owner = "release"
reason = "Per-crate manifests."
broad_glob_reason = "Per-crate enumeration would duplicate the workspace member list."
created = "2026-05-09"
expires = "permanent"
"#
    }

    fn run_git_for_test(root: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .status()
            .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
        assert!(status.success(), "git {args:?} failed with {status}");
    }

    fn parse_json_artifact(
        name: &str,
        json: &str,
        expected_schema_id: &str,
        expected_command: &str,
    ) -> Value {
        let value: Value = serde_json::from_str(json).unwrap_or_else(|err| {
            std::panic::panic_any(format!(
                "{name} artifact should parse as JSON: {err}\n{json}"
            ))
        });
        assert_eq!(
            value.get("schema_version").and_then(Value::as_u64),
            Some(1),
            "{name} schema_version"
        );
        assert_eq!(
            value.get("schema_id").and_then(Value::as_str),
            Some(expected_schema_id),
            "{name} schema_id"
        );
        assert_eq!(
            value.get("command").and_then(Value::as_str),
            Some(expected_command),
            "{name} command"
        );
        assert_json_array_contains(&value, "claim_boundary", "source_tree_inventory", name);
        assert_json_array_contains(
            &value,
            "scanner_limitations",
            "cargo_metadata_not_invoked",
            name,
        );
        assert_json_array_contains(
            &value,
            "scanner_limitations",
            "repository_code_not_executed",
            name,
        );
        assert_eq!(
            value.pointer("/inventory/scope").and_then(Value::as_str),
            Some("source_tree"),
            "{name} inventory scope"
        );
        assert_eq!(
            value.pointer("/inventory/scanner").and_then(Value::as_str),
            Some("source_syntax"),
            "{name} inventory scanner"
        );
        value
    }

    fn assert_json_array_contains(value: &Value, field: &str, expected: &str, artifact: &str) {
        let Some(items) = value.get(field).and_then(Value::as_array) else {
            std::panic::panic_any(format!("{artifact} {field} should be an array"));
        };
        assert!(
            items.iter().any(|item| item.as_str() == Some(expected)),
            "{artifact} {field} should contain {expected}"
        );
    }

    fn assert_inventory_contract(
        name: &str,
        value: &Value,
        expected_source: &str,
        expected_root: Option<&str>,
        expected_files: Option<u64>,
    ) {
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
            "{name} inventory files_scanned"
        );
    }

    fn empty_list_filters<'a>() -> ListFilters<'a> {
        ListFilters {
            kind: None,
            family: None,
            owner: None,
            classification: None,
            path: None,
            source_package: None,
            status: None,
            expired: false,
            review_due: false,
            stale: false,
            baseline_debt: false,
            broad_scope: false,
            missing_evidence: false,
        }
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
            occurrence_limit: None,
            lifecycle: Lifecycle::empty(),
            selector: Selector {
                ast_kind: Some("tracked_file".to_string()),
                ..Selector::default()
            },
            last_seen: None,
        }
    }

    fn non_rust_prune_fixture_entry(id: &str, path: &str) -> AllowEntry {
        let mut entry = test_entry(id, FindingKind::NonRustFile);
        entry.family = Some("documentation".to_string());
        entry.path = Some(PathBuf::from(path));
        entry.lifecycle.review_after = Some("2026-11-01".to_string());
        entry
    }

    fn companion_entry(
        id: &str,
        kind: FindingKind,
        family: &str,
        path: &str,
        ast_kind: &str,
        symbol: &str,
        target_fingerprint: Option<&str>,
    ) -> AllowEntry {
        AllowEntry {
            id: id.to_string(),
            kind,
            family: Some(family.to_string()),
            path: Some(PathBuf::from(path)),
            glob: None,
            owner: "owner".to_string(),
            classification: family.to_string(),
            reason: "retained migrated policy entry".to_string(),
            evidence: vec!["legacy-policy:test".to_string()],
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle {
                created: Some("2026-05-26".to_string()),
                review_after: Some("2026-11-01".to_string()),
                expires: None,
            },
            selector: Selector {
                ast_kind: Some(ast_kind.to_string()),
                symbol: Some(symbol.to_string()),
                target_fingerprint: target_fingerprint.map(str::to_string),
                glob: Some(path.to_string()),
                ..Selector::default()
            },
            last_seen: None,
        }
    }

    fn test_finding(
        kind: FindingKind,
        family: Option<&str>,
        path: &str,
        ast_kind: &str,
    ) -> Finding {
        test_finding_at_line(kind, family, path, ast_kind, 1)
    }

    fn test_finding_at_line(
        kind: FindingKind,
        family: Option<&str>,
        path: &str,
        ast_kind: &str,
        line: u32,
    ) -> Finding {
        Finding {
            kind,
            family: family.map(str::to_string),
            path: PathBuf::from(path),
            span: Some(Span { line, column: 1 }),
            identity: StructuralIdentity::new("file", ast_kind),
            message: "test finding".to_string(),
        }
    }

    fn test_outcome(
        status: MatchStatus,
        allow_id: Option<&str>,
        finding_index: Option<usize>,
        message: &str,
    ) -> MatchOutcome {
        MatchOutcome {
            status,
            allow_id: allow_id.map(str::to_string),
            finding_index,
            message: message.to_string(),
            score: 100,
        }
    }

    fn finding_posture_change(
        kind: allow_diff::FindingPostureKind,
        finding_kind: &str,
        family: Option<&str>,
        path: &str,
    ) -> allow_diff::FindingPostureChange {
        allow_diff::FindingPostureChange {
            kind,
            key: format!("{finding_kind}:{path}"),
            finding_kind: finding_kind.to_string(),
            family: family.map(str::to_string),
            path: path.to_string(),
        }
    }

    fn policy_change(
        severity: allow_diff::PolicyChangeSeverity,
        kind: allow_diff::PolicyChangeKind,
    ) -> allow_diff::PolicyChange {
        allow_diff::PolicyChange {
            allow_id: "allow-0001".to_string(),
            kind,
            severity,
            message: "allow-0001 changed".to_string(),
        }
    }

    fn row_status(rows: &[ListRow], id: &str) -> MatchStatus {
        rows.iter()
            .find(|row| row.id == id)
            .map(|row| row.status)
            .unwrap_or_else(|| std::panic::panic_any(format!("missing row {id}")))
    }

    fn list_row(id: &str, kind: FindingKind, owner: &str, classification: &str) -> ListRow {
        ListRow {
            id: id.to_string(),
            status: if classification == "baseline_debt" {
                MatchStatus::BaselineDebt
            } else {
                MatchStatus::Matched
            },
            matches: 1,
            kind,
            family: None,
            owner: owner.to_string(),
            classification: classification.to_string(),
            scope: "src/lib.rs".to_string(),
            source_package: None,
            evidence_count: 0,
            review_after: "-".to_string(),
            expires: "-".to_string(),
            reason: "reason".to_string(),
        }
    }
}
