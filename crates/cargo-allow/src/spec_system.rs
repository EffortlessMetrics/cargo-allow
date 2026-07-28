use allow_core::{CargoAllowError, CargoAllowResult, read_text_file_capped};
use allow_inventory::resolve_source_tree_root;
use allow_policy::federation::{
    FederationLoadOutcome, evaluate_spec_system_ledger, load_federation_config,
};
use allow_policy::import_roots::{
    ImportDiagnosticKind, ImportGraph, discover_import_graph, resolve_spec_system_import_roots,
    validate_import_roots_config,
};
use allow_policy::spec_system::{
    ArtifactKind, ArtifactStatus, DocArtifact, DocArtifactLedger, ProfileConfigProvenance,
    ResolvedProfileConfig, SpecSystemConfig, SpecSystemGeneration, SpecSystemMode,
    SpecSystemRequirements, SpecSystemRoots, SupportTierLevel, load_doc_artifacts,
    parse_spec_system_config_at, parse_support_tier_claims, profile_config_conflict_message,
    resolve_profile_config, validate_active_goal_manifest_text_at, validate_doc_artifact_files,
    validate_doc_artifact_links, validate_support_tier_claims,
};
use std::fs;
use std::path::{Path, PathBuf};

use crate::spec_system_view::render_self_hosted_explain;
use crate::{OutputFormat, RootArgs, current_dir, emit_text, root_relative_path, write_file};

const PROFILE_NAME: &str = "spec-system";
const DEFAULT_OWNED_ARTIFACT_LEDGER: &str = ".allow/artifacts/doc-artifacts.toml";
const DEFAULT_OWNED_IMPORTS_ROOT: &str = ".allow/imports";
const DEFAULT_PROFILE_CONFIG: &str = ".allow/profiles/spec-system.toml";
const SPEC_SYSTEM_CHECK_PROOF_COMMAND: &str =
    "cargo-allow check --profile spec-system --mode audit";
const SPEC_SYSTEM_WORKLIST_PROOF_COMMAND: &str =
    "cargo-allow worklist --profile spec-system --format json";
const EXPECTED_TEMPLATE_FILES: [&str; 7] = [
    "docs/templates/proposal.md",
    "docs/templates/spec.md",
    "docs/templates/adr.md",
    "docs/templates/implementation-plan.md",
    "docs/templates/plan-item.md",
    "docs/templates/closeout.md",
    "docs/templates/pr-body.md",
];

pub(crate) struct SpecSystemCommandArgs<'a> {
    pub(crate) command: &'a str,
    pub(crate) root: &'a RootArgs,
    pub(crate) config: Option<&'a Path>,
    pub(crate) format: OutputFormat,
    pub(crate) output: Option<&'a Path>,
    pub(crate) receipt: Option<&'a Path>,
    /// Explicit `--mode` value, if the operator passed one. Overrides the
    /// config mode; an unrecognized value fails closed (#1941).
    pub(crate) mode: Option<&'a str>,
}

fn reject_cutover_embedded_authority(root: &RootArgs, surface: &str) -> CargoAllowResult<()> {
    let cwd =
        current_dir()?;
    let resolved = resolve_source_tree_root(root.root.as_deref(), cwd)?;
    crate::intent_delegate::reject_embedded_spec_system_authority(&resolved, surface)
}

pub(crate) fn cmd_spec_system(args: SpecSystemCommandArgs<'_>) -> CargoAllowResult<()> {
    reject_cutover_embedded_authority(args.root, args.command)?;
    let mode_override = args.mode.map(parse_spec_system_mode_override).transpose()?;
    let report = build_spec_system_report(
        args.command,
        args.root,
        args.config,
        false,
        false,
        mode_override,
    )?;
    let rendered = render_spec_system_report(&report, args.format);
    emit_text(args.output, &rendered)?;
    if let Some(path) = args.receipt {
        write_file(path, &render_spec_system_json(&report))?;
    }
    if spec_system_command_failed(&report) {
        return Err(CargoAllowError::new(format!(
            "spec-system blocking findings found: {}",
            spec_system_blocking_finding_count(&report)
        )));
    }
    Ok(())
}

pub(crate) struct SpecSystemWorklistCommandArgs<'a> {
    pub(crate) root: &'a RootArgs,
    pub(crate) config: Option<&'a Path>,
    pub(crate) format_json: bool,
    pub(crate) output: Option<&'a Path>,
}

pub(crate) fn cmd_spec_system_worklist(
    args: SpecSystemWorklistCommandArgs<'_>,
) -> CargoAllowResult<()> {
    reject_cutover_embedded_authority(args.root, "worklist")?;
    let report = build_spec_system_report("worklist", args.root, args.config, true, false, None)?;
    let rendered = if args.format_json {
        render_spec_system_json(&report)
    } else {
        render_spec_system_markdown(&report)
    };
    emit_text(args.output, &rendered)
}

pub(crate) struct SpecSystemDoctorCommandArgs<'a> {
    pub(crate) root: &'a RootArgs,
    pub(crate) config: Option<&'a Path>,
    pub(crate) format_json: bool,
    pub(crate) output: Option<&'a Path>,
}

pub(crate) fn cmd_spec_system_doctor(
    args: SpecSystemDoctorCommandArgs<'_>,
) -> CargoAllowResult<()> {
    reject_cutover_embedded_authority(args.root, "doctor")?;
    let report = build_spec_system_report("doctor", args.root, args.config, true, true, None)?;
    let rendered = if args.format_json {
        render_spec_system_json(&report)
    } else {
        render_spec_system_markdown(&report)
    };
    emit_text(args.output, &rendered)
}

pub(crate) struct SpecSystemExplainCommandArgs<'a> {
    pub(crate) artifact_id: &'a str,
    pub(crate) root: &'a RootArgs,
    pub(crate) config: Option<&'a Path>,
    pub(crate) format_json: bool,
    pub(crate) output: Option<&'a Path>,
}

pub(crate) fn cmd_spec_system_explain(
    args: SpecSystemExplainCommandArgs<'_>,
) -> CargoAllowResult<()> {
    reject_cutover_embedded_authority(args.root, "explain")?;
    let report = build_spec_system_report("explain", args.root, args.config, true, false, None)?;
    if let Some(rendered) =
        render_self_hosted_explain(&report.root, args.artifact_id, args.format_json)?
    {
        emit_text(args.output, &rendered)?;
        return Ok(());
    }
    let report = filter_spec_system_report_for_artifact(&report, args.artifact_id)?;
    let rendered = if args.format_json {
        render_spec_system_json(&report)
    } else {
        render_spec_system_explain_markdown(&report)
    };
    emit_text(args.output, &rendered)
}

pub(crate) struct SpecSystemInitCommandArgs<'a> {
    pub(crate) root: &'a RootArgs,
    pub(crate) config: Option<&'a Path>,
    pub(crate) force: bool,
    pub(crate) dry_run: bool,
}

pub(crate) fn cmd_spec_system_init(args: SpecSystemInitCommandArgs<'_>) -> CargoAllowResult<()> {
    reject_cutover_embedded_authority(args.root, "init")?;
    let cwd =
        current_dir()?;
    let root = resolve_source_tree_root(args.root.root.as_deref(), cwd)?;
    let config_path = args
        .config
        .unwrap_or_else(|| Path::new(DEFAULT_PROFILE_CONFIG));
    let legacy_compatibility = spec_system_legacy_compatibility(&root, config_path)?;
    if !legacy_compatibility {
        let conflicts = legacy_bootstrap_conflicts(&root);
        if args.dry_run {
            for conflict in &conflicts {
                println!(
                    "conflict {}: current bootstrap leaves legacy active-goal state untouched",
                    root_relative_display(&root, conflict)
                );
            }
        } else if let Some(conflict) = conflicts.first() {
            return Err(CargoAllowError::new(format!(
                "current spec-system bootstrap will not overwrite legacy active-goal state at {}; choose an explicit legacy-v1 profile or migrate it first",
                root_relative_display(&root, conflict)
            )));
        }
    }
    let files = spec_system_bootstrap_files(config_path, legacy_compatibility);

    for file in files {
        let path = root_relative_path(&root, &file.path);
        let display = root_relative_display(&root, &path);
        if args.dry_run {
            let action = if path.exists() && args.force {
                "would overwrite"
            } else if path.exists() {
                "would keep"
            } else {
                "would create"
            };
            println!("{action} {display}");
            continue;
        }
        if path.exists() && !args.force {
            println!("kept {display}");
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                CargoAllowError::new(format!("failed to create {}: {e}", parent.display()))
            })?;
        }
        fs::write(&path, file.contents).map_err(|e| {
            CargoAllowError::new(format!("failed to write {}: {e}", path.display()))
        })?;
        let action = if args.force { "wrote" } else { "created" };
        println!("{action} {display}");
    }

    // After the file loop, emit next-steps guidance in both dry-run and write
    // paths so the spec-system init experience matches the default-profile
    // init (which prints next steps via init.rs::next_steps_block). The
    // starter-policy preview is intentionally omitted here because spec-system
    // init writes .allow/profiles/spec-system.toml, not policy/allow.toml.
    println!();
    print!("{}", crate::init::next_steps_block());

    Ok(())
}

#[derive(Debug)]
struct SpecSystemReport {
    command: String,
    root: PathBuf,
    config_source: String,
    config_provenance: String,
    mode: SpecSystemMode,
    artifacts: Vec<SpecSystemArtifact>,
    links: Vec<SpecSystemLink>,
    support_tier_rows: usize,
    findings: Vec<SpecSystemFinding>,
    work_items: Vec<SpecSystemWorkItem>,
    readiness: Option<SpecSystemReadiness>,
    federation: Option<SpecSystemFederationSummary>,
    import_graph: Option<SpecSystemImportGraphSummary>,
}

#[derive(Debug, Clone)]
struct SpecSystemFederationSummary {
    federation_version: String,
    precedence_applied: String,
    ledger_contributors: Vec<SpecSystemLedgerContributor>,
}

#[derive(Debug, Clone)]
struct SpecSystemLedgerContributor {
    id: String,
    path: String,
    role: String,
    dialect: String,
    mode: String,
    priority: u32,
    lanes: Vec<String>,
}

#[derive(Debug, Clone)]
struct SpecSystemImportGraphSummary {
    node_count: usize,
    edge_count: usize,
    diagnostic_count: usize,
    nodes: Vec<SpecSystemImportNode>,
    edges: Vec<SpecSystemImportEdge>,
    diagnostics: Vec<SpecSystemImportDiagnostic>,
}

#[derive(Debug, Clone)]
struct SpecSystemImportNode {
    id: String,
    path: String,
    role: String,
    ecosystem: String,
    provenance: String,
    confidence: String,
}

#[derive(Debug, Clone)]
struct SpecSystemImportEdge {
    source_id: String,
    target_id: String,
    kind: String,
    provenance: String,
}

