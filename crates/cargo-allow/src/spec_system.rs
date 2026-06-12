use allow_core::{CargoAllowError, CargoAllowResult};
use allow_inventory::resolve_source_tree_root;
use allow_policy::spec_system::{
    ArtifactKind, ArtifactStatus, DocArtifact, DocArtifactLedger, SpecSystemConfig, SpecSystemMode,
    SpecSystemRequirements, SpecSystemRoots, SupportTierLevel, load_doc_artifacts,
    parse_spec_system_config, parse_support_tier_claims, validate_doc_artifact_files,
    validate_doc_artifact_links, validate_support_tier_claims,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{OutputFormat, RootArgs, emit_text, root_relative_path, write_file};

const PROFILE_NAME: &str = "spec-system";
const DEFAULT_PROFILE_CONFIG: &str = "policy/spec-system.toml";
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
}

pub(crate) fn cmd_spec_system(args: SpecSystemCommandArgs<'_>) -> CargoAllowResult<()> {
    let report = build_spec_system_report(args.command, args.root, args.config, false, false)?;
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
    let report = build_spec_system_report("worklist", args.root, args.config, true, false)?;
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
    let report = build_spec_system_report("doctor", args.root, args.config, true, true)?;
    let rendered = if args.format_json {
        render_spec_system_json(&report)
    } else {
        render_spec_system_markdown(&report)
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
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    let root = resolve_source_tree_root(args.root.root.as_deref(), cwd)?;
    let config_path = args
        .config
        .unwrap_or_else(|| Path::new(DEFAULT_PROFILE_CONFIG));
    let files = spec_system_bootstrap_files(config_path);

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

    Ok(())
}

#[derive(Debug)]
struct SpecSystemReport {
    command: String,
    root: PathBuf,
    config_source: String,
    mode: SpecSystemMode,
    artifacts: Vec<SpecSystemArtifact>,
    links: Vec<SpecSystemLink>,
    support_tier_rows: usize,
    findings: Vec<SpecSystemFinding>,
    work_items: Vec<SpecSystemWorkItem>,
    readiness: Option<SpecSystemReadiness>,
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

#[derive(Debug)]
struct SpecSystemWorkItem {
    kind: &'static str,
    artifact_id: Option<String>,
    path: Option<String>,
    owner: Option<String>,
    status: Option<String>,
    message: String,
    suggested_actions: Vec<String>,
    proof_commands: Vec<String>,
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
    path: String,
    found: bool,
    valid: Option<bool>,
    diagnostic: Option<String>,
}

struct SpecSystemBootstrapFile {
    path: PathBuf,
    contents: String,
}

fn spec_system_bootstrap_files(config_path: &Path) -> Vec<SpecSystemBootstrapFile> {
    let mut files = vec![
        bootstrap_file(config_path, spec_system_config_template()),
        bootstrap_file(
            Path::new("policy/doc-artifacts.toml"),
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
        bootstrap_file(
            Path::new(".codex/goals/README.md"),
            artifact_root_readme(
                "Active Goals",
                "agent execution state that points at repo truth",
            ),
        ),
        bootstrap_file(
            Path::new(".codex/goals/active.toml"),
            active_goal_template(),
        ),
        bootstrap_file(Path::new(".codex/goals/archive/.gitkeep"), String::new()),
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
    files
}

fn bootstrap_file(path: &Path, contents: String) -> SpecSystemBootstrapFile {
    SpecSystemBootstrapFile {
        path: path.to_path_buf(),
        contents,
    }
}

fn spec_system_config_template() -> String {
    r#"schema_version = "1.0"
profile = "spec-system"
mode = "advisory"

[roots]
proposals = "docs/proposals"
specs = "docs/specs"
adrs = "docs/adr"
plans = "plans"
goals = ".codex/goals"
support_tiers = "docs/status/SUPPORT_TIERS.md"
artifact_ledger = "policy/doc-artifacts.toml"

[requirements]
ledger_required = true
templates_required = true
support_tiers_required = true
active_goal_required = true
closeout_required_for_done_items = true
"#
    .to_string()
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
        "# {title}\n\nThis directory contains spec-system artifacts for {role}.\n\nRegister governed artifacts in `policy/doc-artifacts.toml` so `cargo-allow check --profile spec-system` can validate their source-tree graph links.\n"
    )
}

fn active_goal_template() -> String {
    r#"schema_version = "1.0"

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
) -> CargoAllowResult<SpecSystemReport> {
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    let root = resolve_source_tree_root(root_args.root.as_deref(), cwd)?;
    let loaded_config = load_spec_system_config(&root, config);
    let cfg = loaded_config.cfg.clone();
    let config_source = loaded_config.source.clone();
    let mut findings = profile_config_findings(&loaded_config, config.is_some());
    let mut artifacts = Vec::new();
    let mut links = Vec::new();
    let mut support_tier_rows = 0;
    let mut work_items = Vec::new();

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
                        "create policy/doc-artifacts.toml with registered source-of-truth artifacts"
                            .to_string(),
                        "or correct the configured artifact_ledger path in policy/spec-system.toml"
                            .to_string(),
                    ],
                ));
            }
        }
    }

    let support_tiers_path = root_relative_path(&root, Path::new(&cfg.roots.support_tiers));
    match fs::read_to_string(&support_tiers_path) {
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
                            "or correct the configured support_tiers path in policy/spec-system.toml"
                                .to_string(),
                        ],
                        proof_commands: spec_system_proof_commands(),
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
                        "or correct the configured support_tiers path in policy/spec-system.toml"
                            .to_string(),
                    ],
                    proof_commands: spec_system_proof_commands(),
                });
            }
        }
    }

    if include_work_items {
        work_items.extend(work_items_from_config_findings(&findings));
    }
    let readiness = if include_readiness {
        Some(collect_spec_system_readiness(&root, &loaded_config))
    } else {
        None
    };

    Ok(SpecSystemReport {
        command: command.to_string(),
        root,
        config_source,
        mode: cfg.mode,
        artifacts,
        links,
        support_tier_rows,
        findings,
        work_items,
        readiness,
    })
}

