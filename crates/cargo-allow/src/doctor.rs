use allow_core::{
    AllowConfig, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, normalize_path,
};
use allow_files::{FileFamilyClassification, FileScanOptions, classify_file_family_with_options};
use allow_inventory::{InventoryOptions, inventory, resolve_source_tree_root};
#[cfg(test)]
use allow_policy::load_policy_with_reportable_evidence;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::path::Path;

#[path = "doctor_args.rs"]
mod doctor_args;
pub(crate) use doctor_args::DoctorArgs;

use crate::{
    HumanJsonFormat, InventoryFacts, ProfileArg, SourceTreeReportContext, assert_path_within_root,
    current_dir, emit_text,
    evidence_inventory::{
        PolicyReferenceDiagnostic, current_evidence_source_tree_files,
        policy_reference_diagnostics_for_source_tree,
    },
    federation_doctor::FederationDoctorFacts,
    intent_provider::{
        INTENT_PROVIDER_CANONICAL_COMMAND, INTENT_PROVIDER_REQUIRED_PROTOCOL,
        INTENT_PROVIDER_REQUIRED_VERSION_RANGE, INTENT_PROVIDER_SUPPORT_REFERENCE,
        IntentProviderFailureClass, IntentProviderRequest, discover_intent_provider,
    },
    portable_relative_under_root, spec_system,
    support_bundle::{SupportBundleFacts, write_support_bundle},
    world::CoreWorldContext,
};

#[derive(Debug, Default)]
struct DoctorFileFamilyFacts {
    rules: Vec<DoctorFileFamilyRule>,
    conflicts: Vec<DoctorFileFamilyConflict>,
}

#[derive(Debug)]
struct DoctorFileFamilyRule {
    id: String,
    family: String,
    glob: String,
    matched_files: usize,
}

#[derive(Debug)]
struct DoctorFileFamilyConflict {
    path: String,
    rule_ids: Vec<String>,
    families: Vec<String>,
}

/// The doctor report combines the canonical resolved configuration context
/// with diagnostic-only observations.  Keeping this boundary explicit prevents
/// diagnostics from becoming a second configuration selector while preserving
/// the existing fail-closed status reporting for malformed or missing policy.
struct DoctorWorldContext {
    core: Option<CoreWorldContext>,
    rust_scan: allow_rust::RustScanResult,
}

