use allow_core::{
    AllowConfig, AllowEntry, CargoAllowResult, Finding, FindingKind, MatchOutcome, MatchStatus,
    json_escape, normalize_path,
};
use allow_match::{CheckMode, evaluate};
use allow_policy::evidence_reference_diagnostics;
use clap::{Parser, ValueEnum};
use std::path::{Path, PathBuf};

use crate::{
    RootArgs, json_string_array, load_world_with_evidence_validation, option_json_string,
    option_usize_json, report_config, scope_has_wildcard, source_package_name,
    source_tree_path_matches_filter, source_tree_root_text, write_file,
};
#[derive(Debug, Clone, Parser)]
pub(crate) struct WorklistArgs {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum WorklistFormat {
    Human,
    Json,
}

pub(crate) fn cmd_worklist(args: &WorklistArgs) -> CargoAllowResult<()> {
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

fn work_item_exception_kind(
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
) -> Option<String> {
    finding
        .map(|finding| finding.kind.as_str())
        .or_else(|| entry.map(|entry| entry.kind.as_str()))
        .map(ToOwned::to_owned)
}

pub(crate) fn work_item_kind(
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

pub(crate) fn suggested_actions(kind: &str) -> Vec<String> {
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

pub(crate) fn proof_commands(
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
        allow_report::render_claim_boundary_json()
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        allow_report::render_scanner_limitations_json()
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&allow_report::render_inventory_json(
        allow_report::InventoryContext::source_syntax(
            context.inventory_source,
            context.source_tree_root,
            context.inventory_files,
        ),
        "  ",
    ));
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

#[cfg(test)]
pub(crate) fn sample_worklist_json_for_contract_test() -> String {
    let items = Vec::new();
    render_worklist_json_with_context(
        &items,
        WorklistContext {
            inventory_source: "filesystem_fallback",
            source_tree_root: Some("fixtures/source-snapshot"),
            inventory_files: Some(5),
            filters: WorklistFilters::default(),
        },
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CargoAllowCli, CargoAllowCommand};
    use allow_core::{AllowEntry, Lifecycle, Selector, Span, StructuralIdentity};
    use clap::Parser;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn argv(items: Vec<&str>) -> Vec<String> {
        items.into_iter().map(String::from).collect()
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

    static NEXT_WORKLIST_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    fn migrate_fixture_dir() -> PathBuf {
        let id = NEXT_WORKLIST_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "cargo-allow-cli-worklist-{}-{stamp}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture dir: {err}")));
        dir
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

    fn test_finding(
        kind: FindingKind,
        family: Option<&str>,
        path: &str,
        ast_kind: &str,
    ) -> Finding {
        Finding {
            kind,
            family: family.map(str::to_string),
            path: PathBuf::from(path),
            span: Some(Span { line: 1, column: 1 }),
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
}