fn load_spec_system_config(root: &Path, config: Option<&Path>) -> LoadedSpecSystemConfig {
    let config_path = config
        .map(|path| root_relative_path(root, path))
        .unwrap_or_else(|| root.join(DEFAULT_PROFILE_CONFIG));
    let config_path_text = root_relative_display(root, &config_path);

    if !config_path.exists() {
        return LoadedSpecSystemConfig {
            cfg: default_spec_system_config(),
            source: "default spec-system roots".to_string(),
            path: config_path_text.clone(),
            found: false,
            valid: None,
            diagnostic: Some(format!(
                "spec-system profile config {} does not exist",
                config_path.display()
            )),
        };
    }

    match fs::read_to_string(&config_path) {
        Ok(text) => match parse_spec_system_config(&text) {
            Ok(cfg) => LoadedSpecSystemConfig {
                cfg,
                source: config_path_text.clone(),
                path: config_path_text,
                found: true,
                valid: Some(true),
                diagnostic: None,
            },
            Err(err) => LoadedSpecSystemConfig {
                cfg: default_spec_system_config(),
                source: "default spec-system roots".to_string(),
                path: config_path_text,
                found: true,
                valid: Some(false),
                diagnostic: Some(err.to_string()),
            },
        },
        Err(err) => LoadedSpecSystemConfig {
            cfg: default_spec_system_config(),
            source: "default spec-system roots".to_string(),
            path: config_path_text,
            found: true,
            valid: Some(false),
            diagnostic: Some(format!(
                "failed to read spec-system profile config {}: {err}",
                config_path.display()
            )),
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
        findings.push(SpecSystemFinding::new(kind, err.to_string()));
    }
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
                "spec-system profile config parsed".to_string()
            } else {
                "spec-system profile config is missing; built-in roots are in use".to_string()
            }
        }),
    ));

    for (label, path) in [
        ("artifact_root", cfg.roots.proposals.as_str()),
        ("artifact_root", cfg.roots.specs.as_str()),
        ("artifact_root", cfg.roots.adrs.as_str()),
        ("artifact_root", cfg.roots.plans.as_str()),
        ("artifact_root", cfg.roots.goals.as_str()),
    ] {
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
    checks.push(readiness_check(
        "artifact_ledger",
        Some(cfg.roots.artifact_ledger.clone()),
        ledger_path.is_file(),
        Some(ledger_result.is_ok()),
        match ledger_result {
            Ok(_) => format!("doc artifact ledger {} parsed", cfg.roots.artifact_ledger),
            Err(err) => err.to_string(),
        },
    ));

    let support_tiers_path = root_relative_path(root, Path::new(&cfg.roots.support_tiers));
    let support_tiers_result = fs::read_to_string(&support_tiers_path)
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

    let active_goal = format!("{}/active.toml", cfg.roots.goals.trim_end_matches('/'));
    let active_goal_path = root_relative_path(root, Path::new(&active_goal));
    checks.push(readiness_check(
        "active_goal",
        Some(active_goal.clone()),
        active_goal_path.is_file(),
        Some(active_goal_path.is_file()),
        if active_goal_path.is_file() {
            format!("active goal manifest {active_goal} exists")
        } else {
            format!("active goal manifest {active_goal} is missing")
        },
    ));

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

        match fs::read_to_string(&source_path) {
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
                    "create policy/spec-system.toml".to_string(),
                    "or pass --config with the intended spec-system profile config".to_string(),
                ],
            )
        })
        .collect()
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
    text.push_str(&format!(
        "| {} findings | {} |\n",
        spec_system_mode_title(&report.mode),
        report.findings.len()
    ));
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
            "No spec-system {} findings.\n\n",
            spec_system_mode_name(&report.mode)
        ));
    } else {
        text.push_str(&format!(
            "## {} Findings\n\n",
            spec_system_mode_title(&report.mode)
        ));
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

fn spec_system_mode_title(mode: &SpecSystemMode) -> &'static str {
    match mode {
        SpecSystemMode::Advisory => "Advisory",
        SpecSystemMode::Shadow => "Shadow",
        SpecSystemMode::Blocking => "Blocking",
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
        "doc_artifact_ledger" => doc_artifact_ledger_blocking_reason(message),
        "artifact_file" => artifact_file_blocking_reason(message),
        "artifact_link" => artifact_link_blocking_reason(message),
        _ => None,
    }
}

fn profile_config_blocking_reason(message: &str) -> Option<&'static str> {
    if message.contains("does not exist") {
        return None;
    }
    Some("profile_config_parse_failure")
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
        }],
        readiness: None,
    };
    render_spec_system_json(&report)
}