#[derive(Debug, Clone)]
struct SpecSystemImportDiagnostic {
    kind: String,
    message: String,
    root_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct SpecSystemArtifact {
    id: String,
    kind: &'static str,
    path: String,
    status: &'static str,
    owner: String,
    created: String,
}

#[derive(Debug, Clone)]
struct SpecSystemLink {
    source_id: String,
    field: &'static str,
    target: String,
    target_kind: Option<&'static str>,
}

#[derive(Debug, Clone)]
struct SpecSystemFinding {
    kind: &'static str,
    message: String,
    blocking_eligible: bool,
    blocking_reason: Option<&'static str>,
}

impl SpecSystemFinding {
    fn new(kind: &'static str, message: String) -> Self {
        let blocking_reason = spec_system_blocking_reason(kind, &message);
        Self {
            kind,
            message,
            blocking_eligible: blocking_reason.is_some(),
            blocking_reason,
        }
    }
}

#[derive(Debug, Clone)]
struct SpecSystemWorkItem {
    kind: &'static str,
    artifact_id: Option<String>,
    path: Option<String>,
    owner: Option<String>,
    status: Option<String>,
    message: String,
    suggested_actions: Vec<String>,
    proof_commands: Vec<String>,
    ledger_id: Option<String>,
    ledger_path: Option<String>,
    lane: Option<String>,
    mode: Option<String>,
    role: Option<String>,
}

#[derive(Debug)]
struct SpecSystemReadiness {
    ready: bool,
    mode: &'static str,
    checks: Vec<SpecSystemReadinessCheck>,
}

#[derive(Debug)]
struct SpecSystemReadinessCheck {
    kind: &'static str,
    path: Option<String>,
    found: bool,
    valid: Option<bool>,
    status: &'static str,
    message: String,
}

#[derive(Debug)]
struct LoadedSpecSystemConfig {
    cfg: SpecSystemConfig,
    source: String,
    provenance: ProfileConfigProvenance,
    path: String,
    found: bool,
    valid: Option<bool>,
    diagnostic: Option<String>,
    resolved: ResolvedProfileConfig,
}

struct SpecSystemBootstrapFile {
    path: PathBuf,
    contents: String,
}

fn spec_system_bootstrap_files(
    config_path: &Path,
    legacy_compatibility: bool,
) -> Vec<SpecSystemBootstrapFile> {
    let mut files = vec![
        bootstrap_file(
            config_path,
            spec_system_config_template(legacy_compatibility),
        ),
        bootstrap_file(
            Path::new(DEFAULT_OWNED_ARTIFACT_LEDGER),
            doc_artifacts_template(),
        ),
        bootstrap_file(
            Path::new("docs/proposals/README.md"),
            artifact_root_readme("Proposals", "why work exists and what user value it serves"),
        ),
        bootstrap_file(
            Path::new("docs/specs/README.md"),
            artifact_root_readme("Specs", "required behavior, evidence, and claim boundaries"),
        ),
        bootstrap_file(
            Path::new("docs/adr/README.md"),
            artifact_root_readme("ADRs", "durable architecture decisions"),
        ),
        bootstrap_file(
            Path::new("plans/README.md"),
            artifact_root_readme("Plans", "PR-sized execution sequences and rollback notes"),
        ),
        bootstrap_file(Path::new(".allow/imports/README.md"), imports_root_readme()),
        bootstrap_file(
            Path::new("docs/status/SUPPORT_TIERS.md"),
            support_tiers_template(),
        ),
    ];

    files.extend(
        EXPECTED_TEMPLATE_FILES
            .iter()
            .map(|path| bootstrap_file(Path::new(path), template_contents(path))),
    );

    if legacy_compatibility {
        files.splice(
            6..6,
            [
                bootstrap_file(
                    Path::new(".allow/goals/README.md"),
                    artifact_root_readme(
                        "Legacy Active Goals",
                        "historical compatibility metadata only; not current work authority",
                    ),
                ),
                bootstrap_file(
                    Path::new(".allow/goals/active.toml"),
                    active_goal_template(),
                ),
                bootstrap_file(Path::new(".allow/goals/archive/.gitkeep"), String::new()),
            ],
        );
    }
    files
}

fn bootstrap_file(path: &Path, contents: String) -> SpecSystemBootstrapFile {
    SpecSystemBootstrapFile {
        path: path.to_path_buf(),
        contents,
    }
}

fn spec_system_config_template(legacy_compatibility: bool) -> String {
    if legacy_compatibility {
        return r#"schema_version = "1.0"
profile = "spec-system"
mode = "advisory"
generation = "legacy-v1"

[roots]
proposals = "docs/proposals"
specs = "docs/specs"
adrs = "docs/adr"
plans = "plans"
goals = ".allow/goals"
support_tiers = "docs/status/SUPPORT_TIERS.md"
artifact_ledger = ".allow/artifacts/doc-artifacts.toml"

[requirements]
ledger_required = true
templates_required = true
support_tiers_required = true
# Legacy active-goal compatibility is explicit and historical-only. It cannot
# select current work or authorize mutation, implementation, or support state.
active_goal_required = false
closeout_required_for_done_items = true

[import_roots]
owned = ".allow/imports"

[[import_roots.entries]]
id = "owned-imports"
path = ".allow/imports"
ecosystem = "cargo-allow"
role = "owned"
"#
        .to_string();
    }

    r#"schema_version = "1.0"
profile = "spec-system"
mode = "advisory"
generation = "current-v2"

[roots]
proposals = "docs/proposals"
specs = "docs/specs"
adrs = "docs/adr"
plans = "plans"
support_tiers = "docs/status/SUPPORT_TIERS.md"
artifact_ledger = ".allow/artifacts/doc-artifacts.toml"

[requirements]
ledger_required = true
templates_required = true
support_tiers_required = true
closeout_required_for_done_items = true

[import_roots]
owned = ".allow/imports"

[[import_roots.entries]]
id = "owned-imports"
path = ".allow/imports"
ecosystem = "cargo-allow"
role = "owned"
"#
    .to_string()
}

fn spec_system_legacy_compatibility(root: &Path, config_path: &Path) -> CargoAllowResult<bool> {
    let path = root_relative_path(root, config_path);
    if !path.is_file() {
        return Ok(false);
    }
    let text = read_text_file_capped(&path).map_err(|error| {
        CargoAllowError::new(format!(
            "failed to read existing spec-system profile config {}: {error}",
            path.display()
        ))
    })?;
    let config = parse_spec_system_config_at(Some(&path), &text)?;
    Ok(matches!(config.generation, SpecSystemGeneration::LegacyV1))
}

fn legacy_bootstrap_conflicts(root: &Path) -> Vec<PathBuf> {
    [
        ".allow/goals",
        ".allow/goals/README.md",
        ".allow/goals/active.toml",
        ".allow/goals/archive/.gitkeep",
    ]
    .into_iter()
    .map(Path::new)
    .map(|path| root_relative_path(root, path))
    .filter(|path| path.exists())
    .collect()
}

fn doc_artifacts_template() -> String {
    r#"schema_version = "1.0"
policy = "cargo-allow-doc-artifacts"
owner = "repo-infra"
status = "advisory"
"#
    .to_string()
}

fn artifact_root_readme(title: &str, role: &str) -> String {
    format!(
        "# {title}\n\nThis directory contains spec-system artifacts for {role}.\n\nRegister governed artifacts in `{DEFAULT_OWNED_ARTIFACT_LEDGER}` so `cargo-allow check --profile spec-system` can validate their source-tree graph links.\n"
    )
}

fn imports_root_readme() -> String {
    r#"# Import Roots

External spec ecosystems discovered under import roots are read-only by default.
cargo-allow does not rewrite imported files unless explicitly promoted.

Place import adapters and discovery notes here when the repository adopts
external spec systems such as Kiro, Spec Kit, or generic `.spec/` trees.
"#
    .to_string()
}

fn active_goal_template() -> String {
    r#"schema_version = "1.0"

# Placeholder execution state for explicit legacy compatibility only.
# This file is historical/read-only metadata and is not current work authority.
id = "spec-system-profile"
title = "Spec-system profile"
status = "active"
owner = "codex"
created = "YYYY-MM-DD"

objective = """
Keep proposals, specs, ADRs, implementation plans, active goals, support tiers,
policy ledgers, and closeouts linked and linted.
"""

linked_plan = "plans/spec-system/implementation-plan.md"

[[work_item]]
id = "spec-system-pr-001"
status = "ready"
title = "Register source-of-truth artifacts"
proof_commands = [
  "cargo-allow check --profile spec-system --mode audit",
  "cargo-allow worklist --profile spec-system --format json",
]
"#
    .to_string()
}

fn support_tiers_template() -> String {
    r#"# Support Tiers

| Surface | Tier | Claim | Proof command | Notes |
| --- | --- | --- | --- | --- |
| Spec-system profile | Advisory | Source-of-truth graph artifacts can be linted. | cargo-allow check --profile spec-system --mode audit | Opt-in profile; structural validation only. |
"#
    .to_string()
}

fn template_contents(path: &str) -> String {
    let (id, kind, title) = match path {
        "docs/templates/proposal.md" => ("CARGO-ALLOW-PROP-0000", "proposal", "Proposal"),
        "docs/templates/spec.md" => ("CARGO-ALLOW-SPEC-0000", "spec", "Spec"),
        "docs/templates/adr.md" => ("CARGO-ALLOW-ADR-0000", "adr", "ADR"),
        "docs/templates/implementation-plan.md" => (
            "CARGO-ALLOW-PLAN-0000",
            "implementation_plan",
            "Implementation Plan",
        ),
        "docs/templates/plan-item.md" => ("CARGO-ALLOW-ITEM-0000", "plan_item", "Plan Item"),
        "docs/templates/closeout.md" => ("CARGO-ALLOW-CLOSEOUT-0000", "closeout", "Closeout"),
        "docs/templates/pr-body.md" => ("CARGO-ALLOW-PR-0000", "release_record", "PR Body"),
        _ => ("CARGO-ALLOW-ARTIFACT-0000", "artifact", "Artifact"),
    };
    format!(
        r#"---
id: {id}
kind: {kind}
status: draft
owner: repo-infra
created: YYYY-MM-DD
---

# {title}: Title

## Purpose

State the artifact's job in the source-of-truth graph.

## Links

- Linked proposal:
- Linked spec:
- Linked plan:

## Required Evidence

- Proof command or artifact:

## Claim Boundary

Structural source-tree graph metadata only. This artifact does not prove command
execution or semantic correctness by itself.

## Rollback

Describe how to supersede, withdraw, or close this artifact.
"#
    )
}

fn build_spec_system_report(
    command: &str,
    root_args: &RootArgs,
    config: Option<&Path>,
    include_work_items: bool,
    include_readiness: bool,
    mode_override: Option<SpecSystemMode>,
) -> CargoAllowResult<SpecSystemReport> {
    let cwd =
        current_dir()?;
    let root = resolve_source_tree_root(root_args.root.as_deref(), cwd)?;
    let loaded_config = load_spec_system_config(&root, config);
    let mut cfg = loaded_config.cfg.clone();
    // An explicit `--mode` overrides the config mode (mirrors source-tree
    // `check`), so `--mode blocking`/`--mode audit` are honored instead of
    // silently dropped (#1941).
    if let Some(mode) = mode_override {
        cfg.mode = mode;
    }
    let config_source = loaded_config.source.clone();
    let config_provenance = loaded_config.provenance.as_str().to_string();
    let mut findings = profile_config_findings(&loaded_config, config.is_some());
    if let Some(message) = profile_config_conflict_message(&loaded_config.resolved) {
        findings.push(SpecSystemFinding::new("profile_config", message));
    }
    findings.extend(federation_config_findings(&root));
    let mut artifacts = Vec::new();
    let mut links = Vec::new();
    let mut support_tier_rows = 0;
    let mut work_items = Vec::new();

    if matches!(cfg.generation, SpecSystemGeneration::CurrentV2) {
        let legacy_active_goal = root.join(".allow/goals/active.toml");
        if legacy_active_goal.is_file() {
            let message = format!(
                "legacy active goal manifest {} is historical-only; it cannot select current work, authorize mutation, or promote implementation/support state",
                legacy_active_goal
                    .strip_prefix(&root)
                    .unwrap_or(&legacy_active_goal)
                    .display()
            );
            findings.push(SpecSystemFinding::new(
                "legacy_active_goal_present",
                message.clone(),
            ));
            if include_work_items {
                work_items.push(SpecSystemWorkItem {
                    kind: "legacy_goal_historical_only",
                    artifact_id: None,
                    path: Some(".allow/goals/active.toml".to_string()),
                    owner: None,
                    status: Some("historical_only".to_string()),
                    message,
                    suggested_actions: vec![
                        "archive or remove the legacy active-goal file after preserving its closeout history"
                            .to_string(),
                        "do not use the legacy file as a current issue, implementation, or mutation authority"
                            .to_string(),
                    ],
                    proof_commands: spec_system_proof_commands(),
                    ledger_id: None,
                    ledger_path: None,
                    lane: Some("migration".to_string()),
                    mode: Some("advisory".to_string()),
                    role: Some("legacy".to_string()),
                });
            }
        }
    }

    let ledger_path = root_relative_path(&root, Path::new(&cfg.roots.artifact_ledger));
    match load_doc_artifacts(&ledger_path) {
        Ok(ledger) => {
            artifacts = collect_artifacts(&ledger);
            links = collect_links(&ledger);
            if include_work_items {
                work_items.extend(work_items_from_artifact_files(&root, &ledger));
                work_items.extend(work_items_from_artifact_links(&ledger));
                work_items.extend(work_items_from_missing_closeouts(
                    &ledger,
                    cfg.requirements.closeout_required_for_done_items,
                ));
            }
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
            if cfg.requirements.active_goal_required {
                let active_goal_result = validate_active_goal_file(&root, &cfg, &ledger);
                if let Err(err) = active_goal_result {
                    let message = err.to_string();
                    findings.push(SpecSystemFinding::new("active_goal", message.clone()));
                    if include_work_items {
                        let active_goal_path = active_goal_manifest_source_path(&cfg)
                            .unwrap_or_else(|| ".allow/goals/active.toml".to_string());
                        work_items.push(active_goal_work_item(&active_goal_path, &message));
                    }
                }
            }
        }
        Err(err) => {
            let message = err.to_string();
            findings.push(SpecSystemFinding::new(
                "doc_artifact_ledger",
                message.clone(),
            ));
            if include_work_items && cfg.requirements.ledger_required {
                work_items.push(missing_node_work_item(
                    "doc artifact ledger",
                    &cfg.roots.artifact_ledger,
                    &message,
                    vec![
                        format!(
                            "create {} with registered source-of-truth artifacts",
                            cfg.roots.artifact_ledger
                        ),
                        "or correct the configured artifact_ledger path in the spec-system profile config"
                            .to_string(),
                    ],
                ));
            }
        }
    }

    let support_tiers_path = root_relative_path(&root, Path::new(&cfg.roots.support_tiers));
    match read_text_file_capped(&support_tiers_path) {
        Ok(text) => match parse_support_tier_claims(&text) {
            Ok(rows) => {
                support_tier_rows = rows.len();
                if include_work_items {
                    work_items.extend(work_items_from_support_tiers(
                        &cfg.roots.support_tiers,
                        &rows,
                    ));
                }
                if let Err(err) = validate_support_tier_claims(&text) {
                    findings.push(SpecSystemFinding::new("support_tier", err.to_string()));
                }
            }
            Err(err) => {
                findings.push(SpecSystemFinding::new("support_tier", err.to_string()));
                if include_work_items && cfg.requirements.support_tiers_required {
                    work_items.push(SpecSystemWorkItem {
                        kind: "missing_support_tier",
                        artifact_id: None,
                        path: Some(cfg.roots.support_tiers.clone()),
                        owner: None,
                        status: None,
                        message: "support-tier claims table is missing or invalid".to_string(),
                        suggested_actions: vec![
                            "add a support-tier table with Surface, Tier, Claim, Proof command, and Notes columns"
                                .to_string(),
                            "or correct the configured support_tiers path in the spec-system profile config"
                                .to_string(),
                        ],
                        proof_commands: spec_system_proof_commands(),
                        ledger_id: None,
                        ledger_path: None,
                        lane: None,
                        mode: None,
                        role: None,
                    });
                }
            }
        },
        Err(err) => {
            let message = format!(
                "failed to read support-tier file {}: {err}",
                cfg.roots.support_tiers
            );
            findings.push(SpecSystemFinding::new("support_tier", message.clone()));
            if include_work_items && cfg.requirements.support_tiers_required {
                work_items.push(SpecSystemWorkItem {
                    kind: "missing_support_tier",
                    artifact_id: None,
                    path: Some(cfg.roots.support_tiers.clone()),
                    owner: None,
                    status: None,
                    message,
                    suggested_actions: vec![
                        "create docs/status/SUPPORT_TIERS.md with claim-to-proof rows".to_string(),
                        "or correct the configured support_tiers path in the spec-system profile config"
                            .to_string(),
                    ],
                    proof_commands: spec_system_proof_commands(),
                    ledger_id: None,
                    ledger_path: None,
                    lane: None,
                    mode: None,
                    role: None,
                });
            }
        }
    }

    if include_work_items {
        work_items.extend(work_items_from_config_findings(&findings));
    }
    let import_config = resolve_spec_system_import_roots(cfg.import_roots.as_ref());
    let validated_import_roots = validate_import_roots_config(import_config);
    let import_graph = discover_import_graph(&root, &validated_import_roots);
    findings.extend(import_graph_findings(&import_graph));
    if include_work_items {
        work_items.extend(work_items_from_import_graph(&import_graph));
    }
    let import_graph_summary = Some(import_graph_summary_from_graph(&import_graph));
    let federation = evaluate_spec_system_ledger(&root).map(|evaluation| {
        if let Some(provenance) = &evaluation.active_provenance {
            apply_work_item_ledger_provenance(&mut work_items, provenance);
        }
        SpecSystemFederationSummary {
            federation_version: evaluation.federation_version.to_string(),
            precedence_applied: evaluation.precedence_applied.as_str().to_string(),
            ledger_contributors: evaluation
                .ledger_contributors
                .iter()
                .map(|contributor| SpecSystemLedgerContributor {
                    id: contributor.id.clone(),
                    path: contributor.path.clone(),
                    role: contributor.role.as_str().to_string(),
                    dialect: contributor.dialect.clone(),
                    mode: contributor.mode.as_str().to_string(),
                    priority: contributor.priority,
                    lanes: contributor.lanes.clone(),
                })
                .collect(),
        }
    });
    let readiness = if include_readiness {
        Some(collect_spec_system_readiness(&root, &loaded_config))
    } else {
        None
    };

    Ok(SpecSystemReport {
        command: command.to_string(),
        root,
        config_source,
        config_provenance,
        mode: cfg.mode,
        artifacts,
        links,
        support_tier_rows,
        findings,
        work_items,
        readiness,
        federation,
        import_graph: import_graph_summary,
    })
}

fn load_spec_system_config(root: &Path, config: Option<&Path>) -> LoadedSpecSystemConfig {
    let resolved = resolve_profile_config(root, PROFILE_NAME, config);
    let provenance = resolved.provenance;
    let config_path_text = resolved
        .path
        .clone()
        .unwrap_or_else(|| DEFAULT_PROFILE_CONFIG.to_string());
    let config_path = root_relative_path(root, Path::new(&config_path_text));
    let source = match provenance {
        ProfileConfigProvenance::BuiltInDefault => "default spec-system roots".to_string(),
        _ => config_path_text.clone(),
    };

    if provenance == ProfileConfigProvenance::BuiltInDefault {
        return LoadedSpecSystemConfig {
            cfg: default_spec_system_config(),
            source,
            provenance,
            path: config_path_text,
            found: false,
            valid: None,
            diagnostic: Some(format!(
                "spec-system profile config {} does not exist",
                config_path.display()
            )),
            resolved,
        };
    }

    if !config_path.exists() {
        return LoadedSpecSystemConfig {
            cfg: default_spec_system_config(),
            source: "default spec-system roots".to_string(),
            provenance,
            path: config_path_text,
            found: false,
            valid: None,
            diagnostic: Some(format!(
                "spec-system profile config {} does not exist",
                config_path.display()
            )),
            resolved,
        };
    }

    match read_text_file_capped(&config_path) {
        Ok(text) => match parse_spec_system_config_at(Some(&config_path), &text) {
            Ok(cfg) => LoadedSpecSystemConfig {
                cfg,
                source: config_path_text.clone(),
                provenance,
                path: config_path_text,
                found: true,
                valid: Some(true),
                diagnostic: None,
                resolved,
            },
            Err(err) => LoadedSpecSystemConfig {
                cfg: default_spec_system_config(),
                source: "default spec-system roots".to_string(),
                provenance,
                path: config_path_text,
                found: true,
                valid: Some(false),
                diagnostic: Some(err.to_string()),
                resolved,
            },
        },
        Err(err) => LoadedSpecSystemConfig {
            cfg: default_spec_system_config(),
            source: "default spec-system roots".to_string(),
            provenance,
            path: config_path_text,
            found: true,
            valid: Some(false),
            diagnostic: Some(format!(
                "failed to read spec-system profile config {}: {err}",
                config_path.display()
            )),
            resolved,
        },
    }
}

fn profile_config_findings(
    loaded: &LoadedSpecSystemConfig,
    explicit_config: bool,
) -> Vec<SpecSystemFinding> {
    if loaded.valid == Some(false) || (explicit_config && !loaded.found) {
        return vec![SpecSystemFinding::new(
            "profile_config",
            loaded
                .diagnostic
                .clone()
                .unwrap_or_else(|| "spec-system profile config is invalid".to_string()),
        )];
    }
    Vec::new()
}

fn federation_config_findings(root: &Path) -> Vec<SpecSystemFinding> {
    let Ok(loaded) = load_federation_config(root) else {
        return Vec::new();
    };
    let FederationLoadOutcome::Parsed(validated) = loaded.outcome else {
        return Vec::new();
    };
    validated
        .diagnostics
        .into_iter()
        .filter(|diagnostic| {
            !matches!(
                diagnostic.kind,
                allow_policy::federation::FederationDiagnosticKind::DialectSkipped
            )
        })
        .map(|diagnostic| {
            SpecSystemFinding::new(
                "federation_config",
                format!("{}: {}", diagnostic.kind.as_str(), diagnostic.message),
            )
        })
        .collect()
}

fn import_graph_findings(graph: &ImportGraph) -> Vec<SpecSystemFinding> {
    graph
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.kind != ImportDiagnosticKind::MissingRoot)
        .map(|diagnostic| {
            SpecSystemFinding::new(
                "import_graph",
                format!("{}: {}", diagnostic.kind.as_str(), diagnostic.message),
            )
        })
        .collect()
}

