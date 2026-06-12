use allow_core::{CargoAllowError, CargoAllowResult};
use allow_inventory::resolve_source_tree_root;
use allow_policy::spec_system::{
    SpecSystemConfig, SpecSystemMode, SpecSystemRequirements, SpecSystemRoots, load_doc_artifacts,
    parse_spec_system_config, validate_doc_artifact_files, validate_doc_artifact_links,
    validate_support_tier_claims,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{OutputFormat, RootArgs, emit_text, root_relative_path, write_file};

const PROFILE_NAME: &str = "spec-system";
const DEFAULT_PROFILE_CONFIG: &str = "policy/spec-system.toml";

pub(crate) struct SpecSystemCommandArgs<'a> {
    pub(crate) command: &'a str,
    pub(crate) root: &'a RootArgs,
    pub(crate) config: Option<&'a Path>,
    pub(crate) format: OutputFormat,
    pub(crate) output: Option<&'a Path>,
    pub(crate) receipt: Option<&'a Path>,
}

pub(crate) fn cmd_spec_system(args: SpecSystemCommandArgs<'_>) -> CargoAllowResult<()> {
    let report = build_spec_system_report(args.command, args.root, args.config)?;
    let rendered = render_spec_system_report(&report, args.format);
    emit_text(args.output, &rendered)?;
    if let Some(path) = args.receipt {
        write_file(path, &render_spec_system_json(&report))?;
    }
    Ok(())
}

#[derive(Debug)]
struct SpecSystemReport {
    command: String,
    root: PathBuf,
    config_source: String,
    artifacts: usize,
    support_tier_rows: usize,
    findings: Vec<SpecSystemFinding>,
}

#[derive(Debug)]
struct SpecSystemFinding {
    kind: &'static str,
    message: String,
}

fn build_spec_system_report(
    command: &str,
    root_args: &RootArgs,
    config: Option<&Path>,
) -> CargoAllowResult<SpecSystemReport> {
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    let root = resolve_source_tree_root(root_args.root.as_deref(), cwd)?;
    let (cfg, config_source, mut findings) = load_spec_system_config(&root, config);
    let mut artifacts = 0;
    let mut support_tier_rows = 0;

    let ledger_path = root_relative_path(&root, Path::new(&cfg.roots.artifact_ledger));
    match load_doc_artifacts(&ledger_path) {
        Ok(ledger) => {
            artifacts = ledger.artifact.len();
            collect_validation(
                &mut findings,
                "artifact_file",
                validate_doc_artifact_files(&root, &ledger, &cfg.roots),
            );
            collect_validation(
                &mut findings,
                "artifact_link",
                validate_doc_artifact_links(&ledger),
            );
        }
        Err(err) => findings.push(SpecSystemFinding {
            kind: "doc_artifact_ledger",
            message: err.to_string(),
        }),
    }

    let support_tiers_path = root_relative_path(&root, Path::new(&cfg.roots.support_tiers));
    match fs::read_to_string(&support_tiers_path) {
        Ok(text) => match validate_support_tier_claims(&text) {
            Ok(rows) => {
                support_tier_rows = rows.len();
            }
            Err(err) => findings.push(SpecSystemFinding {
                kind: "support_tier",
                message: err.to_string(),
            }),
        },
        Err(err) => findings.push(SpecSystemFinding {
            kind: "support_tier",
            message: format!(
                "failed to read support-tier file {}: {err}",
                cfg.roots.support_tiers
            ),
        }),
    }

    Ok(SpecSystemReport {
        command: command.to_string(),
        root,
        config_source,
        artifacts,
        support_tier_rows,
        findings,
    })
}

fn load_spec_system_config(
    root: &Path,
    config: Option<&Path>,
) -> (SpecSystemConfig, String, Vec<SpecSystemFinding>) {
    let config_path = config
        .map(|path| root_relative_path(root, path))
        .unwrap_or_else(|| root.join(DEFAULT_PROFILE_CONFIG));

    if !config_path.exists() {
        let findings = if config.is_some() {
            vec![SpecSystemFinding {
                kind: "profile_config",
                message: format!(
                    "spec-system profile config {} does not exist",
                    config_path.display()
                ),
            }]
        } else {
            Vec::new()
        };
        return (
            default_spec_system_config(),
            "default spec-system roots".to_string(),
            findings,
        );
    }

    match fs::read_to_string(&config_path) {
        Ok(text) => match parse_spec_system_config(&text) {
            Ok(cfg) => (cfg, root_relative_display(root, &config_path), Vec::new()),
            Err(err) => (
                default_spec_system_config(),
                "default spec-system roots".to_string(),
                vec![SpecSystemFinding {
                    kind: "profile_config",
                    message: err.to_string(),
                }],
            ),
        },
        Err(err) => (
            default_spec_system_config(),
            "default spec-system roots".to_string(),
            vec![SpecSystemFinding {
                kind: "profile_config",
                message: format!(
                    "failed to read spec-system profile config {}: {err}",
                    config_path.display()
                ),
            }],
        ),
    }
}

fn default_spec_system_config() -> SpecSystemConfig {
    SpecSystemConfig {
        schema_version: "1.0".to_string(),
        profile: PROFILE_NAME.to_string(),
        mode: SpecSystemMode::Advisory,
        roots: SpecSystemRoots {
            proposals: "docs/proposals".to_string(),
            specs: "docs/specs".to_string(),
            adrs: "docs/adr".to_string(),
            plans: "plans".to_string(),
            goals: ".codex/goals".to_string(),
            support_tiers: "docs/status/SUPPORT_TIERS.md".to_string(),
            artifact_ledger: "policy/doc-artifacts.toml".to_string(),
        },
        requirements: SpecSystemRequirements {
            ledger_required: true,
            templates_required: true,
            support_tiers_required: true,
            active_goal_required: true,
            closeout_required_for_done_items: true,
        },
    }
}