pub(crate) fn cmd_doctor(args: &DoctorArgs) -> CargoAllowResult<()> {
    if matches!(args.profile, Some(ProfileArg::SpecSystem)) {
        if args.support_bundle.is_some() {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                "--support-bundle is only supported for the source-exception doctor profile",
            ));
        }
        return spec_system::cmd_spec_system_doctor(spec_system::SpecSystemDoctorCommandArgs {
            root: &args.root,
            config: args.config.as_deref(),
            format_json: matches!(args.format, HumanJsonFormat::Json),
            output: args.output.as_deref(),
        });
    }

    let cwd = current_dir()?;
    let root = resolve_source_tree_root(args.root.root.as_deref(), &cwd)?;
    if let Some(path) = &args.support_bundle {
        assert_path_within_root(&root, path)?;
    }
    let root_discovery = root_discovery_kind(args.root.root.as_deref(), &root);
    let observed =
        crate::policy_config::observe_policy_for_diagnostics(&root, args.config.as_deref());
    let config_discovery = observed.discovery;
    let config = config_discovery.path.clone();
    let policy = observed.policy;
    let opts = doctor_inventory_options(policy.as_ref());
    let inventory = inventory(&root, &opts)?;
    let rust_scan = allow_rust::scan_rust_files(&root, &inventory.files)?;
    let files_scanned = inventory.files.len();
    let empty_git_tracked = inventory.empty_git_tracked;
    let deleted_tracked_files = inventory.deleted_tracked.len();
    let git_inventory_error = inventory.git_error.as_deref();
    let skipped_paths = inventory.skipped_paths.len();
    let submodule_paths = inventory.submodule_paths.len();
    let evidence_source_tree_files = current_evidence_source_tree_files(&root, false);
    let doctor_inventory_facts =
        InventoryFacts::scanned_inventory(&inventory).with_deleted_tracked(deleted_tracked_files);
    let config_text = config
        .as_ref()
        .map(|path| allow_report::source_tree_path_text(path));
    let (mut config_valid, mut config_diagnostic) =
        config_status(&root, policy.as_ref(), evidence_source_tree_files.as_ref());
    if config_discovery.federation_evaluation_failed {
        config_valid = Some(false);
        if config_diagnostic.is_none() {
            config_diagnostic = Some(
                "federation configuration could not be evaluated; conventional fallback is not clean"
                    .to_string(),
            );
        }
    }
    let (broken_evidence_links, weak_evidence_references) =
        doctor_evidence_health(&root, policy.as_ref(), evidence_source_tree_files.as_ref());
    let file_family_facts = doctor_file_family_facts(&inventory.files, policy.as_ref());
    let file_family_rules = file_family_facts
        .rules
        .iter()
        .map(|rule| allow_report::FileFamilyRuleSummary {
            id: rule.id.as_str(),
            family: rule.family.as_str(),
            glob: rule.glob.as_str(),
            matched_files: rule.matched_files,
        })
        .collect::<Vec<_>>();
    let file_family_conflicts = file_family_facts
        .conflicts
        .iter()
        .map(|conflict| allow_report::FileFamilyConflictSummary {
            path: conflict.path.as_str(),
            rule_ids: conflict.rule_ids.as_slice(),
            families: conflict.families.as_slice(),
        })
        .collect::<Vec<_>>();
    let mut federation = FederationDoctorFacts::load(&root)?;
    federation.enrich_runtime_divergences(&root)?;
    if config_discovery.federation_evaluation_failed
        || (config_discovery.precedence == Some(allow_policy::PrecedenceTier::DiscoveryFallback)
            && config_discovery.federation_invalid_observed)
        || (config_discovery.precedence == Some(allow_policy::PrecedenceTier::DiscoveryFallback)
            && federation.valid == Some(false))
    {
        config_valid = Some(false);
        if config_diagnostic.is_none() {
            config_diagnostic = Some(
                "federation configuration is invalid; conventional fallback is not clean"
                    .to_string(),
            );
        }
    }
    let core_context = if let Some(cfg) = policy.as_ref().and_then(|result| result.as_ref().ok()) {
        let mut findings = rust_scan.findings.clone();
        findings.extend(allow_files::scan_files_with_options(
            &inventory.files,
            &FileScanOptions {
                generated: opts.generated.clone(),
                file_families: cfg.workspace.file_families.clone(),
                content_aware_generated: false,
            },
        ));
        if let Ok(companion_findings) =
            crate::canonical_companion_findings(&root, cfg, &inventory.files)
        {
            crate::extend_unique_findings(&mut findings, companion_findings);
        }
        let federation = config_discovery
            .federation
            .clone()
            .unwrap_or_else(crate::world::default_federation_evaluation);
        if let Some(provenance) = federation.active_provenance.clone() {
            for finding in &mut findings {
                finding.ledger = Some(provenance.clone());
            }
        }
        Some(CoreWorldContext {
            root: root.clone(),
            cfg: cfg.clone(),
            findings,
            inventory_facts: InventoryFacts::scanned_inventory(&inventory)
                .with_deleted_tracked(deleted_tracked_files)
                .with_rust_files_considered(rust_scan.files_considered)
                .with_rust_files_skipped(rust_scan.files_skipped)
                .with_rust_files_with_parse_errors(rust_scan.files_with_parse_errors),
            federation,
        })
    } else {
        None
    };
    let doctor_context = DoctorWorldContext {
        core: core_context,
        rust_scan,
    };
    let rust_scan = &doctor_context.rust_scan;
    let source_context = SourceTreeReportContext::new(&root, doctor_inventory_facts);
    let config_schema_version = doctor_context
        .core
        .as_ref()
        .map(|core| core.cfg.schema_version.as_str())
        .or_else(|| {
            policy
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .map(|cfg| cfg.schema_version.as_str())
        });
    let config_policy = doctor_context
        .core
        .as_ref()
        .map(|core| core.cfg.policy.as_str())
        .or_else(|| {
            policy
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .map(|cfg| cfg.policy.as_str())
        });
    let config_owner = doctor_context
        .core
        .as_ref()
        .and_then(|core| core.cfg.owner.as_deref())
        .or_else(|| {
            policy
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .and_then(|cfg| cfg.owner.as_deref())
        });
    let config_status = doctor_context
        .core
        .as_ref()
        .and_then(|core| core.cfg.status.as_deref())
        .or_else(|| {
            policy
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .and_then(|cfg| cfg.status.as_deref())
        });
    let configured_ledgers = federation.configured_ledger_summaries();
    let federation_diagnostics = federation.diagnostic_summaries();
    let federation_divergences = federation.divergence_summaries();
    let report = allow_report::DoctorReport {
        source_tree_root: source_context.source_tree_root(),
        root_discovery,
        config_path: config_text.as_deref(),
        config_schema_version,
        config_policy,
        config_owner,
        config_status,
        config_provenance: config.as_ref().and(config_discovery.source).map(|source| {
            allow_report::ConfigProvenanceSummary {
                source,
                precedence: config_discovery.precedence.map(|tier| tier.as_str()),
            }
        }),
        config_valid,
        config_diagnostic: config_diagnostic.as_deref(),
        broken_evidence_links,
        weak_evidence_references,
        inventory_source: source_context.inventory_source(),
        inventory_completeness: source_context.inventory_completeness(),
        files_scanned,
        empty_git_tracked,
        deleted_tracked_files,
        git_inventory_error,
        skipped_paths,
        submodule_paths,
        rust_scanner_completeness: rust_scanner_completeness(rust_scan),
        rust_files_considered: rust_scan.files_considered,
        rust_files_scanned: rust_scan
            .files_considered
            .saturating_sub(rust_scan.files_skipped),
        rust_files_skipped: rust_scan.files_skipped,
        rust_files_with_parse_errors: rust_scan.files_with_parse_errors,
        rust_files_skipped_by_read_or_unsupported: rust_scan.files_skipped,
        federation_config_path: federation.federation_config_path(),
        federation_config_found: federation.found,
        federation_config_valid: federation.valid,
        configured_ledgers: if configured_ledgers.is_empty() {
            None
        } else {
            Some(configured_ledgers.as_slice())
        },
        federation_diagnostics: if federation_diagnostics.is_empty() {
            None
        } else {
            Some(federation_diagnostics.as_slice())
        },
        federation_divergences: if federation_divergences.is_empty() {
            None
        } else {
            Some(federation_divergences.as_slice())
        },
        file_family_rules: file_family_rules.as_slice(),
        file_family_conflicts: file_family_conflicts.as_slice(),
    };
    if let Some(output) = &args.support_bundle {
        let support_config_path = config
            .as_ref()
            .and_then(|path| {
                path.exists()
                    .then(|| portable_relative_under_root(&root, path).ok())
            })
            .flatten()
            .map(|path| path.to_string_lossy().replace('\\', "/"));
        write_support_bundle(
            &root,
            output,
            SupportBundleFacts {
                root_discovery,
                repository_kind: if root.join(".git").exists() {
                    "git"
                } else {
                    "non_git"
                },
                config_found: config.is_some(),
                config_path: support_config_path.as_deref(),
                config_schema_version,
                config_valid,
                inventory_source: source_context.inventory_source(),
                inventory_completeness: source_context.inventory_completeness(),
                files_scanned,
                deleted_tracked_files,
                skipped_paths,
                submodule_paths,
                federation_found: federation.found,
                federation_valid: federation.valid,
            },
        )?;
    }
    // Common operator grammar (#3149). The detailed doctor report remains
    // authoritative; this projection is additive and derived from the same
    // already-computed facts without rescanning source or reloading policy.
    let summary = doctor_summary(
        report,
        &root,
        &source_context,
        DoctorSetupFacts {
            config_present: config.is_some(),
            config_valid,
            config_diagnostic: config_diagnostic.as_deref(),
            broken_evidence_links,
            weak_evidence_references,
        },
    )?;
    crate::core_command_router::write_summary_artifact(&root, &summary)?;

    let text = match args.format {
        HumanJsonFormat::Human => {
            let style = if args.output.is_none() {
                crate::reporting::output_style()
            } else {
                allow_report::Style::PLAIN
            };
            let mut rendered =
                crate::core_command_summary::render_core_command_summary_human(&summary);
            rendered.push('\n');
            rendered.push_str(&allow_report::render_doctor_human_styled(report, style));
            rendered.push_str(&intent_provider_doctor_section(&root));
            if let Some(path) = &args.support_bundle {
                rendered.push_str(&format!("\nSupport bundle written to {}\n", path.display()));
            }
            rendered
        }
        HumanJsonFormat::Json => allow_report::render_doctor_json(report),
    };
    emit_text(args.output.as_deref(), &text)?;
    // --require-clean: exit non-zero if the policy is invalid or evidence
    // is broken (#1817). This lets CI gates use `doctor --require-clean`
    // as a merge blocker.
    if args.require_clean && (rust_scan.files_skipped > 0 || rust_scan.files_with_parse_errors > 0)
    {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::PolicyViolation,
            format!(
                "doctor --require-clean: Rust scanner coverage is partial ({} skipped, {} parse errors)",
                rust_scan.files_skipped, rust_scan.files_with_parse_errors
            ),
        ));
    }
    if args.require_clean && !matches!(config_valid, Some(true)) {
        let kind = match policy.as_ref() {
            None => CargoAllowErrorKind::InvalidConfig,
            Some(Err(_)) => CargoAllowErrorKind::InvalidPolicy,
            Some(Ok(_)) => CargoAllowErrorKind::PolicyViolation,
        };
        return Err(CargoAllowError::with_kind(
            kind,
            "doctor --require-clean: policy config is invalid or missing",
        ));
    }
    Ok(())
}

