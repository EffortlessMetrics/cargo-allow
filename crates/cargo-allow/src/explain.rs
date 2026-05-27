use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, MatchOutcome, MatchStatus,
    json_escape, normalize_path,
};
use allow_match::{CheckMode, evaluate, finding_location, score_match};
use allow_policy::evidence_reference_diagnostics;
use clap::{Parser, ValueEnum};
use std::path::{Path, PathBuf};

use crate::{
    RootArgs, allow_entry_json, explain_finding_json, json_string_array,
    load_world_with_evidence_validation, option_json_string, option_usize_json,
    source_package_name, source_tree_root_text, worklist, write_file,
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
mod tests {
    use super::*;
    use crate::{CargoAllowCli, CargoAllowCommand};
    use allow_core::{AllowEntry, FindingKind, Lifecycle, Selector, Span, StructuralIdentity};
    use clap::Parser;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn argv(items: Vec<&str>) -> Vec<String> {
        items.into_iter().map(String::from).collect()
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

    static NEXT_EXPLAIN_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    fn migrate_fixture_dir() -> PathBuf {
        let id = NEXT_EXPLAIN_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "cargo-allow-cli-explain-{}-{stamp}-{id}",
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
}
