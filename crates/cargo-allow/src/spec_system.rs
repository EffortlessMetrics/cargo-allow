use allow_core::{CargoAllowError, CargoAllowResult};
use allow_inventory::resolve_source_tree_root;
use allow_policy::spec_system::{
    ArtifactKind, ArtifactStatus, DocArtifact, DocArtifactLedger, SpecSystemConfig, SpecSystemMode,
    SpecSystemRequirements, SpecSystemRoots, load_doc_artifacts, parse_spec_system_config,
    validate_doc_artifact_files, validate_doc_artifact_links, validate_support_tier_claims,
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
    artifacts: Vec<SpecSystemArtifact>,
    links: Vec<SpecSystemLink>,
    support_tier_rows: usize,
    findings: Vec<SpecSystemFinding>,
}

#[derive(Debug)]
struct SpecSystemArtifact {
    id: String,
    kind: &'static str,
    path: String,
    status: &'static str,
    owner: String,
    created: String,
}

#[derive(Debug)]
struct SpecSystemLink {
    source_id: String,
    field: &'static str,
    target: String,
    target_kind: Option<&'static str>,
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
    let mut artifacts = Vec::new();
    let mut links = Vec::new();
    let mut support_tier_rows = 0;

    let ledger_path = root_relative_path(&root, Path::new(&cfg.roots.artifact_ledger));
    match load_doc_artifacts(&ledger_path) {
        Ok(ledger) => {
            artifacts = collect_artifacts(&ledger);
            links = collect_links(&ledger);
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
        links,
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
    text.push_str(&format!("| Artifacts | {} |\n", report.artifacts.len()));
    text.push_str(&format!("| Links | {} |\n", report.links.len()));
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
    text.push_str("  \"schema_version\": 1,\n");
    text.push_str(&format!(
        "  \"schema_id\": \"{}\",\n",
        allow_report::SPEC_SYSTEM_SCHEMA_ID
    ));
    text.push_str("  \"tool\": \"cargo-allow\",\n");
    text.push_str(&format!(
        "  \"command\": \"{}\",\n",
        json_escape(&report.command)
    ));
    text.push_str("  \"profile\": \"spec-system\",\n");
    text.push_str("  \"mode\": \"advisory\",\n");
    text.push_str("  \"status\": \"passed\",\n");
    text.push_str("  \"failed\": false,\n");
    text.push_str("  \"claim_boundary\": ");
    render_string_array(&mut text, allow_report::SPEC_SYSTEM_CLAIM_BOUNDARY, "  ");
    text.push_str(",\n");
    text.push_str("  \"scanner_limitations\": ");
    render_string_array(
        &mut text,
        allow_report::SPEC_SYSTEM_SCANNER_LIMITATIONS,
        "  ",
    );
    text.push_str(",\n");
    text.push_str("  \"inventory\": {\n");
    text.push_str(&format!(
        "    \"scope\": \"{}\",\n",
        allow_report::INVENTORY_SCOPE_SOURCE_TREE
    ));
    text.push_str(&format!(
        "    \"scanner\": \"{}\",\n",
        allow_report::INVENTORY_SCANNER_SOURCE_TREE_GRAPH
    ));
    text.push_str(&format!(
        "    \"source\": \"{}\",\n",
        allow_report::INVENTORY_SOURCE_UNKNOWN
    ));
    text.push_str(&format!(
        "    \"root\": \"{}\"\n",
        json_escape(&report.root.display().to_string())
    ));
    text.push_str("  },\n");
    text.push_str(&format!(
        "  \"source_tree_root\": \"{}\",\n",
        json_escape(&report.root.display().to_string())
    ));
    text.push_str(&format!(
        "  \"config_source\": \"{}\",\n",
        json_escape(&report.config_source)
    ));
    text.push_str("  \"summary\": {\n");
    text.push_str(&format!("    \"artifacts\": {},\n", report.artifacts.len()));
    text.push_str(&format!("    \"links\": {},\n", report.links.len()));
    text.push_str(&format!(
        "    \"support_tier_rows\": {},\n",
        report.support_tier_rows
    ));
    text.push_str(&format!("    \"findings\": {},\n", report.findings.len()));
    text.push_str("    \"work_items\": 0\n");
    text.push_str("  },\n");
    text.push_str("  \"artifacts\": [\n");
    for (index, artifact) in report.artifacts.iter().enumerate() {
        text.push_str("    {\n");
        text.push_str(&format!(
            "      \"id\": \"{}\",\n",
            json_escape(&artifact.id)
        ));
        text.push_str(&format!("      \"kind\": \"{}\",\n", artifact.kind));
        text.push_str(&format!(
            "      \"path\": \"{}\",\n",
            json_escape(&artifact.path)
        ));
        text.push_str(&format!("      \"status\": \"{}\",\n", artifact.status));
        text.push_str(&format!(
            "      \"owner\": \"{}\",\n",
            json_escape(&artifact.owner)
        ));
        text.push_str(&format!(
            "      \"created\": \"{}\"\n",
            json_escape(&artifact.created)
        ));
        text.push_str("    }");
        if index + 1 != report.artifacts.len() {
            text.push(',');
        }
        text.push('\n');
    }
    text.push_str("  ],\n");
    text.push_str("  \"links\": [\n");
    for (index, link) in report.links.iter().enumerate() {
        text.push_str("    {");
        text.push_str(&format!(
            "\"source_id\": \"{}\", ",
            json_escape(&link.source_id)
        ));
        text.push_str(&format!("\"field\": \"{}\", ", link.field));
        text.push_str(&format!("\"target\": \"{}\"", json_escape(&link.target)));
        if let Some(target_kind) = link.target_kind {
            text.push_str(&format!(", \"target_kind\": \"{}\"", target_kind));
        }
        text.push('}');
        if index + 1 != report.links.len() {
            text.push(',');
        }
        text.push('\n');
    }
    text.push_str("  ],\n");
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
    text.push_str("  ],\n");
    text.push_str("  \"work_items\": []\n");
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

fn collect_artifacts(ledger: &DocArtifactLedger) -> Vec<SpecSystemArtifact> {
    ledger
        .artifact
        .iter()
        .map(|artifact| SpecSystemArtifact {
            id: artifact.id.clone(),
            kind: artifact_kind_name(artifact.kind),
            path: artifact.path.clone(),
            status: artifact_status_name(artifact.status),
            owner: artifact.owner.clone(),
            created: artifact.created.clone(),
        })
        .collect()
}

fn collect_links(ledger: &DocArtifactLedger) -> Vec<SpecSystemLink> {
    let mut links = Vec::new();
    for artifact in &ledger.artifact {
        collect_link_fields(&mut links, artifact, ledger);
    }
    links
}

fn collect_link_fields(
    links: &mut Vec<SpecSystemLink>,
    artifact: &DocArtifact,
    ledger: &DocArtifactLedger,
) {
    for (field, value) in [
        ("linked_proposal", artifact.linked_proposal.as_deref()),
        ("linked_spec", artifact.linked_spec.as_deref()),
        ("linked_adr", artifact.linked_adr.as_deref()),
        ("linked_plan", artifact.linked_plan.as_deref()),
        ("linked_goal", artifact.linked_goal.as_deref()),
        (
            "linked_support_tier",
            artifact.linked_support_tier.as_deref(),
        ),
        ("linked_closeout", artifact.linked_closeout.as_deref()),
        ("supersedes", artifact.supersedes.as_deref()),
        ("superseded_by", artifact.superseded_by.as_deref()),
        ("replaces", artifact.replaces.as_deref()),
    ] {
        let Some(target) = value.filter(|target| !target.trim().is_empty()) else {
            continue;
        };
        links.push(SpecSystemLink {
            source_id: artifact.id.clone(),
            field,
            target: target.to_string(),
            target_kind: resolve_target_kind(ledger, target),
        });
    }
}

fn resolve_target_kind(ledger: &DocArtifactLedger, target: &str) -> Option<&'static str> {
    ledger
        .artifact
        .iter()
        .find(|artifact| {
            artifact.id == target
                || normalize_source_path(&artifact.path) == normalize_source_path(target)
        })
        .map(|artifact| artifact_kind_name(artifact.kind))
}

fn artifact_kind_name(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Proposal => "proposal",
        ArtifactKind::Spec => "spec",
        ArtifactKind::Adr => "adr",
        ArtifactKind::ImplementationPlan => "implementation_plan",
        ArtifactKind::PlanItem => "plan_item",
        ArtifactKind::ActiveGoal => "active_goal",
        ArtifactKind::SupportTier => "support_tier",
        ArtifactKind::PolicyLedger => "policy_ledger",
        ArtifactKind::Closeout => "closeout",
        ArtifactKind::ReleaseRecord => "release_record",
    }
}

fn artifact_status_name(status: ArtifactStatus) -> &'static str {
    match status {
        ArtifactStatus::Draft => "draft",
        ArtifactStatus::Proposed => "proposed",
        ArtifactStatus::Accepted => "accepted",
        ArtifactStatus::Active => "active",
        ArtifactStatus::Done => "done",
        ArtifactStatus::Superseded => "superseded",
    }
}