fn rust_scanner_completeness(scan: &allow_rust::RustScanResult) -> &'static str {
    if scan.files_considered == 0 {
        "unknown"
    } else if scan.files_skipped > 0 || scan.files_with_parse_errors > 0 {
        "partial"
    } else {
        "complete"
    }
}

/// Build the common operator summary from facts doctor has already computed.
///
/// Doctor's own JSON artifact supplies the relocation-stable semantic identity,
/// so the summary never rescans source or reloads policy to describe itself.
struct DoctorSetupFacts<'a> {
    config_present: bool,
    config_valid: Option<bool>,
    config_diagnostic: Option<&'a str>,
    /// `None` means evidence health was not probed, not that it is clean.
    broken_evidence_links: Option<usize>,
    weak_evidence_references: Option<usize>,
}

fn doctor_summary(
    report: allow_report::DoctorReport<'_>,
    root: &Path,
    source_context: &SourceTreeReportContext,
    setup: DoctorSetupFacts<'_>,
) -> CargoAllowResult<crate::core_command_summary::CoreCommandSummaryV1> {
    let semantic_identity = crate::core_command_router::canonical_semantic_identity(
        &allow_report::render_doctor_json(report),
        Some(root),
    )?;
    let inventory_source = source_context.inventory_source();
    let (completeness, coverage_limitation) = doctor_completeness(report, source_context);

    let mut subject = crate::core_command_summary::CoreSourceSubjectV1::worktree(
        format!("local-repository:{semantic_identity}"),
        format!("worktree:{inventory_source}:current-unpinned"),
    );
    subject.limitations.push(
        "the current worktree result is not bound to a commit, tree, or Git-index identity"
            .to_string(),
    );

    crate::core_command_summary::core_command_summary_from_doctor(
        crate::core_command_summary::DoctorSummaryFactsV1 {
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            subject,
            completeness,
            coverage_limitation,
            config_present: setup.config_present,
            config_valid: setup.config_valid,
            config_diagnostic: setup.config_diagnostic.map(str::to_string),
            broken_evidence_links: setup.broken_evidence_links,
            weak_evidence_references: setup.weak_evidence_references,
            claim_boundary: effortless_repo_protocol::ClaimBoundaryV1::new(
                "cargo-allow diagnosed source-exception setup health only",
            )
            .with_limitations(vec![
                "cargo metadata, rustc, Clippy, build scripts, proc macros, tests, and repository code were not invoked"
                    .to_string(),
                "macro expansion, type information, MIR, control flow, and data flow were not analyzed"
                    .to_string(),
                "a healthy setup does not prove the repository passes the no-new gate".to_string(),
            ]),
        },
    )
    .map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            format!("failed to build core command summary: {error}"),
        )
    })
}

