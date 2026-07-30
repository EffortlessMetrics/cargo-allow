use allow_core::{CargoAllowError, CargoAllowResult, read_text_file_capped};
use allow_inventory::resolve_source_tree_root;
use allow_policy::federation::{FederationLoadOutcome, load_federation_config};
use allow_policy::spec_system::{
    ArtifactKind, ArtifactStatus, DocArtifact, DocArtifactLedger, ProfileConfigProvenance,
    ResolvedProfileConfig, SpecSystemConfig, SpecSystemGeneration, SpecSystemMode,
    SpecSystemRequirements, SpecSystemRoots, SupportTierLevel, load_doc_artifacts,
    parse_spec_system_config_at, parse_support_tier_claims, profile_config_conflict_message,
    validate_active_goal_manifest_text_at, validate_doc_artifact_files,
    validate_doc_artifact_links, validate_support_tier_claims,
};
use std::fs;
use std::path::{Path, PathBuf};

use crate::spec_system_view::render_self_hosted_explain;
use crate::{OutputFormat, RootArgs, current_dir, emit_text, root_relative_path, write_file};

#[path = "spec_system_render.rs"]
mod spec_system_render;
use spec_system_render::{
    filter_spec_system_report_for_artifact, render_spec_system_explain_markdown,
    render_spec_system_json, render_spec_system_markdown, render_spec_system_report,
};

#[path = "spec_system_bootstrap.rs"]
mod spec_system_bootstrap;
use spec_system_bootstrap::{
    legacy_bootstrap_conflicts, spec_system_bootstrap_files, spec_system_legacy_compatibility,
};

#[path = "spec_system_config.rs"]
mod spec_system_config;
use spec_system_config::{load_spec_system_config, profile_config_findings};

#[path = "spec_system_readiness.rs"]
mod spec_system_readiness;
use spec_system_readiness::{
    active_goal_manifest_source_path, collect_spec_system_readiness, validate_active_goal_file,
};

#[path = "spec_system_graph.rs"]
mod spec_system_graph;
use spec_system_graph::{
    SpecSystemFederationSummary, SpecSystemImportGraphSummary, discover_spec_system_import_graph,
    federation_config_findings, import_graph_findings, import_graph_summary_from_graph,
    spec_system_federation_summary, work_items_from_import_graph,
};

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
    let cwd = current_dir()?;
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
    let cwd = current_dir()?;
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

fn build_spec_system_report(
    command: &str,
    root_args: &RootArgs,
    config: Option<&Path>,
    include_work_items: bool,
    include_readiness: bool,
    mode_override: Option<SpecSystemMode>,
) -> CargoAllowResult<SpecSystemReport> {
    let cwd = current_dir()?;
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
    let import_graph = discover_spec_system_import_graph(&root, cfg.import_roots.as_ref());
    findings.extend(import_graph_findings(&import_graph));
    if include_work_items {
        work_items.extend(work_items_from_import_graph(&import_graph));
    }
    let import_graph_summary = Some(import_graph_summary_from_graph(&import_graph));
    let federation = spec_system_federation_summary(&root, &mut work_items);
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
#[path = "spec_system_tests.rs"]
mod tests;