fn work_items_from_import_graph(graph: &ImportGraph) -> Vec<SpecSystemWorkItem> {
    graph
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.kind != ImportDiagnosticKind::MissingRoot)
        .map(|diagnostic| {
            let kind = match diagnostic.kind {
                ImportDiagnosticKind::MissingRoot => "missing_import_root",
                ImportDiagnosticKind::BrokenEdge => "broken_import",
                ImportDiagnosticKind::DuplicateRootId
                | ImportDiagnosticKind::DuplicateRootPath
                | ImportDiagnosticKind::UnknownRole
                | ImportDiagnosticKind::InvalidRootPath => "broken_import",
            };
            let path = diagnostic
                .root_ids
                .first()
                .cloned()
                .or_else(|| Some(DEFAULT_OWNED_IMPORTS_ROOT.to_string()));
            SpecSystemWorkItem {
                kind,
                artifact_id: None,
                path,
                owner: Some("repo-infra".to_string()),
                status: Some(diagnostic.kind.as_str().to_string()),
                message: diagnostic.message.clone(),
                suggested_actions: import_graph_suggested_actions(diagnostic.kind),
                proof_commands: spec_system_proof_commands(),
                ledger_id: None,
                ledger_path: None,
                lane: Some("import".to_string()),
                mode: Some("advisory".to_string()),
                role: Some("imported".to_string()),
            }
        })
        .collect()
}

fn import_graph_suggested_actions(kind: ImportDiagnosticKind) -> Vec<String> {
    match kind {
        ImportDiagnosticKind::MissingRoot => vec![
            "create the configured import root directory".to_string(),
            "or remove the unused import root entry from the spec-system profile config"
                .to_string(),
        ],
        ImportDiagnosticKind::BrokenEdge => vec![
            "fix the broken import reference in the foreign file".to_string(),
            "or promote the import node into the owned artifact ledger".to_string(),
        ],
        ImportDiagnosticKind::DuplicateRootId | ImportDiagnosticKind::DuplicateRootPath => vec![
            "deduplicate import root ids and paths in the spec-system profile config".to_string(),
        ],
        ImportDiagnosticKind::UnknownRole => {
            vec!["use an import root role of owned, imported, legacy, or generated".to_string()]
        }
        ImportDiagnosticKind::InvalidRootPath => vec![
            "use a source-tree-relative path for the import root (no .., absolute, or drive paths)"
                .to_string(),
        ],
    }
}

fn import_graph_summary_from_graph(graph: &ImportGraph) -> SpecSystemImportGraphSummary {
    SpecSystemImportGraphSummary {
        node_count: graph.nodes.len(),
        edge_count: graph.edges.len(),
        diagnostic_count: graph.diagnostics.len(),
        nodes: graph
            .nodes
            .iter()
            .map(|node| SpecSystemImportNode {
                id: node.id.clone(),
                path: node.path.clone(),
                role: node.role.as_str().to_string(),
                ecosystem: node.ecosystem.clone(),
                provenance: node.provenance.as_str().to_string(),
                confidence: node.confidence.as_str().to_string(),
            })
            .collect(),
        edges: graph
            .edges
            .iter()
            .map(|edge| SpecSystemImportEdge {
                source_id: edge.source_id.clone(),
                target_id: edge.target_id.clone(),
                kind: edge.kind.as_str().to_string(),
                provenance: edge.provenance.as_str().to_string(),
            })
            .collect(),
        diagnostics: graph
            .diagnostics
            .iter()
            .map(|diagnostic| SpecSystemImportDiagnostic {
                kind: diagnostic.kind.as_str().to_string(),
                message: diagnostic.message.clone(),
                root_ids: diagnostic.root_ids.clone(),
            })
            .collect(),
    }
}

fn federation_ledgers_readiness_check(root: &Path) -> SpecSystemReadinessCheck {
    let path = allow_policy::federation::FEDERATION_CONFIG_REL_PATH.to_string();
    match load_federation_config(root) {
        Ok(loaded) => match loaded.outcome {
            FederationLoadOutcome::Missing => SpecSystemReadinessCheck {
                kind: "federation_ledgers",
                path: Some(path),
                found: false,
                valid: None,
                status: "ready",
                message: "federation ledger registry `.allow/config.toml` is not configured"
                    .to_string(),
            },
            FederationLoadOutcome::Parsed(validated) => {
                let count = validated.config.ledgers.len();
                readiness_check(
                    "federation_ledgers",
                    Some(path.clone()),
                    true,
                    Some(validated.valid),
                    if validated.valid {
                        format!("federation registry parsed with {count} configured ledger(s)")
                    } else {
                        format!(
                            "federation registry has {} validation issue(s)",
                            validated.diagnostics.len()
                        )
                    },
                )
            }
        },
        Err(err) => {
            let config_path = allow_policy::federation::FEDERATION_CONFIG_REL_PATH;
            readiness_check(
                "federation_ledgers",
                Some(config_path.to_string()),
                root.join(config_path).is_file(),
                Some(false),
                err.to_string(),
            )
        }
    }
}

fn default_spec_system_config() -> SpecSystemConfig {
    SpecSystemConfig {
        schema_version: "1.0".to_string(),
        profile: PROFILE_NAME.to_string(),
        mode: SpecSystemMode::Advisory,
        generation: SpecSystemGeneration::CurrentV2,
        roots: SpecSystemRoots {
            proposals: "docs/proposals".to_string(),
            specs: "docs/specs".to_string(),
            adrs: "docs/adr".to_string(),
            plans: "plans".to_string(),
            goals: None,
            support_tiers: "docs/status/SUPPORT_TIERS.md".to_string(),
            artifact_ledger: "policy/doc-artifacts.toml".to_string(),
        },
        requirements: SpecSystemRequirements {
            ledger_required: true,
            templates_required: true,
            support_tiers_required: true,
            active_goal_required: false,
            closeout_required_for_done_items: true,
        },
        import_roots: None,
    }
}

