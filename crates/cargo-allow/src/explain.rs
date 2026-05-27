use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, MatchOutcome, MatchStatus,
    normalize_path,
};
use allow_match::{CheckMode, evaluate, finding_location, score_match};
use allow_policy::evidence_reference_diagnostics;
use clap::{Parser, ValueEnum};
use std::path::{Path, PathBuf};

use crate::{
    RootArgs, load_world_with_evidence_validation, source_package_name, source_tree_root_text,
    worklist, write_file,
};
#[derive(Debug, Clone, Parser)]
pub(crate) struct ExplainArgs {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ExplainFormat {
    Human,
    Json,
}

pub(crate) fn cmd_explain(args: &ExplainArgs) -> CargoAllowResult<()> {
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
            let kind = worklist::work_item_kind(outcome, finding, Some(entry));
            out.push_str("\nnext:\n");
            for action in worklist::suggested_actions(&kind).into_iter().take(2) {
                out.push_str(&format!("- action: {action}\n"));
            }
            for command in worklist::proof_commands(&kind, finding, Some(entry))
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
        for action in worklist::suggested_actions(kind).into_iter().take(2) {
            out.push_str(&format!("- action: {action}\n"));
        }
        for command in worklist::proof_commands(kind, finding, Some(entry))
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
    let evidence_diagnostics = evidence_reference_diagnostics(root, entry);
    let normalized_targets = evidence_diagnostics
        .iter()
        .map(|diagnostic| diagnostic.target.as_ref().map(normalize_path))
        .collect::<Vec<_>>();
    let evidence_references = evidence_diagnostics
        .iter()
        .zip(normalized_targets.iter())
        .map(|(diagnostic, target)| allow_report::EvidenceReference {
            raw: &diagnostic.raw,
            prefix: diagnostic.prefix.as_deref(),
            target: target.as_deref(),
            status: diagnostic.status.as_str(),
            message: &diagnostic.message,
        })
        .collect::<Vec<_>>();

    allow_report::render_explain_json(allow_report::ExplainReport {
        inventory: allow_report::InventoryContext::source_syntax(
            context.inventory_source,
            context.source_tree_root,
            context.inventory_files,
        ),
        entry,
        current_findings: findings,
        match_outcomes: outcomes,
        evidence_references: &evidence_references,
        suggested_actions: &suggested_actions,
        proof_commands: &proof_commands,
    })
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
        let kind = worklist::work_item_kind(outcome, finding, Some(entry));
        return (
            worklist::suggested_actions(&kind)
                .into_iter()
                .take(2)
                .collect(),
            worklist::proof_commands(&kind, finding, Some(entry))
                .into_iter()
                .take(3)
                .collect(),
        );
    }
    if entry.classification == "baseline_debt" {
        let finding = findings.first();
        let kind = "baseline_debt";
        return (
            worklist::suggested_actions(kind)
                .into_iter()
                .take(2)
                .collect(),
            worklist::proof_commands(kind, finding, Some(entry))
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

#[cfg(test)]
pub(crate) fn sample_explain_json_for_contract_test() -> String {
    use allow_core::{FindingKind, Lifecycle, Selector, Span, StructuralIdentity};
    use std::path::PathBuf;

    let mut cfg = AllowConfig::empty();
    let entry = AllowEntry {
        id: "allow-json".to_string(),
        kind: FindingKind::NonRustFile,
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
    };
    cfg.allow.push(entry.clone());
    let finding = Finding {
        kind: FindingKind::NonRustFile,
        family: None,
        path: PathBuf::from("tracked.file"),
        span: Some(Span { line: 1, column: 1 }),
        identity: StructuralIdentity::new("file", "tracked_file"),
        message: "test finding".to_string(),
    };
    explain_entry_json(
        Path::new("."),
        &cfg,
        &entry,
        &[finding],
        ExplainContext {
            inventory_source: "git_tracked",
            source_tree_root: Some("H:/Code/Rust/cargo-allow"),
            inventory_files: Some(47),
        },
    )
}

#[cfg(test)]
#[path = "explain_tests.rs"]
mod tests;