fn normalize_source_path(path: &str) -> String {
    path.trim_matches('/').replace('\\', "/")
}

fn render_string_array(text: &mut String, values: &[&str], indent: &str) {
    text.push_str("[\n");
    for (index, value) in values.iter().enumerate() {
        text.push_str(indent);
        text.push_str("  \"");
        text.push_str(&json_escape(value));
        text.push('"');
        if index + 1 != values.len() {
            text.push(',');
        }
        text.push('\n');
    }
    text.push_str(indent);
    text.push(']');
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

#[cfg(test)]
pub(crate) fn sample_spec_system_json_for_contract_test() -> String {
    let report = SpecSystemReport {
        command: "check".to_string(),
        root: PathBuf::from("H:/Code/Rust/cargo-allow"),
        config_source: "policy/spec-system.toml".to_string(),
        artifacts: vec![
            SpecSystemArtifact {
                id: "CARGO-ALLOW-PROP-0001".to_string(),
                kind: "proposal",
                path: "docs/proposals/CARGO-ALLOW-PROP-0001-spec-system-profile.md".to_string(),
                status: "accepted",
                owner: "repo-infra".to_string(),
                created: "2026-06-12".to_string(),
            },
            SpecSystemArtifact {
                id: "CARGO-ALLOW-SPEC-0001".to_string(),
                kind: "spec",
                path: "docs/specs/CARGO-ALLOW-SPEC-0001-spec-system-profile.md".to_string(),
                status: "accepted",
                owner: "repo-infra".to_string(),
                created: "2026-06-12".to_string(),
            },
        ],
        links: vec![SpecSystemLink {
            source_id: "CARGO-ALLOW-SPEC-0001".to_string(),
            field: "linked_proposal",
            target: "CARGO-ALLOW-PROP-0001".to_string(),
            target_kind: Some("proposal"),
        }],
        support_tier_rows: 1,
        findings: vec![SpecSystemFinding {
            kind: "artifact_link",
            message: "example structural graph finding".to_string(),
        }],
    };
    render_spec_system_json(&report)
}