fn collect_validation(
    findings: &mut Vec<SpecSystemFinding>,
    kind: &'static str,
    result: CargoAllowResult<()>,
) {
    if let Err(err) = result {
        findings.push(SpecSystemFinding::new(kind, err.to_string()));
    }
}

fn active_goal_manifest_source_path(cfg: &SpecSystemConfig) -> Option<String> {
    let goals = cfg.roots.goals.as_deref()?.trim_end_matches(['/', '\\']);
    Some(format!("{goals}/active.toml"))
}

fn validate_active_goal_file(
    root: &Path,
    cfg: &SpecSystemConfig,
    ledger: &DocArtifactLedger,
) -> CargoAllowResult<()> {
    let source_path = active_goal_manifest_source_path(cfg).ok_or_else(|| {
        CargoAllowError::new("legacy active-goal validation requires an explicit legacy goals root")
    })?;
    let active_goal_path = root_relative_path(root, Path::new(&source_path));
    let text = read_text_file_capped(&active_goal_path).map_err(|err| {
        CargoAllowError::new(format!(
            "failed to read active goal manifest {source_path}: {err}"
        ))
    })?;
    validate_active_goal_manifest_text_at(Some(&active_goal_path), &text, ledger).map(|_| ())
}

fn collect_spec_system_readiness(
    root: &Path,
    loaded: &LoadedSpecSystemConfig,
) -> SpecSystemReadiness {
    let cfg = &loaded.cfg;
    let mut checks = Vec::new();
    checks.push(readiness_check(
        "profile_config",
        Some(loaded.path.clone()),
        loaded.found,
        loaded.valid,
        loaded.diagnostic.clone().unwrap_or_else(|| {
            if loaded.found {
                format!(
                    "spec-system profile config parsed (provenance: {})",
                    loaded.provenance.as_str()
                )
            } else {
                format!(
                    "spec-system profile config is missing; built-in roots are in use (provenance: {})",
                    loaded.provenance.as_str()
                )
            }
        }),
    ));

    for (label, path) in [
        ("artifact_root", Some(cfg.roots.proposals.as_str())),
        ("artifact_root", Some(cfg.roots.specs.as_str())),
        ("artifact_root", Some(cfg.roots.adrs.as_str())),
        ("artifact_root", Some(cfg.roots.plans.as_str())),
        ("artifact_root", cfg.roots.goals.as_deref()),
    ] {
        let Some(path) = path else {
            continue;
        };
        let full_path = root_relative_path(root, Path::new(path));
        checks.push(readiness_check(
            label,
            Some(path.to_string()),
            full_path.is_dir(),
            Some(full_path.is_dir()),
            if full_path.is_dir() {
                format!("artifact root {path} exists")
            } else {
                format!("artifact root {path} is missing")
            },
        ));
    }

    let ledger_path = root_relative_path(root, Path::new(&cfg.roots.artifact_ledger));
    let ledger_result = load_doc_artifacts(&ledger_path);
    let ledger_valid = ledger_result.is_ok();
    checks.push(readiness_check(
        "artifact_ledger",
        Some(cfg.roots.artifact_ledger.clone()),
        ledger_path.is_file(),
        Some(ledger_valid),
        match &ledger_result {
            Ok(_) => format!("doc artifact ledger {} parsed", cfg.roots.artifact_ledger),
            Err(err) => err.to_string(),
        },
    ));

    let support_tiers_path = root_relative_path(root, Path::new(&cfg.roots.support_tiers));
    let support_tiers_result = read_text_file_capped(&support_tiers_path)
        .map_err(|err| {
            format!(
                "failed to read support-tier file {}: {err}",
                cfg.roots.support_tiers
            )
        })
        .and_then(|text| validate_support_tier_claims(&text).map_err(|err| err.to_string()));
    checks.push(readiness_check(
        "support_tiers",
        Some(cfg.roots.support_tiers.clone()),
        support_tiers_path.is_file(),
        Some(support_tiers_result.is_ok()),
        match support_tiers_result {
            Ok(_) => format!("support-tier file {} parsed", cfg.roots.support_tiers),
            Err(err) => err,
        },
    ));

    if matches!(cfg.generation, SpecSystemGeneration::LegacyV1) {
        let active_goal = active_goal_manifest_source_path(cfg)
            .unwrap_or_else(|| ".allow/goals/active.toml".to_string());
        let active_goal_path = root_relative_path(root, Path::new(&active_goal));
        if cfg.requirements.active_goal_required {
            let active_goal_result = match &ledger_result {
                Ok(ledger) => {
                    validate_active_goal_file(root, cfg, ledger).map_err(|err| err.to_string())
                }
                Err(err) => Err(format!(
                    "active goal manifest cannot be validated until doc artifact ledger parses: {err}"
                )),
            };
            let active_goal_valid = active_goal_result.is_ok();
            checks.push(readiness_check(
                "active_goal",
                Some(active_goal.clone()),
                active_goal_path.is_file(),
                Some(active_goal_valid),
                match active_goal_result {
                    Ok(()) => format!("active goal manifest {active_goal} parsed"),
                    Err(err) => err,
                },
            ));
        } else {
            checks.push(SpecSystemReadinessCheck {
                kind: "active_goal",
                path: Some(active_goal.clone()),
                found: active_goal_path.is_file(),
                valid: None,
                status: "ready",
                message: "active goal validation is optional because active_goal_required = false"
                    .to_string(),
            });
        }
    }

    let missing_templates = EXPECTED_TEMPLATE_FILES
        .iter()
        .filter(|path| !root_relative_path(root, Path::new(path)).is_file())
        .copied()
        .collect::<Vec<_>>();
    checks.push(readiness_check(
        "templates",
        Some("docs/templates".to_string()),
        missing_templates.is_empty(),
        Some(missing_templates.is_empty()),
        if missing_templates.is_empty() {
            "all spec-system templates exist".to_string()
        } else {
            format!(
                "missing spec-system templates: {}",
                missing_templates.join(", ")
            )
        },
    ));

    if matches!(
        loaded.provenance,
        ProfileConfigProvenance::AllowProfiles | ProfileConfigProvenance::AllowConfig
    ) {
        let imports_path = root_relative_path(root, Path::new(DEFAULT_OWNED_IMPORTS_ROOT));
        checks.push(readiness_check(
            "allow_imports",
            Some(DEFAULT_OWNED_IMPORTS_ROOT.to_string()),
            imports_path.is_dir(),
            Some(imports_path.is_dir()),
            if imports_path.is_dir() {
                format!("owned import root {DEFAULT_OWNED_IMPORTS_ROOT} exists")
            } else {
                format!("owned import root {DEFAULT_OWNED_IMPORTS_ROOT} is missing")
            },
        ));
    }

    checks.push(federation_ledgers_readiness_check(root));

    SpecSystemReadiness {
        ready: checks.iter().all(|check| check.status == "ready"),
        mode: spec_system_mode_name(&cfg.mode),
        checks,
    }
}

fn readiness_check(
    kind: &'static str,
    path: Option<String>,
    found: bool,
    valid: Option<bool>,
    message: String,
) -> SpecSystemReadinessCheck {
    let status = match (found, valid) {
        (false, _) => "missing",
        (true, Some(false)) => "invalid",
        (true, _) => "ready",
    };
    SpecSystemReadinessCheck {
        kind,
        path,
        found,
        valid,
        status,
        message,
    }
}

fn work_items_from_artifact_files(
    root: &Path,
    ledger: &DocArtifactLedger,
) -> Vec<SpecSystemWorkItem> {
    let mut items = Vec::new();
    for artifact in &ledger.artifact {
        let source_path = root_relative_path(root, Path::new(&artifact.path));
        if !source_path.is_file() {
            items.push(artifact_work_item(
                "artifact_file_missing",
                artifact,
                format!(
                    "{} artifact file is missing: {}",
                    artifact.id, artifact.path
                ),
                vec![
                    "create the registered artifact file".to_string(),
                    "or correct the artifact path in policy/doc-artifacts.toml".to_string(),
                ],
            ));
            continue;
        }

        match read_text_file_capped(&source_path) {
            Ok(text) if !text.contains(&artifact.id) => {
                items.push(artifact_work_item(
                    "artifact_id_not_in_file",
                    artifact,
                    format!(
                        "{} is registered but does not appear in {}",
                        artifact.id, artifact.path
                    ),
                    vec![
                        "add the artifact id to the file so links are machine-readable".to_string(),
                        "or correct the id/path pair in policy/doc-artifacts.toml".to_string(),
                    ],
                ));
            }
            Ok(_) => {}
            Err(err) => items.push(artifact_work_item(
                "artifact_file_unreadable",
                artifact,
                format!("failed to read artifact file {}: {err}", artifact.path),
                vec![
                    "make the registered artifact file readable".to_string(),
                    "or correct the artifact path in policy/doc-artifacts.toml".to_string(),
                ],
            )),
        }
    }
    items
}

fn work_items_from_artifact_links(ledger: &DocArtifactLedger) -> Vec<SpecSystemWorkItem> {
    let mut items = Vec::new();
    for artifact in &ledger.artifact {
        items.extend(work_items_from_required_edges(artifact));
        for (field, value) in artifact_link_fields(artifact) {
            let Some(target) = value.filter(|target| !target.trim().is_empty()) else {
                continue;
            };
            if resolve_target_kind(ledger, target).is_none() {
                items.push(artifact_work_item(
                    "unknown_link_target",
                    artifact,
                    format!(
                        "{} {field} target {} is not registered",
                        artifact.id, target
                    ),
                    vec![
                        format!("register {target} in policy/doc-artifacts.toml"),
                        format!("or correct {field} in {}", artifact.path),
                    ],
                ));
            }
        }
    }
    items
}

fn work_items_from_required_edges(artifact: &DocArtifact) -> Vec<SpecSystemWorkItem> {
    let mut items = Vec::new();
    match artifact.kind {
        ArtifactKind::Spec
            if artifact.status == ArtifactStatus::Accepted
                && !has_artifact_value(artifact.linked_proposal.as_deref())
                && !has_artifact_value(artifact.standalone_reason.as_deref()) =>
        {
            items.push(artifact_work_item(
                "missing_linked_proposal",
                artifact,
                format!(
                    "{} accepted spec requires linked_proposal or standalone_reason",
                    artifact.id
                ),
                vec![
                    "link this accepted spec to the proposal that justified it".to_string(),
                    "or add standalone_reason if the spec is intentionally standalone".to_string(),
                ],
            ));
        }
        ArtifactKind::Adr
            if artifact.status == ArtifactStatus::Accepted
                && !has_artifact_value(artifact.linked_spec.as_deref())
                && !has_artifact_value(artifact.standalone_reason.as_deref()) =>
        {
            items.push(missing_required_edge_work_item(
                artifact,
                "linked_spec",
                "accepted ADR requires linked_spec or standalone_reason",
            ));
        }
        ArtifactKind::ImplementationPlan | ArtifactKind::PlanItem
            if artifact.status == ArtifactStatus::Active
                && !has_artifact_value(artifact.linked_proposal.as_deref())
                && !has_artifact_value(artifact.linked_spec.as_deref()) =>
        {
            items.push(missing_required_edge_work_item(
                artifact,
                "linked_proposal or linked_spec",
                "active plan requires linked_proposal or linked_spec",
            ));
        }
        ArtifactKind::ActiveGoal if artifact.status == ArtifactStatus::Active => {
            for (field, value) in [
                ("linked_proposal", artifact.linked_proposal.as_deref()),
                ("linked_spec", artifact.linked_spec.as_deref()),
                ("linked_plan", artifact.linked_plan.as_deref()),
            ] {
                if !has_artifact_value(value) {
                    items.push(missing_required_edge_work_item(
                        artifact,
                        field,
                        "active goal requires links to proposal, spec, and plan",
                    ));
                }
            }
        }
        ArtifactKind::Closeout if !has_artifact_value(artifact.linked_plan.as_deref()) => {
            items.push(missing_required_edge_work_item(
                artifact,
                "linked_plan",
                "closeout requires linked_plan",
            ));
        }
        _ => {}
    }
    items
}

fn work_items_from_missing_closeouts(
    ledger: &DocArtifactLedger,
    closeout_required_for_done_items: bool,
) -> Vec<SpecSystemWorkItem> {
    if !closeout_required_for_done_items {
        return Vec::new();
    }
    ledger
        .artifact
        .iter()
        .filter(|artifact| {
            matches!(
                artifact.kind,
                ArtifactKind::ImplementationPlan | ArtifactKind::PlanItem
            ) && artifact.status == ArtifactStatus::Done
                && !has_artifact_value(artifact.linked_closeout.as_deref())
        })
        .map(|artifact| {
            artifact_work_item(
                "missing_closeout",
                artifact,
                format!("{} done plan item requires linked_closeout", artifact.id),
                vec![
                    "add a closeout artifact and link it with linked_closeout".to_string(),
                    "or move the plan item out of done until closeout evidence exists".to_string(),
                ],
            )
        })
        .collect()
}

