use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, FindingKind, Lifecycle,
    Selector, normalize_path,
};
use allow_inventory::{InventoryOptions, discover_workspace_root, inventory_files};
use allow_match::{CheckMode, evaluate};
use allow_policy::{find_config, load_policy, render_policy, starter_policy};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Human,
    Json,
    Markdown,
}

impl OutputFormat {
    fn parse(value: Option<String>) -> Self {
        match value.as_deref() {
            Some("json") => Self::Json,
            Some("markdown") | Some("md") => Self::Markdown,
            _ => Self::Human,
        }
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(2);
    }
}

fn run() -> CargoAllowResult<()> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(|s| s.as_str()) == Some("allow") {
        args.remove(0);
    }
    let Some(cmd) = args.first().cloned() else {
        print_help();
        return Ok(());
    };
    let rest = &args[1..];
    match cmd.as_str() {
        "init" => cmd_init(rest),
        "audit" => cmd_audit(rest),
        "check" => cmd_check(rest),
        "diff" => cmd_diff(rest),
        "list" => cmd_list(rest),
        "explain" => cmd_explain(rest),
        "propose" => cmd_propose(rest),
        "migrate" => cmd_migrate(rest),
        "prune" => cmd_prune(rest),
        "doctor" => cmd_doctor(rest),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => Err(CargoAllowError::new(format!("unknown command `{other}`"))),
    }
}

fn cmd_init(args: &[String]) -> CargoAllowResult<()> {
    let strict = has_flag(args, "--strict");
    let config = value_of(args, "--config").unwrap_or_else(|| "policy/allow.toml".to_string());
    let path = PathBuf::from(config);
    if path.exists() && !has_flag(args, "--force") {
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
    fs::write(&path, starter_policy(strict))
        .map_err(|e| CargoAllowError::new(format!("failed to write {}: {e}", path.display())))?;
    println!("created {}", path.display());
    Ok(())
}

fn cmd_audit(args: &[String]) -> CargoAllowResult<()> {
    let format = OutputFormat::parse(value_of(args, "--format"));
    let kind = value_of(args, "--kind");
    let (root, cfg, findings) = load_world(args, false, kind.as_deref())?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::Audit);
    print_report(
        "audit",
        format,
        &findings,
        &outcomes,
        false,
        value_of(args, "--output"),
    )?;
    eprintln!("workspace: {}", root.display());
    Ok(())
}

