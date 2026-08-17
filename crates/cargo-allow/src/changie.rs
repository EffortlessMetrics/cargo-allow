//! Opt-in Changie sensor projection (#3623 PR C3).
//!
//! `cargo allow changie lint` runs the exact Rust-native static analysis
//! over one deliberately selected source subject and projects the one
//! canonical `ChangieAnalysisResultV1` through human, JSON, and SARIF
//! surfaces. The command is thin over the #3622 adapter: it selects the
//! view and format, renders, and maps to an exit code. It re-evaluates
//! nothing, starts no process, touches no ledger, and never claims
//! Changie rendered, batched, or merged anything.
//!
//! Exit codes: 0 only for a complete analysis with zero findings;
//! 1 for any finding or any non-complete acquisition state (partial is
//! never equivalent to clean for scripts); 2 for acquisition errors.

use crate::cli_types::RootArgs;
use crate::{CargoAllowResult, current_dir, emit_text, resolve_source_tree_root};
use allow_files::changie_lint::ChangieDiagnostic;
use allow_files::changie_lint::ChangieResultClass;
use allow_files::changie_lint::sensor::ChangieSensor;
use clap::{Args, Subcommand, ValueEnum};

use crate::changie_source_view::{
    ChangieAcquisitionCompleteness, ChangieAnalysisResultV1, ChangieConfigSelectionV1,
    ChangieSourceViewError, analyze_source_view,
};