fn work_items_from_support_tiers(
    support_tiers_path: &str,
    rows: &[allow_policy::spec_system::SupportTierRow],
) -> Vec<SpecSystemWorkItem> {
    rows.iter()
        .filter(|row| {
            matches!(
                row.tier,
                SupportTierLevel::Stable | SupportTierLevel::Stabilizing
            ) && row.proof_command.trim().is_empty()
        })
        .map(|row| SpecSystemWorkItem {
            kind: "missing_proof_command",
            artifact_id: None,
            path: Some(support_tiers_path.to_string()),
            owner: None,
            status: Some(support_tier_level_name(row.tier).to_string()),
            message: format!(
                "{} support-tier claim requires a proof command",
                row.surface
            ),
            suggested_actions: vec![
                "add a non-empty proof command to the support-tier row".to_string(),
                "or lower the tier if the claim is not ready for proof-backed status".to_string(),
            ],
            proof_commands: spec_system_proof_commands(),
            ledger_id: None,
            ledger_path: None,
            lane: None,
            mode: None,
            role: None,
        })
        .collect()
}

fn work_items_from_config_findings(findings: &[SpecSystemFinding]) -> Vec<SpecSystemWorkItem> {
    findings
        .iter()
        .filter(|finding| finding.kind == "profile_config")
        .map(|finding| {
            missing_node_work_item(
                "spec-system profile config",
                DEFAULT_PROFILE_CONFIG,
                &finding.message,
                vec![
                    format!("create {DEFAULT_PROFILE_CONFIG} with spec-system roots"),
                    "or pass --config with the intended spec-system profile config".to_string(),
                ],
            )
        })
        .collect()
}

fn active_goal_work_item(path: &str, message: &str) -> SpecSystemWorkItem {
    let kind = active_goal_work_item_kind(message);
    SpecSystemWorkItem {
        kind,
        artifact_id: None,
        path: Some(path.to_string()),
        owner: Some("codex".to_string()),
        status: None,
        message: format!("active goal manifest is missing or invalid: {message}"),
        suggested_actions: active_goal_suggested_actions(kind),
        proof_commands: spec_system_proof_commands(),
        ledger_id: None,
        ledger_path: None,
        lane: None,
        mode: None,
        role: None,
    }
}

fn active_goal_work_item_kind(message: &str) -> &'static str {
    if message.contains("proof_commands") || message.contains("proof command") {
        return "missing_proof_command";
    }
    if message.contains("closeout") {
        return "missing_closeout";
    }
    "stale_active_goal"
}

fn active_goal_suggested_actions(kind: &str) -> Vec<String> {
    match kind {
        "missing_proof_command" => vec![
            "add non-empty proof_commands to ready, in_progress, or done active goal work items"
                .to_string(),
            "or move the work item to blocked with blocker_reason if proof cannot be named"
                .to_string(),
        ],
        "missing_closeout" => vec![
            "link done active goal work items to a registered closeout artifact".to_string(),
            "or move the work item out of done until closeout evidence exists".to_string(),
        ],
        _ => vec![
            "update the active goal manifest so its IDs, status, and links match the doc artifact ledger"
                .to_string(),
            "or register the intended active goal, linked plan, spec, proposal, and support tier in the doc artifact ledger"
                .to_string(),
        ],
    }
}

fn missing_required_edge_work_item(
    artifact: &DocArtifact,
    field: &str,
    reason: &str,
) -> SpecSystemWorkItem {
    artifact_work_item(
        "missing_required_edge",
        artifact,
        format!("{} {reason}", artifact.id),
        vec![
            format!("add {field} to the registered artifact metadata"),
            "or add standalone_reason where the artifact is intentionally standalone".to_string(),
        ],
    )
}

fn missing_node_work_item(
    node: &str,
    path: &str,
    message: &str,
    suggested_actions: Vec<String>,
) -> SpecSystemWorkItem {
    SpecSystemWorkItem {
        kind: "missing_node",
        artifact_id: None,
        path: Some(path.to_string()),
        owner: None,
        status: None,
        message: format!("{node} is missing or invalid: {message}"),
        suggested_actions,
        proof_commands: spec_system_proof_commands(),
        ledger_id: None,
        ledger_path: None,
        lane: None,
        mode: None,
        role: None,
    }
}

fn apply_work_item_ledger_provenance(
    work_items: &mut [SpecSystemWorkItem],
    provenance: &allow_core::LedgerProvenance,
) {
    for item in work_items {
        item.ledger_id = Some(provenance.ledger_id.clone());
        item.ledger_path = Some(provenance.ledger_path.clone());
        item.lane = Some(provenance.lane.clone());
        item.mode = Some(provenance.mode.clone());
        item.role = Some(provenance.role.clone());
    }
}

fn artifact_work_item(
    kind: &'static str,
    artifact: &DocArtifact,
    message: String,
    suggested_actions: Vec<String>,
) -> SpecSystemWorkItem {
    SpecSystemWorkItem {
        kind,
        artifact_id: Some(artifact.id.clone()),
        path: Some(artifact.path.clone()),
        owner: Some(artifact.owner.clone()),
        status: Some(artifact_status_name(artifact.status).to_string()),
        message,
        suggested_actions,
        proof_commands: spec_system_proof_commands(),
        ledger_id: None,
        ledger_path: None,
        lane: None,
        mode: None,
        role: None,
    }
}

fn spec_system_proof_commands() -> Vec<String> {
    vec![
        SPEC_SYSTEM_CHECK_PROOF_COMMAND.to_string(),
        SPEC_SYSTEM_WORKLIST_PROOF_COMMAND.to_string(),
    ]
}