/// Map doctor's inventory facts onto summary coverage.
///
/// Mirrors the audit/check router: a scoped inventory is complete for the paths
/// it claims, while absent tracked files, an empty Git inventory, skipped
/// paths, or a fallback inventory make the diagnosis non-conclusive.
fn doctor_completeness(
    report: allow_report::DoctorReport<'_>,
    source_context: &SourceTreeReportContext,
) -> (effortless_repo_protocol::CompletenessV1, Option<String>) {
    use effortless_repo_protocol::CompletenessV1;

    let mut reasons = Vec::new();
    if report.empty_git_tracked {
        reasons.push("Git reported no tracked files".to_string());
    }
    if report.deleted_tracked_files > 0 {
        reasons.push(format!(
            "{} tracked path(s) are absent from the worktree",
            report.deleted_tracked_files
        ));
    }
    if report.skipped_paths > 0 {
        reasons.push(format!("{} path(s) were skipped", report.skipped_paths));
    }
    if report.rust_files_skipped > 0 {
        reasons.push(format!(
            "{} Rust file(s) were skipped",
            report.rust_files_skipped
        ));
    }
    if report.rust_files_with_parse_errors > 0 {
        reasons.push(format!(
            "{} Rust file(s) contained parse errors",
            report.rust_files_with_parse_errors
        ));
    }
    if report.git_inventory_error.is_some() {
        reasons.push("the Git inventory reported an error".to_string());
    }
    if matches!(
        source_context.inventory_completeness(),
        "partial" | "fallback"
    ) {
        reasons.push(format!(
            "inventory completeness is {}",
            source_context.inventory_completeness()
        ));
    }

    if reasons.is_empty() {
        (CompletenessV1::Complete, None)
    } else {
        (CompletenessV1::Partial, Some(reasons.join("; ")))
    }
}