/// Output format for the Changie sensor projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ChangieFormat {
    Human,
    Json,
    Sarif,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ChangieArgs {
    #[command(flatten)]
    pub(crate) root: RootArgs,
    #[command(subcommand)]
    pub(crate) command: ChangieCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum ChangieCommand {
    /// Run the Rust-native Changie static sensor over one exact source subject.
    Lint(ChangieLintArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ChangieLintArgs {
    /// Analyze the staged index instead of the saved worktree. Staged
    /// analysis never reads dirty worktree bytes.
    #[arg(long, conflicts_with_all = ["committed"])]
    pub(crate) staged: bool,
    /// Analyze an exact committed revision instead of the saved worktree.
    #[arg(long, conflicts_with = "staged")]
    pub(crate) committed: Option<String>,
    /// Select an explicit repository-relative Changie config path.
    /// Default: `.changie.yaml` before `.changie.yml`.
    #[arg(long)]
    pub(crate) config: Option<String>,
    #[arg(long, value_enum, default_value_t = ChangieFormat::Human)]
    pub(crate) format: ChangieFormat,
    /// Write output to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

pub(crate) fn cmd_changie(args: &ChangieArgs) -> CargoAllowResult<()> {
    let ChangieCommand::Lint(lint) = &args.command;
    let cwd = current_dir()?;
    let root = resolve_source_tree_root(args.root.root.as_deref(), cwd)?;
    let view = if lint.staged {
        effortless_repo_snapshot::RepositorySourceView::staged(&root)
            .map_err(|error| usage_error(format!("staged source view: {error}")))?
    } else if let Some(revision) = &lint.committed {
        effortless_repo_snapshot::RepositorySourceView::committed(&root, revision)
            .map_err(|error| usage_error(format!("committed source view {revision}: {error}")))?
    } else {
        effortless_repo_snapshot::RepositorySourceView::filesystem(&root)
            .map_err(|error| usage_error(format!("saved worktree view: {error}")))?
    };
    // The command supports exactly the three #3622 view kinds; any
    // other request is rejected before analysis rather than gaining a
    // filesystem/Git fallback (falsifier 6).
    if lint.staged && lint.committed.is_some() {
        return Err(usage_error(
            crate::changie_source_view::unsupported_view_kind("staged+committed").to_string(),
        ));
    }
    let selection = match &lint.config {
        Some(path) => ChangieConfigSelectionV1::Explicit(path.clone()),
        None => ChangieConfigSelectionV1::DefaultNames,
    };
    let mut result = analyze_source_view(&view, &selection).map_err(map_source_error)?;
    // Instrumentation honesty: any view limitation that names an
    // instrument failure degrades the acquisition to not-proven rather
    // than letting it render as clean (#3623 falsifier 3).
    if result
        .limitations
        .iter()
        .any(|limitation| limitation.contains("instrument failure"))
    {
        crate::changie_source_view::mark_not_proven(
            &mut result,
            "view-reported instrument failure",
        );
    }
    let failed = should_fail(&result);
    let text = match lint.format {
        ChangieFormat::Human => render_human(&result),
        ChangieFormat::Json => render_json(&result),
        ChangieFormat::Sarif => render_sarif(&result),
    };
    emit_text(lint.output.as_deref(), &text)?;
    if failed {
        std::process::exit(1);
    }
    Ok(())
}

/// Exit posture: only a complete, finding-free analysis exits 0. Partial
/// and not-proven acquisitions are never equivalent to clean for
/// scripts (#3623 falsifier 3).
fn should_fail(result: &ChangieAnalysisResultV1) -> bool {
    !result.report.diagnostics.is_empty()
        || result.completeness != ChangieAcquisitionCompleteness::Complete
}

fn usage_error(message: String) -> allow_core::CargoAllowError {
    allow_core::CargoAllowError::with_kind(allow_core::CargoAllowErrorKind::Usage, message)
}

fn map_source_error(error: ChangieSourceViewError) -> allow_core::CargoAllowError {
    allow_core::CargoAllowError::with_kind(
        allow_core::CargoAllowErrorKind::Usage,
        format!("changie lint: {error}"),
    )
}

fn render_human(result: &ChangieAnalysisResultV1) -> String {
    let mut out = String::new();
    out.push_str("Changie static sensor (Rust-native; no Go, Aqua, or Changie executed)\n");
    out.push_str(&format!(
        "  generation: {}  view: {}  completeness: {}\n",
        result.generation,
        result.view_kind.as_str(),
        result.completeness.as_str()
    ));
    out.push_str(&format!(
        "  config: {} ({} identity {})\n",
        result.config_selection.selected_path,
        match result.config_selection.mode {
            ChangieConfigSelectionV1::Explicit(_) => "explicit",
            ChangieConfigSelectionV1::DefaultNames => "default names",
        },
        result
            .config_content_identity
            .map(|identity| identity.to_string())
            .unwrap_or_else(|| "unknown".into())
    ));
    out.push_str(&format!(
        "  population root: {}  inspected: {}  omitted: {}\n",
        result.population.root,
        result.population.inspected.len(),
        result.population.omitted.len()
    ));
    out.push_str(&format!(
        "  analysis identity: {}\n",
        result.analysis_identity
    ));
    if result.config_selection.ambiguous {
        out.push_str(
            "  note: both .changie.yaml and .changie.yml exist; precedence selected the former\n",
        );
    }
    for limitation in &result.limitations {
        out.push_str(&format!("  limitation: {limitation}\n"));
    }
    if result.report.diagnostics.is_empty() {
        out.push_str(match result.completeness {
            ChangieAcquisitionCompleteness::Complete => {
                "Result: clean (static authoring contract satisfied; rendering not claimed)\n"
            }
            ChangieAcquisitionCompleteness::Partial => {
                "Result: incomplete acquisition — no findings, but coverage is partial\n"
            }
            ChangieAcquisitionCompleteness::NotProven => {
                "Result: not proven — operation-affecting configuration is outside the static contract\n"
            }
        });
        return out;
    }
    out.push_str(&format!(
        "Result: {} finding(s)\n",
        result.report.diagnostics.len()
    ));
    for diagnostic in &result.report.diagnostics {
        out.push_str(&render_diagnostic_human(diagnostic));
    }
    out
}

fn render_diagnostic_human(diagnostic: &ChangieDiagnostic) -> String {
    let mut line = format!(
        "  {} [{}] {}",
        diagnostic.rule.as_str(),
        class_label(diagnostic.result_class),
        diagnostic.message
    );
    if let Some(range) = diagnostic.range {
        line.push_str(&format!(
            " ({}:{}:{})",
            diagnostic.repo_path, range.start.line, range.start.column
        ));
    } else {
        line.push_str(&format!(" ({})", diagnostic.repo_path));
    }
    line.push('\n');
    if let Some(expected_actual) = &diagnostic.expected_actual {
        line.push_str(&format!(
            "    expected: {}  actual: {}\n",
            expected_actual.expected, expected_actual.actual
        ));
    }
    if !diagnostic.actions.is_empty() {
        let actions: Vec<&str> = diagnostic
            .actions
            .iter()
            .map(|action| action.as_str())
            .collect();
        line.push_str(&format!("    action: {}\n", actions.join("; ")));
    }
    line.push_str(&format!("    provenance: {}\n", diagnostic.provenance()));
    line
}

fn class_label(class: ChangieResultClass) -> &'static str {
    match class {
        ChangieResultClass::Finding => "finding",
        ChangieResultClass::Malformed => "malformed",
        ChangieResultClass::Unsupported => "unsupported",
        ChangieResultClass::Partial => "partial",
    }
}

/// Deterministic machine projection: the canonical sensor serialization
/// plus the source-view identity envelope. Scripts parse this, never
/// the human prose.
fn render_json(result: &ChangieAnalysisResultV1) -> String {
    let sensor = ChangieSensor;
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"schema\": \"cargo-allow.changie-analysis.v1\",\n");
    out.push_str(&format!("  \"generation\": \"{}\",\n", result.generation));
    out.push_str(&format!(
        "  \"diagnostic_schema_generation\": {},\n",
        sensor.diagnostic_schema_generation()
    ));
    out.push_str(&format!(
        "  \"effective_rule_schema_generation\": {},\n",
        sensor.effective_rule_schema_generation()
    ));
    out.push_str(&format!(
        "  \"view\": {{\"kind\": \"{}\", \"identity\": {}, \"completeness\": \"{}\"}},\n",
        result.view_kind.as_str(),
        match &result.view_identity {
            Some(identity) => format!("\"{identity}\""),
            None => "null".to_string(),
        },
        result.completeness.as_str()
    ));
    out.push_str(&format!(
        "  \"config\": {{\"selected_path\": \"{}\", \"ambiguous\": {}}},\n",
        result.config_selection.selected_path, result.config_selection.ambiguous
    ));
    out.push_str(&format!(
        "  \"population\": {{\"root\": \"{}\", \"inspected\": {}, \"omitted\": {}}},\n",
        result.population.root,
        result.population.inspected.len(),
        result.population.omitted.len()
    ));
    out.push_str(&format!(
        "  \"analysisIdentity\": \"{}\",\n",
        result.analysis_identity
    ));
    let rules: Vec<String> = result
        .report
        .diagnostics
        .iter()
        .map(|diagnostic| {
            format!(
                "{{\"rule\": \"{}\", \"class\": \"{}\", \"provenance\": \"{}\", \"path\": \"{}\", \"line\": {}}}",
                diagnostic.rule.as_str(),
                class_label(diagnostic.result_class),
                diagnostic.provenance(),
                diagnostic.repo_path,
                diagnostic.range.map(|range| range.start.line).unwrap_or(0)
            )
        })
        .collect();
    out.push_str(&format!("  \"diagnostics\": [{}]\n", rules.join(", ")));
    out.push_str("}\n");
    out
}

/// SARIF 2.1.0 projection. Source ranges become physical locations;
/// related config declarations become additional locations. The taxon
/// is the stable changie.* rule id; no compilation or security proof
/// is claimed anywhere.
fn render_sarif(result: &ChangieAnalysisResultV1) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"$schema\": \"https://json.schemastore.org/sarif-2.1.0.json\",\n");
    out.push_str("  \"version\": \"2.1.0\",\n");
    out.push_str("  \"runs\": [\n    {\n");
    out.push_str("      \"tool\": {\n        \"driver\": {\n");
    out.push_str("          \"name\": \"cargo-allow\",\n");
    out.push_str(
        "          \"informationUri\": \"https://github.com/EffortlessMetrics/cargo-allow\",\n",
    );
    out.push_str(&format!(
        "          \"semanticVersion\": \"changie-sensor-{}\",\n",
        result.generation
    ));
    out.push_str("        }\n      },\n");
    out.push_str(&format!(
        "      \"properties\": {{\"analysisIdentity\": \"{}\", \"completeness\": \"{}\", \"claimBoundary\": \"static authoring contract only; no render, batch, or merge proof\"}},\n",
        result.analysis_identity,
        result.completeness.as_str()
    ));
    out.push_str("      \"results\": [\n");
    let entries: Vec<String> = result
        .report
        .diagnostics
        .iter()
        .map(render_sarif_result)
        .collect();
    out.push_str(&entries.join(",\n"));
    if entries.is_empty() {
        out.push_str("        {\"message\": {\"text\": \"no findings\"}, \"kind\": \"pass\"}");
    }
    out.push_str("\n      ]\n");
    out.push_str("    }\n  ]\n}\n");
    out
}