fn cmd_check(args: &[String]) -> CargoAllowResult<()> {
    let format = OutputFormat::parse(value_of(args, "--format"));
    let mode = CheckMode::parse(&value_of(args, "--mode").unwrap_or_else(|| "no-new".to_string()));
    let kind = value_of(args, "--kind");
    let (_root, cfg, findings) = load_world(args, true, kind.as_deref())?;
    let outcomes = evaluate(&cfg, &findings, mode);
    let failed = outcomes.iter().any(|o| mode.fails(o.status));
    print_report(
        "check",
        format,
        &findings,
        &outcomes,
        failed,
        value_of(args, "--output"),
    )?;
    if let Some(path) = value_of(args, "--receipt") {
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

fn cmd_diff(args: &[String]) -> CargoAllowResult<()> {
    let base = value_of(args, "--base")
        .ok_or_else(|| CargoAllowError::new("diff requires --base <rev>"))?;
    let head = value_of(args, "--head");
    let format = OutputFormat::parse(value_of(args, "--format"));
    let (root, cfg, findings) = load_world(args, true, value_of(args, "--kind").as_deref())?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);
    let failed = outcomes.iter().any(|o| CheckMode::NoNew.fails(o.status));
    let mut text = match format {
        OutputFormat::Json => allow_report::render_json("diff", &findings, &outcomes, failed),
        OutputFormat::Markdown => {
            allow_report::render_markdown("diff", &findings, &outcomes, failed)
        }
        OutputFormat::Human => allow_report::render_human("diff", &findings, &outcomes, failed),
    };
    match allow_diff::changed_files(&root, &base, head.as_deref()) {
        Ok(changed) => {
            if format == OutputFormat::Human {
                text.push_str("\nChanged files from git diff:\n");
                for path in changed.iter().take(80) {
                    text.push_str(&format!("  {}\n", normalize_path(path)));
                }
            }
        }
        Err(err) => {
            if format == OutputFormat::Human {
                text.push_str(&format!("\nwarning: could not compute git diff: {err}\n"));
            }
        }
    }
    if let Some(path) = value_of(args, "--output") {
        write_file(path, &text)?;
    } else {
        println!("{text}");
    }
    if failed {
        process::exit(1);
    }
    Ok(())
}

fn cmd_list(args: &[String]) -> CargoAllowResult<()> {
    let cfg = load_config_required(args)?;
    let kind = value_of(args, "--kind");
    for entry in cfg.allow.iter().filter(|e| {
        kind.as_deref()
            .map(|k| e.kind.as_str() == k)
            .unwrap_or(true)
    }) {
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

fn cmd_explain(args: &[String]) -> CargoAllowResult<()> {
    let id = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .ok_or_else(|| CargoAllowError::new("explain requires an allow id"))?;
    let cfg = load_config_required(args)?;
    let entry = cfg
        .allow
        .iter()
        .find(|e| e.id == *id)
        .ok_or_else(|| CargoAllowError::new(format!("no allow entry `{id}`")))?;
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

fn cmd_propose(args: &[String]) -> CargoAllowResult<()> {
    let kind = value_of(args, "--kind");
    let (_root, cfg, findings) = load_world(args, false, kind.as_deref())?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::Audit);
    let mut proposed = cfg.clone();
    let start = proposed.allow.len() + 1;
    let expires = value_of(args, "--expires").unwrap_or_else(|| "2026-08-01".to_string());
    for (n, outcome) in outcomes
        .iter()
        .filter(|o| o.status == allow_core::MatchStatus::New)
        .enumerate()
    {
        if let Some(idx) = outcome.finding_index {
            proposed
                .allow
                .push(entry_from_finding(&findings[idx], start + n, &expires));
        }
    }
    let rendered = render_policy(&proposed);
    if let Some(path) = value_of(args, "--write") {
        write_file(path, &rendered)?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}

fn cmd_migrate(args: &[String]) -> CargoAllowResult<()> {
    let from = value_of(args, "--from")
        .ok_or_else(|| CargoAllowError::new("migrate requires --from <path>"))?;
    let out = value_of(args, "--out").unwrap_or_else(|| "policy/allow.toml".to_string());
    let cfg = allow_policy_legacy::load_legacy_or_canonical(&from)?;
    write_file(out, &render_policy(&cfg))?;
    eprintln!("{}", allow_policy_legacy::migration_notes());
    Ok(())
}

fn cmd_prune(_args: &[String]) -> CargoAllowResult<()> {
    println!(
        "prune MVP is dry-run only. Use check/audit stale results, then edit policy/allow.toml."
    );
    Ok(())
}

fn cmd_doctor(args: &[String]) -> CargoAllowResult<()> {
    let root = discover_workspace_root(
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?,
    )?;
    println!("workspace root: {}", root.display());
    match config_path(args) {
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
    args: &[String],
    require_config: bool,
    kind_filter: Option<&str>,
) -> CargoAllowResult<(PathBuf, AllowConfig, Vec<Finding>)> {
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    let root = discover_workspace_root(cwd)?;
    let cfg = if require_config {
        load_config_required(args)?
    } else {
        load_config_optional(args)?.unwrap_or_else(AllowConfig::empty)
    };
    let opts = InventoryOptions {
        ignored: cfg.workspace.ignored.clone(),
        generated: cfg.workspace.generated.clone(),
        include_untracked: false,
    };
    let files = inventory_files(&root, &opts)?;
    let mut findings = Vec::new();
    findings.extend(allow_rust::scan_rust_files(&root, &files)?);
    findings.extend(allow_files::scan_files(&files));
    if let Some(kind) = kind_filter {
        let parsed = FindingKind::from_str(kind)?;
        findings.retain(|f| {
            f.kind == parsed || (parsed == FindingKind::Panic && f.kind == FindingKind::Panic)
        });
    }
    Ok((root, cfg, findings))
}

fn load_config_required(args: &[String]) -> CargoAllowResult<AllowConfig> {
    let path = config_path(args).ok_or_else(|| {
        CargoAllowError::new("no policy config found; run `cargo allow init` or pass --config")
    })?;
    load_policy(path)
}

fn load_config_optional(args: &[String]) -> CargoAllowResult<Option<AllowConfig>> {
    match config_path(args) {
        Some(path) => Ok(Some(load_policy(path)?)),
        None => Ok(None),
    }
}

fn config_path(args: &[String]) -> Option<PathBuf> {
    value_of(args, "--config")
        .map(PathBuf::from)
        .or_else(|| find_config(env::current_dir().ok()?))
}

fn print_report(
    command: &str,
    format: OutputFormat,
    findings: &[Finding],
    outcomes: &[allow_core::MatchOutcome],
    failed: bool,
    output: Option<String>,
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

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn value_of(args: &[String], flag: &str) -> Option<String> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == flag {
            return iter.next().cloned();
        }
        if let Some(rest) = arg.strip_prefix(&format!("{flag}=")) {
            return Some(rest.to_string());
        }
    }
    None
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

fn print_help() {
    println!(
        r#"cargo-allow: source exception ledger for Rust workspaces

Usage through Cargo:
  cargo allow <command> [options]

Commands:
  init       Create policy/allow.toml
  audit      Inventory exceptions and policy health
  check      CI gate for the exception ledger
  diff       PR-oriented report with git changed files
  list       List allow entries
  explain    Explain one allow entry
  propose    Generate temporary baseline_debt entries
  migrate    Convert compatible legacy policy files
  prune      Dry-run stale cleanup guidance
  doctor     Validate local setup

Common options:
  --config <path>       Policy config path
  --kind <kind>         panic | unsafe | lint_exception | non_rust_file | generated_code
  --format <format>     human | json | markdown
"#
    );
}
