use allow_core::{
    AllowConfig, AllowEntry, CargoAllowResult, Finding, FindingKind, MatchOutcome, MatchStatus,
    normalize_path,
};
use allow_match::{CheckMode, evaluate};
use allow_policy::evidence_reference_diagnostics;
use clap::{Parser, ValueEnum};
use std::path::{Path, PathBuf};

use crate::{
    RootArgs, load_world_with_evidence_validation, report_config, scope_has_wildcard,
    source_package_name, source_tree_path_matches_filter, source_tree_root_text, write_file,
};

#[path = "worklist_actions.rs"]
mod worklist_actions;
#[path = "worklist_render.rs"]
mod worklist_render;
pub(crate) use worklist_actions::{proof_commands, suggested_actions};
use worklist_render::{render_worklist_human_with_context, render_worklist_json_with_context};

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
#[path = "worklist_filter_tests.rs"]
mod filter_tests;
#[cfg(test)]
#[path = "worklist_render_tests.rs"]
mod render_tests;
#[cfg(test)]
#[path = "worklist_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "worklist_tests.rs"]
mod tests;