fn render_sarif_result(diagnostic: &ChangieDiagnostic) -> String {
    let mut entry = String::new();
    entry.push_str("        {\n");
    entry.push_str(&format!(
        "          \"ruleId\": \"{}\",\n",
        diagnostic.rule.as_str()
    ));
    entry.push_str(&format!(
        "          \"kind\": \"{}\",\n",
        match diagnostic.result_class {
            ChangieResultClass::Finding => "fail",
            ChangieResultClass::Malformed => "fail",
            ChangieResultClass::Unsupported => "review",
            ChangieResultClass::Partial => "review",
        }
    ));
    entry.push_str(&format!(
        "          \"level\": \"{}\",\n",
        match diagnostic.result_class {
            ChangieResultClass::Finding | ChangieResultClass::Malformed => "error",
            ChangieResultClass::Unsupported | ChangieResultClass::Partial => "note",
        }
    ));
    entry.push_str(&format!(
        "          \"message\": {{\"text\": {}}},\n",
        json_string(&diagnostic.message)
    ));
    entry.push_str(&format!(
        "          \"locations\": [{{\"physicalLocation\": {{\"artifactLocation\": {{\"uri\": \"{}\"}}, \"region\": {}}}}}",
        diagnostic.repo_path,
        diagnostic
            .range
            .map(|range| {
                format!(
                    "{{\"startLine\": {}, \"startColumn\": {}}}",
                    range.start.line, range.start.column
                )
            })
            .unwrap_or_else(|| "{\"startLine\": 1}".to_string())
    ));
    entry.push_str("],\n");
    entry.push_str(&format!(
        "          \"properties\": {{\"provenance\": \"{}\", \"resultClass\": \"{}\"}}\n",
        diagnostic.provenance(),
        class_label(diagnostic.result_class)
    ));
    entry.push_str("        }");
    entry
}

fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\\\""),
            '\\' => out.push_str("\\\\\\\\"),
            '\n' => out.push_str("\\\\n"),
            '\r' => out.push_str("\\\\r"),
            '\t' => out.push_str("\\\\t"),
            other if (other as u32) < 0x20 => {
                out.push_str(&format!("\\\\u{:04x}", other as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

use std::path::PathBuf;

#[cfg(test)]
#[path = "changie_command_tests.rs"]
mod changie_command_tests;