fn intent_provider_doctor_section(root: &Path) -> String {
    let request = IntentProviderRequest {
        root,
        config_path: None,
        explicit_executable: None,
    };
    match discover_intent_provider(&request) {
        Ok(resolution) => format!(
            "\nIntent provider\n  status: available\n  discovery: {:?}\n  binary: {}\n  version: not probed\n  required version range: {INTENT_PROVIDER_REQUIRED_VERSION_RANGE}\n  required protocol: {INTENT_PROVIDER_REQUIRED_PROTOCOL}\n  canonical command: {INTENT_PROVIDER_CANONICAL_COMMAND}\n  support reference: {INTENT_PROVIDER_SUPPORT_REFERENCE}\n",
            resolution.discovery_mode,
            resolution.executable.display()
        ),
        Err(failure) => render_intent_provider_failure(&failure),
    }
}

fn render_intent_provider_failure(
    failure: &crate::intent_provider::IntentProviderFailure,
) -> String {
    let status = if failure.class == IntentProviderFailureClass::Absent {
        "unavailable"
    } else {
        "incompatible"
    };
    let detected = if failure.class == IntentProviderFailureClass::Absent {
        "none detected"
    } else {
        "binary detected, version not probed"
    };
    format!(
        "\nIntent provider\n  status: {status}\n  detected binary/version: {detected}\n  required version range: {INTENT_PROVIDER_REQUIRED_VERSION_RANGE}\n  required protocol: {INTENT_PROVIDER_REQUIRED_PROTOCOL}\n  canonical command: {INTENT_PROVIDER_CANONICAL_COMMAND}\n  support reference: {INTENT_PROVIDER_SUPPORT_REFERENCE}\n  intent evaluation: not performed\n  clean claim: not available\n  reason: {failure}\n"
    )
}

fn doctor_file_family_facts(
    files: &[std::path::PathBuf],
    policy: Option<&CargoAllowResult<AllowConfig>>,
) -> DoctorFileFamilyFacts {
    let Some(Ok(cfg)) = policy else {
        return DoctorFileFamilyFacts::default();
    };
    let options = FileScanOptions {
        generated: cfg.workspace.generated.clone(),
        file_families: cfg.workspace.file_families.clone(),
        content_aware_generated: false,
    };
    let mut matched_files = cfg
        .workspace
        .file_families
        .iter()
        .map(|rule| (rule.id.clone(), 0usize))
        .collect::<HashMap<_, _>>();
    let mut conflicts = Vec::new();
    for path in files {
        match classify_file_family_with_options(path, &options) {
            Some(FileFamilyClassification::Custom { rule_id, .. }) => {
                if let Some(count) = matched_files.get_mut(&rule_id) {
                    *count += 1;
                }
            }
            Some(FileFamilyClassification::Ambiguous { rule_ids, families }) => {
                conflicts.push(DoctorFileFamilyConflict {
                    path: normalize_path(path),
                    rule_ids,
                    families,
                })
            }
            Some(FileFamilyClassification::Generated)
            | Some(FileFamilyClassification::BuiltIn(_))
            | None => {}
        }
    }
    conflicts.sort_by(|left, right| left.path.cmp(&right.path));
    let rules = cfg
        .workspace
        .file_families
        .iter()
        .map(|rule| DoctorFileFamilyRule {
            id: rule.id.clone(),
            family: rule.family.clone(),
            glob: rule.glob.clone(),
            matched_files: matched_files.get(&rule.id).copied().unwrap_or(0),
        })
        .collect();
    DoctorFileFamilyFacts { rules, conflicts }
}

#[cfg(test)]
fn load_doctor_policy(config: Option<&Path>) -> Option<CargoAllowResult<AllowConfig>> {
    config.map(load_policy_with_reportable_evidence)
}

