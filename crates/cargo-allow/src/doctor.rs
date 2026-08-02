use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult, normalize_path};
use allow_files::{FileFamilyClassification, FileScanOptions, classify_file_family_with_options};
use allow_inventory::{InventoryOptions, inventory, resolve_source_tree_root};
use allow_policy::load_policy_with_reportable_evidence;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::path::Path;

#[path = "doctor_args.rs"]
mod doctor_args;
pub(crate) use doctor_args::DoctorArgs;

use crate::{
    HumanJsonFormat, InventoryFacts, ProfileArg, SourceTreeReportContext, config_path, current_dir,
    emit_text,
    evidence_inventory::{
        PolicyReferenceDiagnostic, current_evidence_source_tree_files,
        policy_reference_diagnostics_for_source_tree,
    },
    federation_doctor::FederationDoctorFacts,
    intent_provider::{IntentProviderRequest, discover_intent_provider},
    spec_system,
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

pub(crate) fn cmd_doctor(args: &DoctorArgs) -> CargoAllowResult<()> {
    if matches!(args.profile, Some(ProfileArg::SpecSystem)) {
        return spec_system::cmd_spec_system_doctor(spec_system::SpecSystemDoctorCommandArgs {
            root: &args.root,
            config: args.config.as_deref(),
            format_json: matches!(args.format, HumanJsonFormat::Json),
            output: args.output.as_deref(),
        });
    }

    let cwd = current_dir()?;
    let root = resolve_source_tree_root(args.root.root.as_deref(), &cwd)?;
    let root_discovery = root_discovery_kind(args.root.root.as_deref(), &root);
    let config = config_path(&root, args.config.as_deref());
    let policy = load_doctor_policy(config.as_deref());
    let opts = doctor_inventory_options(policy.as_ref());
    let inventory = inventory(&root, &opts)?;
    let files_scanned = inventory.files.len();
    let empty_git_tracked = inventory.empty_git_tracked;
    let deleted_tracked_files = inventory.deleted_tracked.len();
    let git_inventory_error = inventory.git_error.as_deref();
    let skipped_paths = inventory.skipped_paths.len();
    let submodule_paths = inventory.submodule_paths.len();
    let evidence_source_tree_files = current_evidence_source_tree_files(&root, false);
    let source_context = SourceTreeReportContext::new(
        &root,
        InventoryFacts::scanned_inventory(&inventory).with_deleted_tracked(deleted_tracked_files),
    );
    let config_text = config
        .as_ref()
        .map(|path| allow_report::source_tree_path_text(path));
    let (config_valid, config_diagnostic) =
        config_status(&root, policy.as_ref(), evidence_source_tree_files.as_ref());
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
    let config_schema_version = policy
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .map(|cfg| cfg.schema_version.as_str());
    let config_policy = policy
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .map(|cfg| cfg.policy.as_str());
    let config_owner = policy
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .and_then(|cfg| cfg.owner.as_deref());
    let config_status = policy
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .and_then(|cfg| cfg.status.as_deref());
    let mut federation = FederationDoctorFacts::load(&root)?;
    federation.enrich_runtime_divergences(&root)?;
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
    let text = match args.format {
        HumanJsonFormat::Human => {
            let style = if args.output.is_none() {
                crate::reporting::output_style()
            } else {
                allow_report::Style::PLAIN
            };
            let mut rendered = allow_report::render_doctor_human_styled(report, style);
            rendered.push_str(&intent_provider_doctor_section(&root));
            rendered
        }
        HumanJsonFormat::Json => allow_report::render_doctor_json(report),
    };
    emit_text(args.output.as_deref(), &text)?;
    // --require-clean: exit non-zero if the policy is invalid or evidence
    // is broken (#1817). This lets CI gates use `doctor --require-clean`
    // as a merge blocker.
    if args.require_clean {
        if !matches!(config_valid, Some(true)) {
            return Err(CargoAllowError::new(
                "doctor --require-clean: policy config is invalid or missing",
            ));
        }
        if broken_evidence_links.unwrap_or(0) > 0 {
            let count = broken_evidence_links.unwrap_or(0);
            return Err(CargoAllowError::new(format!(
                "doctor --require-clean: {count} broken evidence link(s)",
            )));
        }
    }
    Ok(())
}

fn intent_provider_doctor_section(root: &Path) -> String {
    let request = IntentProviderRequest {
        root,
        config_path: None,
        explicit_executable: None,
    };
    match discover_intent_provider(&request) {
        Ok(resolution) => format!(
            "\nIntent provider: {:?} at {}\n",
            resolution.discovery_mode,
            resolution.executable.display()
        ),
        Err(failure) => format!("\nIntent provider: unavailable ({failure})\n"),
    }
}

fn load_doctor_policy(config: Option<&Path>) -> Option<CargoAllowResult<AllowConfig>> {
    config.map(load_policy_with_reportable_evidence)
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