fn has_artifact_value(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
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

fn filter_spec_system_report_for_artifact(
    report: &SpecSystemReport,
    artifact_id: &str,
) -> CargoAllowResult<SpecSystemReport> {
    let artifact = report
        .artifacts
        .iter()
        .find(|artifact| artifact.id == artifact_id)
        .ok_or_else(|| CargoAllowError::new(format!("no spec-system artifact `{artifact_id}`")))?;
    let links = report
        .links
        .iter()
        .filter(|link| spec_system_link_touches_artifact(link, artifact))
        .cloned()
        .collect::<Vec<_>>();
    let findings = report
        .findings
        .iter()
        .filter(|finding| spec_system_message_mentions_artifact(&finding.message, artifact))
        .cloned()
        .collect::<Vec<_>>();
    let work_items = report
        .work_items
        .iter()
        .filter(|item| spec_system_work_item_touches_artifact(item, artifact))
        .cloned()
        .collect::<Vec<_>>();

    Ok(SpecSystemReport {
        command: report.command.clone(),
        root: report.root.clone(),
        config_source: report.config_source.clone(),
        config_provenance: report.config_provenance.clone(),
        mode: report.mode.clone(),
        artifacts: vec![artifact.clone()],
        links,
        support_tier_rows: report.support_tier_rows,
        findings,
        work_items,
        readiness: None,
        federation: report.federation.clone(),
        import_graph: report.import_graph.clone(),
    })
}

fn spec_system_link_touches_artifact(link: &SpecSystemLink, artifact: &SpecSystemArtifact) -> bool {
    link.source_id == artifact.id || link.target == artifact.id || link.target == artifact.path
}

fn spec_system_work_item_touches_artifact(
    item: &SpecSystemWorkItem,
    artifact: &SpecSystemArtifact,
) -> bool {
    item.artifact_id.as_deref() == Some(artifact.id.as_str())
        || item.path.as_deref() == Some(artifact.path.as_str())
        || spec_system_message_mentions_artifact(&item.message, artifact)
}

fn spec_system_message_mentions_artifact(message: &str, artifact: &SpecSystemArtifact) -> bool {
    message.contains(&artifact.id) || message.contains(&artifact.path)
}

fn render_spec_system_explain_markdown(report: &SpecSystemReport) -> String {
    let Some(artifact) = report.artifacts.first() else {
        return render_spec_system_markdown(report);
    };
    let mut text = String::new();
    text.push_str(&format!(
        "# cargo-allow explain {} --profile spec-system\n\n",
        artifact.id
    ));
    text.push_str(&format!(
        "**Result:** {}\n\n",
        spec_system_mode_name(&report.mode)
    ));
    text.push_str(&format!(
        "Mode: `{}`\n\n",
        spec_system_mode_name(&report.mode)
    ));
    text.push_str(&format!(
        "Status: `{}`\n\n",
        spec_system_report_status(report)
    ));
    text.push_str("Profile: `spec-system`\n\n");
    text.push_str(&format!(
        "Source tree root: `{}`\n\n",
        report.root.display()
    ));
    text.push_str(&format!("Config: `{}`\n\n", report.config_source));
    text.push_str(&format!(
        "Config provenance: `{}`\n\n",
        report.config_provenance
    ));

    text.push_str("## Artifact\n\n");
    text.push_str("| Field | Value |\n|---|---|\n");
    text.push_str(&format!("| ID | `{}` |\n", artifact.id));
    text.push_str(&format!("| Kind | `{}` |\n", artifact.kind));
    text.push_str(&format!("| Path | `{}` |\n", artifact.path));
    text.push_str(&format!("| Status | `{}` |\n", artifact.status));
    text.push_str(&format!("| Owner | `{}` |\n", artifact.owner));
    text.push_str(&format!("| Created | `{}` |\n\n", artifact.created));

    render_spec_system_link_section(&mut text, "Outgoing Links", report, artifact, true);
    render_spec_system_link_section(&mut text, "Incoming Links", report, artifact, false);

    text.push_str("## Current Findings\n\n");
    if report.findings.is_empty() {
        text.push_str("No findings for this artifact.\n\n");
    } else {
        for finding in &report.findings {
            let posture = finding.blocking_reason.unwrap_or("advisory");
            text.push_str(&format!(
                "- `{}` (`{}`): {}\n",
                finding.kind, posture, finding.message
            ));
        }
        text.push('\n');
    }

    text.push_str("## Repair Work Items\n\n");
    if report.work_items.is_empty() {
        text.push_str("No work items for this artifact.\n\n");
    } else {
        for item in &report.work_items {
            let posture = spec_system_work_item_blocking_reason(item).unwrap_or("advisory");
            text.push_str(&format!(
                "- `{}` (`{}`): {}\n",
                item.kind, posture, item.message
            ));
            if !item.suggested_actions.is_empty() {
                text.push_str("  - Suggested actions:\n");
                for action in &item.suggested_actions {
                    text.push_str(&format!("    - {action}\n"));
                }
            }
            if !item.proof_commands.is_empty() {
                text.push_str("  - Proof commands:\n");
                for command in &item.proof_commands {
                    text.push_str(&format!("    - `{command}`\n"));
                }
            }
        }
        text.push('\n');
    }

    text.push_str("## Proof Commands\n\n");
    for command in spec_system_explain_proof_commands(&artifact.id) {
        text.push_str(&format!("- `{command}`\n"));
    }
    text.push('\n');
    text.push_str("> Claim boundary: structural source-tree graph validation only; cargo-allow did not execute proof commands, run tests, invoke Cargo, rustc, Clippy, build scripts, proc macros, external proof tools, network calls, or GitHub APIs.\n");
    text
}

fn render_spec_system_link_section(
    text: &mut String,
    title: &str,
    report: &SpecSystemReport,
    artifact: &SpecSystemArtifact,
    outgoing: bool,
) {
    let links = report
        .links
        .iter()
        .filter(|link| {
            if outgoing {
                link.source_id == artifact.id
            } else {
                link.target == artifact.id || link.target == artifact.path
            }
        })
        .collect::<Vec<_>>();
    text.push_str(&format!("## {title}\n\n"));
    if links.is_empty() {
        text.push_str("None.\n\n");
        return;
    }
    text.push_str("| Field | Source | Target | Target kind |\n|---|---|---|---|\n");
    for link in links {
        let target_kind = link.target_kind.unwrap_or("");
        text.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` |\n",
            link.field, link.source_id, link.target, target_kind
        ));
    }
    text.push('\n');
}

fn render_spec_system_markdown(report: &SpecSystemReport) -> String {
    let mut text = String::new();
    text.push_str(&format!(
        "# cargo-allow {} --profile spec-system\n\n",
        report.command
    ));
    text.push_str(&format!(
        "**Result:** {}\n\n",
        spec_system_mode_name(&report.mode)
    ));
    text.push_str(&format!(
        "Mode: `{}`\n\n",
        spec_system_mode_name(&report.mode)
    ));
    text.push_str(&format!(
        "Status: `{}`\n\n",
        spec_system_report_status(report)
    ));
    text.push_str("Profile: `spec-system`\n\n");
    text.push_str(&format!(
        "Source tree root: `{}`\n\n",
        report.root.display()
    ));
    text.push_str(&format!("Config: `{}`\n\n", report.config_source));
    text.push_str(&format!(
        "Config provenance: `{}`\n\n",
        report.config_provenance
    ));
    if let Some(readiness) = &report.readiness {
        text.push_str("## Setup Readiness\n\n");
        text.push_str(&format!("Mode: `{}`\n\n", readiness.mode));
        text.push_str(&format!("Ready: `{}`\n\n", readiness.ready));
        text.push_str("| Check | Status | Path | Message |\n|---|---|---|---|\n");
        for check in &readiness.checks {
            let path = check.path.as_deref().unwrap_or("");
            text.push_str(&format!(
                "| `{}` | `{}` | `{}` | {} |\n",
                check.kind, check.status, path, check.message
            ));
        }
        text.push('\n');
    }
    text.push_str("| Metric | Count |\n|---|---:|\n");
    text.push_str(&format!("| Artifacts | {} |\n", report.artifacts.len()));
    text.push_str(&format!("| Links | {} |\n", report.links.len()));
    text.push_str(&format!(
        "| Support-tier rows | {} |\n",
        report.support_tier_rows
    ));
    text.push_str(&format!("| Findings | {} |\n", report.findings.len()));
    text.push_str(&format!(
        "| Blocking-eligible findings | {} |\n",
        spec_system_blocking_finding_count(report)
    ));
    text.push_str(&format!(
        "| Advisory findings | {} |\n",
        spec_system_advisory_finding_count(report)
    ));
    text.push_str(&format!("| Work items | {} |\n", report.work_items.len()));
    text.push_str(&format!(
        "| Blocking-eligible work items | {} |\n",
        spec_system_blocking_work_item_count(report)
    ));
    text.push_str(&format!(
        "| Advisory work items | {} |\n",
        spec_system_advisory_work_item_count(report)
    ));
    text.push('\n');
    if report.findings.is_empty() {
        text.push_str(&format!(
            "No spec-system findings in `{}` mode.\n\n",
            spec_system_mode_name(&report.mode)
        ));
    } else {
        text.push_str("## Findings\n\n");
        if spec_system_blocking_finding_count(report) > 0 {
            text.push_str("### Blocking-Eligible Findings\n\n");
            for finding in report
                .findings
                .iter()
                .filter(|finding| finding.blocking_eligible)
            {
                let posture = finding.blocking_reason.unwrap_or("blocking_eligible");
                text.push_str(&format!(
                    "- `{}` (`{}`): {}\n",
                    finding.kind, posture, finding.message
                ));
            }
            text.push('\n');
        }
        if spec_system_advisory_finding_count(report) > 0 {
            text.push_str("### Advisory Findings\n\n");
            for finding in report
                .findings
                .iter()
                .filter(|finding| !finding.blocking_eligible)
            {
                text.push_str(&format!(
                    "- `{}` (`advisory`): {}\n",
                    finding.kind, finding.message
                ));
            }
            text.push('\n');
        }
    }
    if !report.work_items.is_empty() {
        text.push_str("## Work Items\n\n");
        for item in &report.work_items {
            let posture = spec_system_work_item_blocking_reason(item).unwrap_or("advisory");
            text.push_str(&format!(
                "- `{}` (`{}`): {}\n",
                item.kind, posture, item.message
            ));
            if let Some(artifact_id) = &item.artifact_id {
                text.push_str(&format!("  - Artifact: `{artifact_id}`\n"));
            }
            if let Some(path) = &item.path {
                text.push_str(&format!("  - Path: `{path}`\n"));
            }
            if !item.suggested_actions.is_empty() {
                text.push_str("  - Suggested actions:\n");
                for action in &item.suggested_actions {
                    text.push_str(&format!("    - {action}\n"));
                }
            }
            if !item.proof_commands.is_empty() {
                text.push_str("  - Proof commands:\n");
                for command in &item.proof_commands {
                    text.push_str(&format!("    - `{command}`\n"));
                }
            }
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
    text.push_str(&format!(
        "  \"mode\": \"{}\",\n",
        spec_system_mode_name(&report.mode)
    ));
    text.push_str(&format!(
        "  \"status\": \"{}\",\n",
        spec_system_report_status(report)
    ));
    text.push_str(&format!(
        "  \"failed\": {},\n",
        if spec_system_report_failed(report) {
            "true"
        } else {
            "false"
        }
    ));
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
    text.push_str(&format!(
        "  \"config_provenance\": \"{}\",\n",
        json_escape(&report.config_provenance)
    ));
    if let Some(federation) = &report.federation {
        text.push_str("  \"federation\": {\n");
        text.push_str(&format!(
            "    \"federation_version\": \"{}\",\n",
            json_escape(&federation.federation_version)
        ));
        text.push_str(&format!(
            "    \"precedence_applied\": \"{}\",\n",
            json_escape(&federation.precedence_applied)
        ));
        text.push_str("    \"ledger_contributors\": [\n");
        for (index, contributor) in federation.ledger_contributors.iter().enumerate() {
            if index > 0 {
                text.push_str(",\n");
            }
            text.push_str("      {\n");
            text.push_str(&format!(
                "        \"id\": \"{}\",\n",
                json_escape(&contributor.id)
            ));
            text.push_str(&format!(
                "        \"path\": \"{}\",\n",
                json_escape(&contributor.path)
            ));
            text.push_str(&format!(
                "        \"role\": \"{}\",\n",
                json_escape(&contributor.role)
            ));
            text.push_str(&format!(
                "        \"dialect\": \"{}\",\n",
                json_escape(&contributor.dialect)
            ));
            text.push_str(&format!(
                "        \"mode\": \"{}\",\n",
                json_escape(&contributor.mode)
            ));
            text.push_str(&format!(
                "        \"priority\": {},\n",
                contributor.priority
            ));
            text.push_str("        \"lanes\": ");
            render_string_array(&mut text, &contributor.lanes, "        ");
            text.push_str("\n      }");
        }
        text.push_str("\n    ]\n  },\n");
    }
    if let Some(import_graph) = &report.import_graph {
        text.push_str("  \"import_graph\": {\n");
        text.push_str(&format!(
            "    \"node_count\": {},\n",
            import_graph.node_count
        ));
        text.push_str(&format!(
            "    \"edge_count\": {},\n",
            import_graph.edge_count
        ));
        text.push_str(&format!(
            "    \"diagnostic_count\": {},\n",
            import_graph.diagnostic_count
        ));
        text.push_str("    \"nodes\": [\n");
        for (index, node) in import_graph.nodes.iter().enumerate() {
            if index > 0 {
                text.push_str(",\n");
            }
            text.push_str("      {\n");
            text.push_str(&format!("        \"id\": \"{}\",\n", json_escape(&node.id)));
            text.push_str(&format!(
                "        \"path\": \"{}\",\n",
                json_escape(&node.path)
            ));
            text.push_str(&format!(
                "        \"role\": \"{}\",\n",
                json_escape(&node.role)
            ));
            text.push_str(&format!(
                "        \"ecosystem\": \"{}\",\n",
                json_escape(&node.ecosystem)
            ));
            text.push_str(&format!(
                "        \"provenance\": \"{}\",\n",
                json_escape(&node.provenance)
            ));
            text.push_str(&format!(
                "        \"confidence\": \"{}\"\n",
                json_escape(&node.confidence)
            ));
            text.push_str("      }");
        }
        text.push_str("\n    ],\n");
        text.push_str("    \"edges\": [\n");
        for (index, edge) in import_graph.edges.iter().enumerate() {
            if index > 0 {
                text.push_str(",\n");
            }
            text.push_str("      {\n");
            text.push_str(&format!(
                "        \"source_id\": \"{}\",\n",
                json_escape(&edge.source_id)
            ));
            text.push_str(&format!(
                "        \"target_id\": \"{}\",\n",
                json_escape(&edge.target_id)
            ));
            text.push_str(&format!(
                "        \"kind\": \"{}\",\n",
                json_escape(&edge.kind)
            ));
            text.push_str(&format!(
                "        \"provenance\": \"{}\"\n",
                json_escape(&edge.provenance)
            ));
            text.push_str("      }");
        }
        text.push_str("\n    ],\n");
        text.push_str("    \"diagnostics\": [\n");
        for (index, diagnostic) in import_graph.diagnostics.iter().enumerate() {
            if index > 0 {
                text.push_str(",\n");
            }
            text.push_str("      {\n");
            text.push_str(&format!(
                "        \"kind\": \"{}\",\n",
                json_escape(&diagnostic.kind)
            ));
            text.push_str(&format!(
                "        \"message\": \"{}\",\n",
                json_escape(&diagnostic.message)
            ));
            text.push_str("        \"root_ids\": ");
            render_string_array(&mut text, &diagnostic.root_ids, "        ");
            text.push_str("\n      }");
        }
        text.push_str("\n    ]\n  },\n");
    }
    if report.command == "explain"
        && let Some(artifact) = report.artifacts.first()
    {
        text.push_str(&format!(
            "  \"explained_artifact_id\": \"{}\",\n",
            json_escape(&artifact.id)
        ));
        text.push_str("  \"proof_commands\": ");
        render_string_array(
            &mut text,
            &spec_system_explain_proof_commands(&artifact.id),
            "  ",
        );
        text.push_str(",\n");
    }
    if let Some(readiness) = &report.readiness {
        text.push_str("  \"readiness\": {\n");
        text.push_str(&format!(
            "    \"ready\": {},\n",
            if readiness.ready { "true" } else { "false" }
        ));
        text.push_str(&format!(
            "    \"mode\": \"{}\",\n",
            json_escape(readiness.mode)
        ));
        text.push_str("    \"checks\": [\n");
        for (index, check) in readiness.checks.iter().enumerate() {
            text.push_str("      {\n");
            text.push_str(&format!(
                "        \"kind\": \"{}\",\n",
                json_escape(check.kind)
            ));
            if let Some(path) = &check.path {
                text.push_str(&format!("        \"path\": \"{}\",\n", json_escape(path)));
            }
            text.push_str(&format!(
                "        \"found\": {},\n",
                if check.found { "true" } else { "false" }
            ));
            text.push_str(&format!(
                "        \"valid\": {},\n",
                optional_bool_json(check.valid)
            ));
            text.push_str(&format!(
                "        \"status\": \"{}\",\n",
                json_escape(check.status)
            ));
            text.push_str(&format!(
                "        \"message\": \"{}\"\n",
                json_escape(&check.message)
            ));
            text.push_str("      }");
            if index + 1 != readiness.checks.len() {
                text.push(',');
            }
            text.push('\n');
        }
        text.push_str("    ]\n");
        text.push_str("  },\n");
    }
    text.push_str("  \"summary\": {\n");
    text.push_str(&format!("    \"artifacts\": {},\n", report.artifacts.len()));
    text.push_str(&format!("    \"links\": {},\n", report.links.len()));
    text.push_str(&format!(
        "    \"support_tier_rows\": {},\n",
        report.support_tier_rows
    ));
    text.push_str(&format!("    \"findings\": {},\n", report.findings.len()));
    text.push_str(&format!(
        "    \"blocking_eligible_findings\": {},\n",
        spec_system_blocking_finding_count(report)
    ));
    text.push_str(&format!(
        "    \"advisory_findings\": {},\n",
        spec_system_advisory_finding_count(report)
    ));
    text.push_str(&format!(
        "    \"work_items\": {},\n",
        report.work_items.len()
    ));
    text.push_str(&format!(
        "    \"blocking_eligible_work_items\": {},\n",
        spec_system_blocking_work_item_count(report)
    ));
    text.push_str(&format!(
        "    \"advisory_work_items\": {}\n",
        spec_system_advisory_work_item_count(report)
    ));
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
            "\"message\": \"{}\", ",
            json_escape(&finding.message)
        ));
        text.push_str(&format!(
            "\"blocking_eligible\": {}",
            if finding.blocking_eligible {
                "true"
            } else {
                "false"
            }
        ));
        if let Some(reason) = finding.blocking_reason {
            text.push_str(&format!(
                ", \"blocking_reason\": \"{}\"",
                json_escape(reason)
            ));
        }
        text.push('}');
        if index + 1 != report.findings.len() {
            text.push(',');
        }
        text.push('\n');
    }
    text.push_str("  ],\n");
    text.push_str("  \"work_items\": [\n");
    for (index, item) in report.work_items.iter().enumerate() {
        text.push_str("    {\n");
        text.push_str(&format!("      \"kind\": \"{}\"", json_escape(item.kind)));
        if let Some(artifact_id) = &item.artifact_id {
            text.push_str(&format!(
                ",\n      \"artifact_id\": \"{}\"",
                json_escape(artifact_id)
            ));
        }
        if let Some(path) = &item.path {
            text.push_str(&format!(",\n      \"path\": \"{}\"", json_escape(path)));
        }
        if let Some(owner) = &item.owner {
            text.push_str(&format!(",\n      \"owner\": \"{}\"", json_escape(owner)));
        }
        if let Some(status) = &item.status {
            text.push_str(&format!(",\n      \"status\": \"{}\"", json_escape(status)));
        }
        let blocking_reason = spec_system_work_item_blocking_reason(item);
        text.push_str(&format!(
            ",\n      \"blocking_eligible\": {}",
            if blocking_reason.is_some() {
                "true"
            } else {
                "false"
            }
        ));
        if let Some(reason) = blocking_reason {
            text.push_str(&format!(
                ",\n      \"blocking_reason\": \"{}\"",
                json_escape(reason)
            ));
        }
        text.push_str(&format!(
            ",\n      \"message\": \"{}\",\n",
            json_escape(&item.message)
        ));
        text.push_str("      \"suggested_actions\": ");
        render_string_array(&mut text, &item.suggested_actions, "      ");
        text.push_str(",\n      \"proof_commands\": ");
        render_string_array(&mut text, &item.proof_commands, "      ");
        if let Some(ledger_id) = &item.ledger_id {
            text.push_str(&format!(
                ",\n      \"ledger_id\": \"{}\"",
                json_escape(ledger_id)
            ));
        }
        if let Some(ledger_path) = &item.ledger_path {
            text.push_str(&format!(
                ",\n      \"ledger_path\": \"{}\"",
                json_escape(ledger_path)
            ));
        }
        if let Some(lane) = &item.lane {
            text.push_str(&format!(",\n      \"lane\": \"{}\"", json_escape(lane)));
        }
        if let Some(mode) = &item.mode {
            text.push_str(&format!(",\n      \"mode\": \"{}\"", json_escape(mode)));
        }
        if let Some(role) = &item.role {
            text.push_str(&format!(",\n      \"role\": \"{}\"", json_escape(role)));
        }
        text.push('\n');
        text.push_str("    }");
        if index + 1 != report.work_items.len() {
            text.push(',');
        }
        text.push('\n');
    }
    text.push_str("  ]\n");
    text.push_str("}\n");
    text
}

fn spec_system_explain_proof_commands(artifact_id: &str) -> Vec<String> {
    let mut commands = spec_system_proof_commands();
    commands.push(format!(
        "cargo-allow explain {artifact_id} --profile spec-system"
    ));
    commands
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
    for (field, value) in artifact_link_fields(artifact) {
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

fn artifact_link_fields(artifact: &DocArtifact) -> [(&'static str, Option<&str>); 10] {
    [
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
    ]
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

fn spec_system_mode_name(mode: &SpecSystemMode) -> &'static str {
    match mode {
        SpecSystemMode::Advisory => "advisory",
        SpecSystemMode::Shadow => "shadow",
        SpecSystemMode::Blocking => "blocking",
    }
}

/// Parse an explicit `--mode` value for a `check --profile spec-system` run
/// into a [`SpecSystemMode`] override.
///
/// The shared `--mode` flag is validated by clap against the source-tree
/// vocabulary (`audit`, `no-new`, `strict`, `release`). Of those, `audit` is
/// the report-only mode, so it maps to spec-system `advisory` and makes the
/// documented `--mode audit` proof command work. The enforcing source-tree
/// modes have no unambiguous spec-system meaning, so they fail closed with a
/// clear error rather than silently reverting to the config mode (#1941).
/// Shadow and blocking enforcement stay config-driven.
fn parse_spec_system_mode_override(value: &str) -> CargoAllowResult<SpecSystemMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "audit" | "advisory" => Ok(SpecSystemMode::Advisory),
        other => Err(CargoAllowError::new(format!(
            "--mode `{other}` is not supported with --profile spec-system; use --mode audit for a report-only run, and set shadow or blocking enforcement in the spec-system config"
        ))),
    }
}

fn spec_system_command_failed(report: &SpecSystemReport) -> bool {
    spec_system_setup_failed(report)
        || (report.mode == SpecSystemMode::Blocking
            && spec_system_blocking_finding_count(report) > 0)
}

fn spec_system_report_failed(report: &SpecSystemReport) -> bool {
    if spec_system_setup_failed(report) {
        return true;
    }
    match report.mode {
        SpecSystemMode::Advisory => false,
        SpecSystemMode::Shadow => !report.findings.is_empty(),
        SpecSystemMode::Blocking => spec_system_blocking_finding_count(report) > 0,
    }
}

fn spec_system_setup_failed(report: &SpecSystemReport) -> bool {
    report
        .findings
        .iter()
        .any(|finding| finding.kind == "profile_config" && finding.blocking_eligible)
}

fn spec_system_blocking_finding_count(report: &SpecSystemReport) -> usize {
    report
        .findings
        .iter()
        .filter(|finding| finding.blocking_eligible)
        .count()
}

fn spec_system_advisory_finding_count(report: &SpecSystemReport) -> usize {
    report.findings.len() - spec_system_blocking_finding_count(report)
}

fn spec_system_blocking_work_item_count(report: &SpecSystemReport) -> usize {
    report
        .work_items
        .iter()
        .filter(|item| spec_system_work_item_blocking_reason(item).is_some())
        .count()
}

fn spec_system_advisory_work_item_count(report: &SpecSystemReport) -> usize {
    report.work_items.len() - spec_system_blocking_work_item_count(report)
}

fn spec_system_report_status(report: &SpecSystemReport) -> &'static str {
    if spec_system_report_failed(report) {
        "failed"
    } else {
        "passed"
    }
}

fn spec_system_blocking_reason(kind: &str, message: &str) -> Option<&'static str> {
    match kind {
        "profile_config" => profile_config_blocking_reason(message),
        "federation_config" => federation_config_blocking_reason(message),
        "doc_artifact_ledger" => doc_artifact_ledger_blocking_reason(message),
        "artifact_file" => artifact_file_blocking_reason(message),
        "artifact_link" => artifact_link_blocking_reason(message),
        _ => None,
    }
}

fn profile_config_blocking_reason(message: &str) -> Option<&'static str> {
    if message.contains("does not exist") || message.contains("both owned profile config") {
        return None;
    }
    if message.contains("failed to parse spec-system config TOML")
        || message.contains("failed to read spec-system profile config")
    {
        return Some("profile_config_parse_failure");
    }
    None
}

fn federation_config_blocking_reason(message: &str) -> Option<&'static str> {
    if message.contains("duplicate federation ledger id") {
        return Some("duplicate_id");
    }
    if message.contains("dialect_conflict") || message.contains("foreign dialect") {
        return Some("dialect_conflict");
    }
    if message.contains("duplicate_path")
        || message.contains("duplicate_canonical_lane")
        || message.contains("mirror_missing_target")
        || message.contains("unknown_mirror_target")
        || message.contains("unknown_drain_mirror_ledger")
        || message.contains("drain_window_missing_field")
    {
        return Some("federation_config_invalid");
    }
    if message.contains("failed to parse federation config TOML") {
        return Some("federation_config_parse_failure");
    }
    None
}

fn doc_artifact_ledger_blocking_reason(message: &str) -> Option<&'static str> {
    if message.contains("failed to read doc artifact ledger") {
        return Some("doc_artifact_ledger_missing");
    }
    if message.contains("failed to parse doc artifact ledger TOML") {
        if message.contains("unknown variant") {
            return Some("invalid_artifact_kind_or_status");
        }
        return Some("doc_artifact_ledger_parse_failure");
    }
    if message.contains("duplicate doc artifact id") {
        return Some("duplicate_id");
    }
    None
}

fn artifact_file_blocking_reason(message: &str) -> Option<&'static str> {
    if message.contains(" artifact file missing: ") {
        return Some("artifact_file_missing");
    }
    if message.contains("failed to read artifact ") {
        return Some("artifact_file_unreadable");
    }
    if message.contains(" not found in artifact file ") {
        return Some("artifact_id_not_in_file");
    }
    None
}

fn artifact_link_blocking_reason(message: &str) -> Option<&'static str> {
    if message.contains(" target ") && message.contains(" is not registered") {
        return Some("unknown_link_target");
    }
    if message.contains(" target ") && message.contains(" is not registered by id or path") {
        return Some("unknown_link_target");
    }
    None
}

fn spec_system_work_item_blocking_reason(item: &SpecSystemWorkItem) -> Option<&'static str> {
    match item.kind {
        "artifact_file_missing" => Some("artifact_file_missing"),
        "artifact_file_unreadable" => Some("artifact_file_unreadable"),
        "artifact_id_not_in_file" => Some("artifact_id_not_in_file"),
        "unknown_link_target" => Some("unknown_link_target"),
        "missing_node" => missing_node_work_item_blocking_reason(&item.message),
        _ => None,
    }
}

fn missing_node_work_item_blocking_reason(message: &str) -> Option<&'static str> {
    if message.contains("spec-system profile config") && !message.contains("does not exist") {
        return Some("profile_config_parse_failure");
    }
    if message.contains("doc artifact ledger") {
        if message.contains("failed to read doc artifact ledger") {
            return Some("doc_artifact_ledger_missing");
        }
        if message.contains("duplicate doc artifact id") {
            return Some("duplicate_id");
        }
        if message.contains("failed to parse doc artifact ledger TOML") {
            if message.contains("unknown variant") {
                return Some("invalid_artifact_kind_or_status");
            }
            return Some("doc_artifact_ledger_parse_failure");
        }
    }
    None
}

fn support_tier_level_name(tier: SupportTierLevel) -> &'static str {
    match tier {
        SupportTierLevel::Stable => "stable",
        SupportTierLevel::Stabilizing => "stabilizing",
        SupportTierLevel::Advisory => "advisory",
    }
}

fn normalize_source_path(path: &str) -> String {
    path.trim_matches('/').replace('\\', "/")
}

fn render_string_array<T: AsRef<str>>(text: &mut String, values: &[T], indent: &str) {
    text.push_str("[\n");
    for (index, value) in values.iter().enumerate() {
        text.push_str(indent);
        text.push_str("  \"");
        text.push_str(&json_escape(value.as_ref()));
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

fn optional_bool_json(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "true",
        Some(false) => "false",
        None => "null",
    }
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
        config_provenance: ProfileConfigProvenance::LegacyPolicy.as_str().to_string(),
        mode: SpecSystemMode::Advisory,
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
        findings: vec![SpecSystemFinding::new(
            "artifact_link",
            "example structural graph finding".to_string(),
        )],
        work_items: vec![SpecSystemWorkItem {
            kind: "unknown_link_target",
            artifact_id: Some("CARGO-ALLOW-SPEC-0001".to_string()),
            path: Some("docs/specs/CARGO-ALLOW-SPEC-0001-spec-system-profile.md".to_string()),
            owner: Some("repo-infra".to_string()),
            status: Some("accepted".to_string()),
            message: "CARGO-ALLOW-SPEC-0001 linked_proposal target CARGO-ALLOW-PROP-0001 is not registered".to_string(),
            suggested_actions: vec![
                "register CARGO-ALLOW-PROP-0001 in policy/doc-artifacts.toml".to_string(),
                "or correct linked_proposal in docs/specs/CARGO-ALLOW-SPEC-0001-spec-system-profile.md".to_string(),
            ],
            proof_commands: spec_system_proof_commands(),
            ledger_id: None,
            ledger_path: None,
            lane: None,
            mode: None,
            role: None,
        }],
        readiness: None,
        federation: None,
        import_graph: None,
    };
    render_spec_system_json(&report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_policy::spec_system::ResolvedProfileConfig;

    fn test_loaded_spec_system_config(cfg: SpecSystemConfig) -> LoadedSpecSystemConfig {
        LoadedSpecSystemConfig {
            cfg,
            source: "built-in".to_string(),
            provenance: ProfileConfigProvenance::BuiltInDefault,
            path: DEFAULT_PROFILE_CONFIG.to_string(),
            found: true,
            valid: Some(true),
            diagnostic: None,
            resolved: ResolvedProfileConfig {
                path: None,
                provenance: ProfileConfigProvenance::BuiltInDefault,
                legacy_conflict_path: None,
            },
        }
    }

    fn legacy_test_config() -> SpecSystemConfig {
        let mut cfg = default_spec_system_config();
        cfg.generation = SpecSystemGeneration::LegacyV1;
        cfg.roots.goals = Some(".codex/goals".to_string());
        cfg.requirements.active_goal_required = true;
        cfg
    }

    #[test]
    fn spec_system_name_helpers_cover_all_variants() {
        assert_eq!(artifact_kind_name(ArtifactKind::Proposal), "proposal");
        assert_eq!(artifact_kind_name(ArtifactKind::Spec), "spec");
        assert_eq!(artifact_kind_name(ArtifactKind::Adr), "adr");
        assert_eq!(
            artifact_kind_name(ArtifactKind::ImplementationPlan),
            "implementation_plan"
        );
        assert_eq!(artifact_kind_name(ArtifactKind::PlanItem), "plan_item");
        assert_eq!(artifact_kind_name(ArtifactKind::ActiveGoal), "active_goal");
        assert_eq!(
            artifact_kind_name(ArtifactKind::SupportTier),
            "support_tier"
        );
        assert_eq!(
            artifact_kind_name(ArtifactKind::PolicyLedger),
            "policy_ledger"
        );
        assert_eq!(artifact_kind_name(ArtifactKind::Closeout), "closeout");
        assert_eq!(
            artifact_kind_name(ArtifactKind::ReleaseRecord),
            "release_record"
        );

        assert_eq!(artifact_status_name(ArtifactStatus::Draft), "draft");
        assert_eq!(artifact_status_name(ArtifactStatus::Proposed), "proposed");
        assert_eq!(artifact_status_name(ArtifactStatus::Accepted), "accepted");
        assert_eq!(artifact_status_name(ArtifactStatus::Active), "active");
        assert_eq!(artifact_status_name(ArtifactStatus::Done), "done");
        assert_eq!(
            artifact_status_name(ArtifactStatus::Superseded),
            "superseded"
        );

        assert_eq!(spec_system_mode_name(&SpecSystemMode::Advisory), "advisory");
        assert_eq!(spec_system_mode_name(&SpecSystemMode::Shadow), "shadow");
        assert_eq!(spec_system_mode_name(&SpecSystemMode::Blocking), "blocking");

        assert_eq!(support_tier_level_name(SupportTierLevel::Stable), "stable");
        assert_eq!(
            support_tier_level_name(SupportTierLevel::Stabilizing),
            "stabilizing"
        );
        assert_eq!(
            support_tier_level_name(SupportTierLevel::Advisory),
            "advisory"
        );
    }

    #[test]
    fn spec_system_json_helpers_escape_values_and_optional_bools() {
        assert_eq!(
            json_escape("quote: \" slash: \\ newline:\n tab:\t return:\r bell:\u{0007}"),
            "quote: \\\" slash: \\\\ newline:\\n tab:\\t return:\\r bell: "
        );
        assert_eq!(json_escape("plain"), "plain");

        assert_eq!(optional_bool_json(Some(true)), "true");
        assert_eq!(optional_bool_json(Some(false)), "false");
        assert_eq!(optional_bool_json(None), "null");
    }

    #[test]
    fn spec_system_finding_blocking_reasons_are_discriminated() {
        assert_eq!(
            spec_system_blocking_reason(
                "profile_config",
                "failed to parse spec-system config TOML: invalid type"
            ),
            Some("profile_config_parse_failure")
        );
        assert_eq!(
            spec_system_blocking_reason("profile_config", "policy/spec-system.toml does not exist"),
            None
        );
        assert_eq!(
            spec_system_blocking_reason(
                "profile_config",
                "both owned profile config `.allow/profiles/spec-system.toml` and legacy `policy/spec-system.toml` exist"
            ),
            None
        );

        assert_eq!(
            spec_system_blocking_reason(
                "doc_artifact_ledger",
                "failed to read doc artifact ledger"
            ),
            Some("doc_artifact_ledger_missing")
        );
        assert_eq!(
            spec_system_blocking_reason(
                "doc_artifact_ledger",
                "failed to parse doc artifact ledger TOML: unknown variant `bad_kind`"
            ),
            Some("invalid_artifact_kind_or_status")
        );
        assert_eq!(
            spec_system_blocking_reason(
                "doc_artifact_ledger",
                "failed to parse doc artifact ledger TOML"
            ),
            Some("doc_artifact_ledger_parse_failure")
        );
        assert_eq!(
            spec_system_blocking_reason(
                "doc_artifact_ledger",
                "duplicate doc artifact id CARGO-ALLOW-SPEC-0001"
            ),
            Some("duplicate_id")
        );

        assert_eq!(
            spec_system_blocking_reason(
                "artifact_file",
                "CARGO-ALLOW-SPEC-0001 artifact file missing: docs/specs/missing.md"
            ),
            Some("artifact_file_missing")
        );
        assert_eq!(
            spec_system_blocking_reason("artifact_file", "failed to read artifact CARGO"),
            Some("artifact_file_unreadable")
        );
        assert_eq!(
            spec_system_blocking_reason(
                "artifact_file",
                "CARGO-ALLOW-SPEC-0001 not found in artifact file docs/specs/spec.md"
            ),
            Some("artifact_id_not_in_file")
        );

        assert_eq!(
            spec_system_blocking_reason(
                "artifact_link",
                "CARGO-ALLOW-SPEC-0001 linked_proposal target CARGO-ALLOW-PROP-9999 is not registered"
            ),
            Some("unknown_link_target")
        );
        assert_eq!(
            spec_system_blocking_reason("active_goal", "stale goal"),
            None
        );
    }

    #[test]
    fn spec_system_work_item_blocking_reasons_are_discriminated() {
        assert_eq!(
            spec_system_work_item_blocking_reason(&work_item(
                "artifact_file_missing",
                "registered artifact file is missing"
            )),
            Some("artifact_file_missing")
        );
        assert_eq!(
            spec_system_work_item_blocking_reason(&work_item(
                "artifact_file_unreadable",
                "registered artifact file is unreadable"
            )),
            Some("artifact_file_unreadable")
        );
        assert_eq!(
            spec_system_work_item_blocking_reason(&work_item(
                "artifact_id_not_in_file",
                "registered artifact file does not contain its id"
            )),
            Some("artifact_id_not_in_file")
        );
        assert_eq!(
            spec_system_work_item_blocking_reason(&work_item(
                "unknown_link_target",
                "linked target is unknown"
            )),
            Some("unknown_link_target")
        );
        assert_eq!(
            spec_system_work_item_blocking_reason(&work_item(
                "missing_node",
                "spec-system profile config failed to parse"
            )),
            Some("profile_config_parse_failure")
        );
        assert_eq!(
            spec_system_work_item_blocking_reason(&work_item(
                "missing_node",
                "doc artifact ledger failed to parse doc artifact ledger TOML: unknown variant"
            )),
            Some("invalid_artifact_kind_or_status")
        );
        assert_eq!(
            spec_system_work_item_blocking_reason(&work_item(
                "missing_node",
                "doc artifact ledger duplicate doc artifact id CARGO-ALLOW-SPEC-0001"
            )),
            Some("duplicate_id")
        );
        assert_eq!(
            spec_system_work_item_blocking_reason(&work_item(
                "missing_closeout",
                "done work item has no closeout"
            )),
            None
        );
    }

    #[test]
    fn validate_active_goal_file_reports_source_path_read_errors() -> std::io::Result<()> {
        let root = temp_root("missing-active-goal")?;
        let cfg = legacy_test_config();
        let ledger = empty_doc_artifact_ledger();

        let err = match validate_active_goal_file(&root, &cfg, &ledger) {
            Ok(()) => {
                return Err(std::io::Error::other(
                    "missing active goal file should be reported",
                ));
            }
            Err(err) => err,
        };
        let _ = std::fs::remove_dir_all(&root);

        assert!(
            err.to_string()
                .contains("failed to read active goal manifest .codex/goals/active.toml"),
            "unexpected active goal read error: {err}"
        );
        Ok(())
    }

    #[test]
    fn collect_spec_system_readiness_discriminates_invalid_inputs() -> std::io::Result<()> {
        let root = temp_root("invalid-readiness")?;
        let cfg = legacy_test_config();
        for path in [
            Some(cfg.roots.proposals.as_str()),
            Some(cfg.roots.specs.as_str()),
            Some(cfg.roots.adrs.as_str()),
            Some(cfg.roots.plans.as_str()),
            cfg.roots.goals.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            std::fs::create_dir_all(root.join(path))?;
        }
        write_fixture_file(&root, &cfg.roots.artifact_ledger, "not = valid = toml")?;

        let readiness = collect_spec_system_readiness(&root, &test_loaded_spec_system_config(cfg));
        let _ = std::fs::remove_dir_all(&root);

        let ledger = readiness_check_by_kind(&readiness, "artifact_ledger");
        assert!(
            ledger.is_some(),
            "missing artifact_ledger check: {readiness:?}"
        );
        let Some(ledger) = ledger else {
            return Ok(());
        };
        assert!(ledger.found);
        assert_eq!(ledger.valid, Some(false));
        assert_eq!(ledger.status, "invalid");
        assert!(
            ledger
                .message
                .contains("failed to parse doc artifact ledger TOML"),
            "unexpected ledger message: {}",
            ledger.message
        );

        let support_tiers = readiness_check_by_kind(&readiness, "support_tiers");
        assert!(
            support_tiers.is_some(),
            "missing support_tiers check: {readiness:?}"
        );
        let Some(support_tiers) = support_tiers else {
            return Ok(());
        };
        assert!(!support_tiers.found);
        assert_eq!(support_tiers.valid, Some(false));
        assert_eq!(support_tiers.status, "missing");
        assert!(
            support_tiers
                .message
                .contains("failed to read support-tier file docs/status/SUPPORT_TIERS.md"),
            "unexpected support-tier message: {}",
            support_tiers.message
        );

        let active_goal = readiness_check_by_kind(&readiness, "active_goal");
        assert!(
            active_goal.is_some(),
            "missing active_goal check: {readiness:?}"
        );
        let Some(active_goal) = active_goal else {
            return Ok(());
        };
        assert!(!active_goal.found);
        assert_eq!(active_goal.valid, Some(false));
        assert_eq!(active_goal.status, "missing");
        assert!(
            active_goal.message.contains(
                "active goal manifest cannot be validated until doc artifact ledger parses"
            ),
            "unexpected active-goal message: {}",
            active_goal.message
        );
        Ok(())
    }

    #[test]
    fn collect_spec_system_readiness_discriminates_invalid_active_goal() -> std::io::Result<()> {
        let root = temp_root("invalid-active-goal")?;
        for file in spec_system_bootstrap_files(Path::new(DEFAULT_PROFILE_CONFIG), false) {
            write_fixture_file(&root, &file.path.display().to_string(), &file.contents)?;
        }
        write_fixture_file(
            &root,
            ".codex/goals/active.toml",
            "schema_version = 1\nstatus = []\n",
        )?;

        let readiness = collect_spec_system_readiness(
            &root,
            &test_loaded_spec_system_config(legacy_test_config()),
        );
        let _ = std::fs::remove_dir_all(&root);

        let active_goal = readiness_check_by_kind(&readiness, "active_goal");
        assert!(
            active_goal.is_some(),
            "missing active_goal check: {readiness:?}"
        );
        let Some(active_goal) = active_goal else {
            return Ok(());
        };
        assert!(active_goal.found);
        assert_eq!(active_goal.valid, Some(false));
        assert_eq!(active_goal.status, "invalid");
        assert!(
            active_goal.message.contains("active goal")
                || active_goal.message.contains("failed to parse"),
            "unexpected active-goal message: {}",
            active_goal.message
        );
        Ok(())
    }

    fn work_item(kind: &'static str, message: &'static str) -> SpecSystemWorkItem {
        SpecSystemWorkItem {
            kind,
            artifact_id: None,
            path: None,
            owner: None,
            status: None,
            message: message.to_string(),
            suggested_actions: Vec::new(),
            proof_commands: Vec::new(),
            ledger_id: None,
            ledger_path: None,
            lane: None,
            mode: None,
            role: None,
        }
    }

    fn empty_doc_artifact_ledger() -> DocArtifactLedger {
        DocArtifactLedger {
            schema_version: "1.0".to_string(),
            policy: "cargo-allow-doc-artifacts".to_string(),
            owner: "repo-infra".to_string(),
            status: SpecSystemMode::Advisory,
            artifact: Vec::new(),
        }
    }

    fn readiness_check_by_kind<'a>(
        readiness: &'a SpecSystemReadiness,
        kind: &str,
    ) -> Option<&'a SpecSystemReadinessCheck> {
        readiness.checks.iter().find(|check| check.kind == kind)
    }

    #[test]
    fn parse_spec_system_mode_override_maps_audit_and_fails_closed() {
        // #1941: `audit` (the report-only source-tree mode and the documented
        // proof command) maps to advisory, case-insensitive. The enforcing
        // source-tree modes have no spec-system meaning and must fail closed
        // instead of being silently dropped.
        for value in ["audit", "AUDIT", "  audit  "] {
            assert_eq!(
                parse_spec_system_mode_override(value).unwrap_or_else(|err| {
                    std::panic::panic_any(format!("{value} should parse: {err}"))
                }),
                SpecSystemMode::Advisory,
                "{value} should map to advisory"
            );
        }
        for value in ["no-new", "strict", "release", "blocking"] {
            let err = parse_spec_system_mode_override(value)
                .expect_err("an unsupported spec-system mode must fail closed");
            assert!(
                err.to_string()
                    .contains(&format!("--mode `{value}` is not supported")),
                "error should name the rejected value: {err}"
            );
        }
    }

    #[test]
    fn spec_system_check_mode_override_reaches_report_mode() -> std::io::Result<()> {
        // #1941: an explicit --mode must reach the spec-system evaluation and
        // override the config mode, instead of being silently dropped.
        let root = temp_root("mode-override")?;
        for file in spec_system_bootstrap_files(Path::new(DEFAULT_PROFILE_CONFIG), false) {
            write_fixture_file(&root, &file.path.display().to_string(), &file.contents)?;
        }
        let root_args = RootArgs {
            root: Some(root.clone()),
        };

        let build = |mode: Option<SpecSystemMode>| {
            build_spec_system_report("check", &root_args, None, false, false, mode)
                .unwrap_or_else(|err| std::panic::panic_any(format!("report builds: {err}")))
        };

        // `--mode blocking` forces blocking even though the bootstrap config is
        // not blocking; `--mode audit` maps to advisory; no override keeps the
        // config mode.
        let blocking = build(Some(SpecSystemMode::Blocking));
        assert_eq!(blocking.mode, SpecSystemMode::Blocking);
        let advisory = build(Some(SpecSystemMode::Advisory));
        assert_eq!(advisory.mode, SpecSystemMode::Advisory);
        let default_mode = build(None).mode;
        assert_ne!(
            default_mode,
            SpecSystemMode::Blocking,
            "override must be what forced blocking, not the config default"
        );

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    fn temp_root(name: &str) -> std::io::Result<PathBuf> {
        let mut root = std::env::temp_dir();
        let nanos = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(_) => 0,
        };
        root.push(format!(
            "cargo-allow-spec-system-{name}-{}-{}",
            std::process::id(),
            nanos
        ));
        std::fs::create_dir_all(&root)?;
        Ok(root)
    }

    fn write_fixture_file(root: &Path, relative: &str, contents: &str) -> std::io::Result<()> {
        let path = root_relative_path(root, Path::new(relative));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)
    }
}
