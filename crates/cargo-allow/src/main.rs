use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, FindingKind, Lifecycle,
    MatchOutcome, MatchStatus, Selector, SimpleDate, glob_matches_str, json_escape, normalize_path,
};
use allow_inventory::{InventoryOptions, InventorySource, inventory, resolve_source_tree_root};
use allow_match::{CheckMode, evaluate, finding_location};
use allow_policy::{
    find_config, load_policy, render_policy, starter_policy, validate_local_evidence_references,
    validate_policy,
};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::str::FromStr;

mod doctor;
mod explain;
mod list;
mod propose;
mod prune;
mod worklist;

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
    List(list::ListArgs),
    /// Explain one allow entry.
    Explain(explain::ExplainArgs),
    /// Generate an allow entry from a current finding.
    Add(AddArgs),
    /// Generate temporary baseline_debt entries.
    Propose(propose::ProposeArgs),
    /// Emit actionable work items for humans or agents.
    Worklist(worklist::WorklistArgs),
    /// Convert compatible legacy policy files.
    Migrate(MigrateArgs),
    /// Preview or remove stale allow entries.
    Prune(prune::PruneArgs),
    /// Validate local setup.
    Doctor(doctor::DoctorArgs),
}

#[derive(Debug, Clone, Default, Args)]
struct RootArgs {
    /// Source tree root. Defaults to the nearest git root, then current directory.
    #[arg(long)]
    root: Option<PathBuf>,
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
    /// Summary output format. Policy output remains TOML.
    #[arg(long, value_enum, default_value_t = MigrateSummaryFormat::Human)]
    summary_format: MigrateSummaryFormat,
    /// Write migration summary to a file instead of stderr.
    #[arg(long)]
    summary_output: Option<PathBuf>,
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
enum AddSummaryFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum MigrateSummaryFormat {
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
        CargoAllowCommand::List(args) => list::cmd_list(&args),
        CargoAllowCommand::Explain(args) => explain::cmd_explain(&args),
        CargoAllowCommand::Add(args) => cmd_add(&args),
        CargoAllowCommand::Propose(args) => propose::cmd_propose(&args),
        CargoAllowCommand::Worklist(args) => worklist::cmd_worklist(&args),
        CargoAllowCommand::Migrate(args) => cmd_migrate(&args),
        CargoAllowCommand::Prune(args) => prune::cmd_prune(&args),
        CargoAllowCommand::Doctor(args) => doctor::cmd_doctor(&args),
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
        json_string_array(allow_report::CLAIM_BOUNDARY)
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        json_string_array(allow_report::SCANNER_LIMITATIONS)
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&migrate_inventory_json(context, "  "));
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

fn migrate_inventory_json(context: &MigrateContext, indent: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("{indent}  \"scope\": \"source_tree\",\n"));
    out.push_str(&format!("{indent}  \"scanner\": \"policy_migration\",\n"));
    out.push_str(&format!(
        "{indent}  \"source\": \"{}\"",
        json_escape(&context.inventory_source)
    ));
    if let Some(root) = &context.source_tree_root {
        out.push_str(&format!(",\n{indent}  \"root\": \"{}\"", json_escape(root)));
    }
    if let Some(files) = context.inventory_files {
        out.push_str(&format!(",\n{indent}  \"files_scanned\": {files}"));
    }
    out.push_str(&format!("\n{indent}}}"));
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
            "migrate entries with evidence"
        );
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

        let list_json = list::sample_list_json_for_contract_test();
        let list = parse_json_artifact("list", &list_json, allow_report::LIST_SCHEMA_ID, "list");
        assert_eq!(
            list.pointer("/summary/allow_entries")
                .and_then(Value::as_u64),
            Some(1),
            "list allow_entries"
        );

        let explain_json = explain::sample_explain_json_for_contract_test();
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

        let worklist_json = worklist::sample_worklist_json_for_contract_test();
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

        let prune_json = prune::sample_prune_json_for_contract_test();
        let prune =
            parse_json_artifact("prune", &prune_json, allow_report::PRUNE_SCHEMA_ID, "prune");
        assert_eq!(
            prune
                .pointer("/summary/stale_entries")
                .and_then(Value::as_u64),
            Some(0),
            "prune stale_entries"
        );

        let propose_json = propose::sample_propose_json_for_contract_test();
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

        let mut migrate_cfg = AllowConfig::empty();
        migrate_cfg
            .allow
            .push(test_entry("allow-migrate", FindingKind::NonRustFile));
        let migrate_json = render_migrate_summary_json(
            &migrate_cfg,
            &MigrateContext {
                inventory_source: "unknown".to_string(),
                source_tree_root: None,
                inventory_files: None,
                input_kind: "from".to_string(),
                input_path: "policy/legacy.toml".to_string(),
            },
            Path::new("policy/allow.toml"),
            false,
        );
        let migrate = parse_json_artifact(
            "migrate",
            &migrate_json,
            allow_report::MIGRATE_SCHEMA_ID,
            "migrate",
        );
        assert_eq!(
            migrate
                .pointer("/summary/allow_entries")
                .and_then(Value::as_u64),
            Some(1),
            "migrate allow_entries"
        );

        let doctor_json = doctor::sample_doctor_json_for_contract_test();
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
            value
                .pointer("/inventory/scanner")
                .and_then(Value::as_str)
                .map(|scanner| scanner == "source_syntax" || scanner == "policy_migration"),
            Some(true),
            "{name} inventory scanner should be source_syntax or policy_migration"
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
}
