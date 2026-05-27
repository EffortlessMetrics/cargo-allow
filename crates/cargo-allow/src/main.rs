use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, FindingKind, Lifecycle,
    Selector, glob_matches_str, json_escape, normalize_path,
};
use allow_inventory::{InventoryOptions, InventorySource, inventory, resolve_source_tree_root};
#[cfg(test)]
use allow_match::{CheckMode, evaluate};
use allow_policy::{find_config, load_policy, validate_local_evidence_references};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::str::FromStr;

mod add;
mod audit;
mod check;
mod diff;
mod doctor;
mod explain;
mod init;
mod list;
mod migrate;
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
    Init(init::InitArgs),
    /// Inventory exceptions and policy health.
    Audit(audit::ReportArgs),
    /// CI gate for the exception ledger.
    Check(check::CheckArgs),
    /// PR-oriented report with git changed files.
    Diff(diff::DiffArgs),
    /// List allow entries.
    List(list::ListArgs),
    /// Explain one allow entry.
    Explain(explain::ExplainArgs),
    /// Generate an allow entry from a current finding.
    Add(add::AddArgs),
    /// Generate temporary baseline_debt entries.
    Propose(propose::ProposeArgs),
    /// Emit actionable work items for humans or agents.
    Worklist(worklist::WorklistArgs),
    /// Convert compatible legacy policy files.
    Migrate(migrate::MigrateArgs),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Human,
    Html,
    Json,
    Sarif,
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
        CargoAllowCommand::Init(args) => init::cmd_init(&args),
        CargoAllowCommand::Audit(args) => audit::cmd_audit(&args),
        CargoAllowCommand::Check(args) => check::cmd_check(&args),
        CargoAllowCommand::Diff(args) => diff::cmd_diff(&args),
        CargoAllowCommand::List(args) => list::cmd_list(&args),
        CargoAllowCommand::Explain(args) => explain::cmd_explain(&args),
        CargoAllowCommand::Add(args) => add::cmd_add(&args),
        CargoAllowCommand::Propose(args) => propose::cmd_propose(&args),
        CargoAllowCommand::Worklist(args) => worklist::cmd_worklist(&args),
        CargoAllowCommand::Migrate(args) => migrate::cmd_migrate(&args),
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
    use allow_core::{MatchStatus, Span, StructuralIdentity};
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
            Some(CargoAllowCommand::Check(check::CheckArgs {
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
            Some(CargoAllowCommand::Check(check::CheckArgs {
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
            Some(CargoAllowCommand::Check(check::CheckArgs {
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
            Some(CargoAllowCommand::Check(check::CheckArgs {
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
        let diff_json = diff::render_diff_json_with_posture(diff_base_json, &[], &[], &[]);
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

        let add_json = add::sample_add_json_for_contract_test();
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

        let migrate_json = migrate::sample_migrate_json_for_contract_test();
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
            Some(CargoAllowCommand::Check(check::CheckArgs {
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
            Some(CargoAllowCommand::Check(check::CheckArgs {
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
            Some(CargoAllowCommand::Check(check::CheckArgs {
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
            Some(CargoAllowCommand::Check(check::CheckArgs {
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
            Some(CargoAllowCommand::Check(check::CheckArgs {
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
            Some(CargoAllowCommand::Check(check::CheckArgs {
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
            Some(CargoAllowCommand::Check(check::CheckArgs {
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
            Some(CargoAllowCommand::Check(check::CheckArgs {
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
            Some(CargoAllowCommand::Check(check::CheckArgs {
                compat: true,
                kind: Some(kind),
                ..
            })) if kind == "network"
        ));
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
}