fn config_status(
    root: &Path,
    policy: Option<&CargoAllowResult<AllowConfig>>,
    source_tree_files: Option<&BTreeSet<String>>,
) -> (Option<bool>, Option<String>) {
    match policy {
        None => (None, None),
        Some(Ok(cfg)) => match first_broken_evidence_diagnostic(root, cfg, source_tree_files) {
            Some((entry_id, reference)) => (
                Some(false),
                Some(format!(
                    "{} {} `{}`: {}",
                    entry_id,
                    reference.source.label(),
                    reference.diagnostic.raw,
                    reference.source.message(&reference.diagnostic.message)
                )),
            ),
            None => (Some(true), None),
        },
        Some(Err(err)) => (Some(false), Some(err.to_string())),
    }
}

fn doctor_evidence_health(
    root: &Path,
    policy: Option<&CargoAllowResult<AllowConfig>>,
    source_tree_files: Option<&BTreeSet<String>>,
) -> (Option<usize>, Option<usize>) {
    match policy {
        Some(Ok(cfg)) => {
            let diagnostics = evidence_diagnostics(root, cfg, source_tree_files);
            let broken = diagnostics
                .iter()
                .filter(|reference| reference.diagnostic.status.is_broken_local_link())
                .count();
            let weak = diagnostics
                .iter()
                .filter(|reference| reference.diagnostic.status.is_weak_reference())
                .count();
            (Some(broken), Some(weak))
        }
        _ => (None, None),
    }
}

fn first_broken_evidence_diagnostic(
    root: &Path,
    cfg: &AllowConfig,
    source_tree_files: Option<&BTreeSet<String>>,
) -> Option<(String, PolicyReferenceDiagnostic)> {
    cfg.allow.iter().find_map(|entry| {
        policy_reference_diagnostics_for_source_tree(root, entry, source_tree_files)
            .into_iter()
            .find(|reference| reference.diagnostic.status.is_broken_local_link())
            .map(|reference| (entry.id.clone(), reference))
    })
}

fn evidence_diagnostics(
    root: &Path,
    cfg: &AllowConfig,
    source_tree_files: Option<&BTreeSet<String>>,
) -> Vec<PolicyReferenceDiagnostic> {
    cfg.allow
        .iter()
        .flat_map(|entry| {
            policy_reference_diagnostics_for_source_tree(root, entry, source_tree_files)
        })
        .collect()
}

fn doctor_inventory_options(policy: Option<&CargoAllowResult<AllowConfig>>) -> InventoryOptions {
    match policy {
        Some(Ok(cfg)) => InventoryOptions {
            ignored: cfg.workspace.ignored.clone(),
            generated: cfg.workspace.generated.clone(),
            include_untracked: false,
        },
        _ => InventoryOptions::default(),
    }
}

fn root_discovery_kind(explicit_root: Option<&Path>, root: &Path) -> &'static str {
    if explicit_root.is_some() {
        "explicit_root"
    } else if root.join(".git").exists() {
        "nearest_git_root"
    } else {
        "current_directory_fallback"
    }
}

#[cfg(test)]
pub(crate) fn sample_doctor_json_for_contract_test() -> String {
    allow_report::render_doctor_json(allow_report::DoctorReport {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: Some("H:/Code/Rust/cargo-allow/policy/allow.toml"),
        config_schema_version: Some("0.1"),
        config_policy: Some("cargo-allow"),
        config_owner: Some("core/policy"),
        config_status: Some("active"),
        config_provenance: None,
        config_valid: Some(true),
        config_diagnostic: None,
        broken_evidence_links: Some(0),
        weak_evidence_references: Some(0),
        inventory_source: "git_tracked",
        inventory_completeness: "scoped",
        files_scanned: 50,
        empty_git_tracked: false,
        deleted_tracked_files: 0,
        git_inventory_error: None,
        skipped_paths: 0,
        submodule_paths: 0,
        rust_scanner_completeness: "unknown",
        rust_files_considered: 0,
        rust_files_scanned: 0,
        rust_files_skipped: 0,
        rust_files_with_parse_errors: 0,
        rust_files_skipped_by_read_or_unsupported: 0,
        federation_config_path: None,
        federation_config_found: false,
        federation_config_valid: None,
        configured_ledgers: None,
        federation_diagnostics: None,
        federation_divergences: None,
        file_family_rules: &[],
        file_family_conflicts: &[],
    })
}

#[cfg(test)]
#[path = "doctor_tests.rs"]
mod tests;