fn collect_validation(
    findings: &mut Vec<SpecSystemFinding>,
    kind: &'static str,
    result: CargoAllowResult<()>,
) {
    if let Err(err) = result {
        findings.push(SpecSystemFinding {
            kind,
            message: err.to_string(),
        });
    }
}

fn render_spec_system_report(report: &SpecSystemReport, format: OutputFormat) -> String {
    match format {
        OutputFormat::Json => render_spec_system_json(report),
        OutputFormat::Html => format!(
            "<!doctype html><meta charset=\"utf-8\"><title>cargo-allow spec-system</title><pre>{}</pre>\n",
            html_escape(&render_spec_system_markdown(report))
        ),
        OutputFormat::Sarif => render_spec_system_sarif(report),
        OutputFormat::Human | OutputFormat::Markdown => render_spec_system_markdown(report),
    }
}

fn render_spec_system_markdown(report: &SpecSystemReport) -> String {
    let mut text = String::new();
    text.push_str(&format!(
        "# cargo-allow {} --profile spec-system\n\n",
        report.command
    ));
    text.push_str("**Result:** advisory\n\n");
    text.push_str("Profile: `spec-system`\n\n");
    text.push_str(&format!(
        "Source tree root: `{}`\n\n",
        report.root.display()
    ));
    text.push_str(&format!("Config: `{}`\n\n", report.config_source));
    text.push_str("| Metric | Count |\n|---|---:|\n");
    text.push_str(&format!("| Artifacts | {} |\n", report.artifacts));
    text.push_str(&format!(
        "| Support-tier rows | {} |\n",
        report.support_tier_rows
    ));
    text.push_str(&format!(
        "| Advisory findings | {} |\n",
        report.findings.len()
    ));
    text.push('\n');
    if report.findings.is_empty() {
        text.push_str("No spec-system advisory findings.\n\n");
    } else {
        text.push_str("## Advisory Findings\n\n");
        for finding in &report.findings {
            text.push_str(&format!("- `{}`: {}\n", finding.kind, finding.message));
        }
        text.push('\n');
    }
    text.push_str("> Claim boundary: structural source-tree graph validation only; cargo-allow did not execute proof commands, run tests, invoke Cargo, rustc, Clippy, build scripts, proc macros, external proof tools, network calls, or GitHub APIs.\n");
    text
}

fn render_spec_system_json(report: &SpecSystemReport) -> String {
    let mut text = String::new();
    text.push_str("{\n");
    text.push_str("  \"schema_id\": \"cargo-allow.spec-system.preview.v0\",\n");
    text.push_str("  \"schema_version\": 0,\n");
    text.push_str("  \"tool\": \"cargo-allow\",\n");
    text.push_str(&format!(
        "  \"command\": \"{}\",\n",
        json_escape(&report.command)
    ));
    text.push_str("  \"profile\": \"spec-system\",\n");
    text.push_str("  \"mode\": \"advisory\",\n");
    text.push_str("  \"failed\": false,\n");
    text.push_str("  \"claim_boundary\": [\"structural_source_tree_graph_validation\", \"proof_commands_not_executed\", \"cargo_not_invoked\", \"rustc_not_invoked\", \"clippy_not_invoked\", \"network_not_used\", \"github_api_not_used\"],\n");
    text.push_str(&format!(
        "  \"source_tree_root\": \"{}\",\n",
        json_escape(&report.root.display().to_string())
    ));
    text.push_str(&format!(
        "  \"config_source\": \"{}\",\n",
        json_escape(&report.config_source)
    ));
    text.push_str("  \"summary\": {\n");
    text.push_str(&format!("    \"artifacts\": {},\n", report.artifacts));
    text.push_str(&format!(
        "    \"support_tier_rows\": {},\n",
        report.support_tier_rows
    ));
    text.push_str(&format!("    \"findings\": {}\n", report.findings.len()));
    text.push_str("  },\n");
    text.push_str("  \"findings\": [\n");
    for (index, finding) in report.findings.iter().enumerate() {
        text.push_str("    {");
        text.push_str(&format!("\"kind\": \"{}\", ", json_escape(finding.kind)));
        text.push_str(&format!(
            "\"message\": \"{}\"",
            json_escape(&finding.message)
        ));
        text.push('}');
        if index + 1 != report.findings.len() {
            text.push(',');
        }
        text.push('\n');
    }
    text.push_str("  ]\n");
    text.push_str("}\n");
    text
}

fn render_spec_system_sarif(report: &SpecSystemReport) -> String {
    let mut text = String::new();
    text.push_str("{\n");
    text.push_str("  \"version\": \"2.1.0\",\n");
    text.push_str("  \"runs\": [\n");
    text.push_str("    {\n");
    text.push_str("      \"tool\": {\"driver\": {\"name\": \"cargo-allow spec-system\"}},\n");
    text.push_str("      \"results\": [\n");
    for (index, finding) in report.findings.iter().enumerate() {
        text.push_str("        {");
        text.push_str(&format!("\"ruleId\": \"{}\", ", json_escape(finding.kind)));
        text.push_str(&format!(
            "\"message\": {{\"text\": \"{}\"}}",
            json_escape(&finding.message)
        ));
        text.push('}');
        if index + 1 != report.findings.len() {
            text.push(',');
        }
        text.push('\n');
    }
    text.push_str("      ]\n");
    text.push_str("    }\n");
    text.push_str("  ]\n");
    text.push_str("}\n");
    text
}

fn root_relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push(' '),
            ch => escaped.push(ch),
        }
    }
    escaped
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
