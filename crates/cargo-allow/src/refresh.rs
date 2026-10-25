use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, Finding,
    normalize_path,
};
use allow_match::{CheckMode, evaluate};
use allow_policy::validate_policy;

#[path = "refresh_args.rs"]
mod refresh_args;
#[path = "refresh_policy.rs"]
mod refresh_policy;
#[path = "refresh_render.rs"]
mod refresh_render;
#[path = "refresh_select.rs"]
mod refresh_select;
#[path = "refresh_types.rs"]
mod refresh_types;
use allow_report::MutationReceipt;
pub(crate) use refresh_args::RefreshArgs;
use refresh_render::{render_bulk_refresh_json, render_refresh_json, render_refresh_result_styled};
use refresh_select::{
    apply_last_seen_refresh, select_all_location_drift_refresh, select_location_drift_refresh,
};
use refresh_types::{RefreshContext, RefreshEmitInput, RefreshRenderInput};

pub(crate) fn parity_refresh_args(
    root: std::path::PathBuf,
    config: std::path::PathBuf,
) -> RefreshArgs {
    RefreshArgs {
        root: crate::RootArgs { root: Some(root) },
        config: Some(config),
        allow_id: Some("allow-0250".to_string()),
        all: false,
        dry_run: false,
        write: true,
        include_untracked: true,
        format: HumanJsonFormat::Human,
        output: None,
    }
}

use crate::{
    HumanJsonFormat, MutationLock, SourceTreeReportContext, current_dir, emit_text,
    evidence_inventory::{
        current_evidence_source_tree_files, validate_evidence_references_for_source_tree,
    },
    load_selected_mutation_policy, load_world_from_resolved_policy_with_options,
    portable_relative_under_root, resolve_source_tree_root, select_mutation_policy,
};
use effortless_repo_edit::{
    SingleTargetApplyMode, SingleTargetApplyRequest,
    apply_single_target_with_target_and_expected_digest,
};

pub(crate) fn cmd_refresh(args: &RefreshArgs) -> CargoAllowResult<()> {
    if args.dry_run && args.write {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            "pass either --dry-run or --write, not both",
        ));
    }
    let root = resolve_source_tree_root(args.root.root.as_deref(), current_dir()?)?;
    let (policy_path, federation) = select_mutation_policy(&root, args.config.as_deref())?;
    crate::command_support::reject_output_collision(
        &root,
        args.output.as_deref(),
        &[policy_path.as_path()],
        "--output must differ from the selected policy output",
    )?;
    let (resolved_mutation_target, mutation_lock) = if args.write {
        let target = effortless_repo_edit::resolve_mutation_target(&policy_path, &root)
            .map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)?;
        let lock = MutationLock::acquire_for_target(&target)
            .map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)?;
        (Some(target), Some(lock))
    } else {
        (None, None)
    };
    let _mutation_lock = mutation_lock;
    let (cfg, policy_digest) = load_selected_mutation_policy(&policy_path)?;
    let (root, mut cfg, findings, inventory_facts, _federation) =
        load_world_from_resolved_policy_with_options(
            &root,
            cfg,
            Some(policy_digest.clone()),
            federation,
            args.include_untracked,
            None,
            true,
        )?;
    let outcomes = evaluate(
        &cfg,
        &findings,
        CheckMode::NoNew,
        allow_core::SimpleDate::today_utc_approx(),
    );
    // #4651: `--all` was advertised by #3184 but never wired up, so the
    // command looped back to demanding `--allow-id`. Bulk mode selects every
    // location-drifted entry (policy order, first outcome per entry — the
    // same rule the single path applies) and every selection below flows
    // through the identical per-entry select, apply, validate, and render
    // steps. CLI callers cannot reach the missing-selector error (clap
    // enforces `required_unless_present = "all"`); it guards direct library
    // calls only.
    let selections = if args.all {
        select_all_location_drift_refresh(&cfg, &outcomes, &findings)?
    } else {
        let Some(allow_id) = args.allow_id.as_deref() else {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                "allow entry id is required; pass --allow-id <id> or use --all for bulk refresh",
            ));
        };
        vec![select_location_drift_refresh(
            &cfg, &outcomes, &findings, allow_id,
        )?]
    };
    let mut originals = Vec::with_capacity(selections.len());
    let mut previews = Vec::with_capacity(selections.len());
    for (entry_index, finding_index, _drift_message) in &selections {
        let finding = selected_refresh_finding(&findings, *finding_index)?;
        let original_entry = selected_refresh_entry(&cfg, *entry_index)?.clone();
        let mut preview_entry = original_entry.clone();
        apply_last_seen_refresh(&mut preview_entry, finding);
        originals.push(original_entry);
        previews.push(preview_entry);
    }
    let written_path = if args.write {
        for (entry_index, finding_index, _drift_message) in &selections {
            let finding = selected_refresh_finding(&findings, *finding_index)?;
            let entry = selected_refresh_entry_mut(&mut cfg, *entry_index)?;
            apply_last_seen_refresh(entry, finding);
        }
        validate_policy(&cfg)?;
        let evidence_source_tree_files =
            current_evidence_source_tree_files(&root, args.include_untracked);
        validate_evidence_references_for_source_tree(
            &root,
            &cfg,
            evidence_source_tree_files.as_ref(),
        )?;
        let policy_target =
            crate::policy_config::git_relative_selected_config_path(&root, &policy_path)?;
        let updates = previews
            .iter()
            .map(|preview_entry| {
                let location = preview_entry.last_seen.as_ref().ok_or_else(|| {
                    CargoAllowError::with_kind(
                        CargoAllowErrorKind::Internal,
                        "internal error: selected refresh has no coordinates",
                    )
                })?;
                Ok((preview_entry.id.as_str(), location))
            })
            .collect::<CargoAllowResult<Vec<_>>>()?;
        let rendered =
            refresh_policy::refresh_bound_policy(&policy_path, &policy_digest, &updates)?;
        apply_single_target_with_target_and_expected_digest(
            SingleTargetApplyRequest {
                repository_root: &root,
                target: &policy_target,
                contents: &rendered,
                caller_reference: Some("cargo-allow:refresh"),
                lock_identity: Some(
                    policy_target
                        .to_string_lossy()
                        .replace(std::path::MAIN_SEPARATOR, "/"),
                ),
                mode: SingleTargetApplyMode::AtomicReplace,
            },
            resolved_mutation_target.as_ref().ok_or_else(|| {
                CargoAllowError::with_kind(
                    CargoAllowErrorKind::Internal,
                    "internal error: refresh mutation target was not resolved",
                )
            })?,
            &policy_digest,
        )
        .into_result()
        .map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)?;
        Some(policy_path.clone())
    } else {
        None
    };
    let repo_root = allow_core::strip_win32_verbatim_prefix(&root.display().to_string());
    let config_source = allow_core::strip_win32_verbatim_prefix(&policy_path.display().to_string());
    let emit_inputs = selections
        .iter()
        .zip(&originals)
        .zip(&previews)
        .map(|((selection, original_entry), preview_entry)| {
            let mutation_receipt = MutationReceipt {
                operation: "refresh",
                tool_version: env!("CARGO_PKG_VERSION"),
                repo_root: Some(&repo_root),
                config_source: Some(&config_source),
                ledger_ids: Vec::new(),
                changed_allow_ids: vec![original_entry.id.as_str()],
                before_fingerprints: vec![Some(allow_core::allow_entry_content_fingerprint(
                    original_entry,
                ))],
                after_fingerprints: vec![Some(allow_core::allow_entry_content_fingerprint(
                    preview_entry,
                ))],
                result: if args.write { "written" } else { "stdout" },
                next_commands: vec![
                    format!("cargo-allow explain {}", original_entry.id),
                    "cargo-allow check --mode no-new".to_string(),
                ],
            };
            let entry = if args.write {
                selected_refresh_entry(&cfg, selection.0)?
            } else {
                preview_entry
            };
            Ok(RefreshEmitInput {
                entry,
                finding: selected_refresh_finding(&findings, selection.1)?,
                previous_last_seen: original_entry.last_seen.clone(),
                drift_message: &selection.2,
                root: &root,
                policy_path: &policy_path,
                inventory_facts,
                written_path: written_path.as_deref(),
                mutation_receipt,
            })
        })
        .collect::<CargoAllowResult<Vec<_>>>()?;
    render_and_emit(args, &emit_inputs)
}

fn selected_refresh_finding(
    findings: &[Finding],
    finding_index: usize,
) -> CargoAllowResult<&Finding> {
    findings.get(finding_index).ok_or_else(|| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            "internal error: selected finding index out of range",
        )
    })
}

fn selected_refresh_entry(
    config: &AllowConfig,
    entry_index: usize,
) -> CargoAllowResult<&AllowEntry> {
    config.allow.get(entry_index).ok_or_else(|| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            "internal error: selected allow entry index out of range",
        )
    })
}

fn selected_refresh_entry_mut(
    config: &mut AllowConfig,
    entry_index: usize,
) -> CargoAllowResult<&mut AllowEntry> {
    config.allow.get_mut(entry_index).ok_or_else(|| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            "internal error: selected allow entry index out of range",
        )
    })
}

fn render_and_emit(args: &RefreshArgs, inputs: &[RefreshEmitInput<'_>]) -> CargoAllowResult<()> {
    let first = inputs.first().ok_or_else(|| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            "internal error: refresh produced no selections to render",
        )
    })?;
    let source_context = SourceTreeReportContext::new(first.root, first.inventory_facts);
    // One routing summary per invocation (#4651): with `--all` it names the
    // first refreshed entry, and each per-entry report below carries its own
    // mutation receipt with the complete per-entry change set.
    let policy_path = portable_relative_under_root(first.root, first.policy_path)?
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    let core_summary = crate::core_command_summary::core_command_summary_from_refresh(
        crate::core_command_summary::RefreshSummaryFactsV1 {
            repository_identity: format!(
                "local-repository:{}",
                first.inventory_facts.source.as_str()
            ),
            portable_identity: format!("worktree:refresh:{}:{}", policy_path, first.entry.id),
            root_path: normalize_path(first.root),
            policy_path,
            allow_id: first.entry.id.clone(),
            write_requested: args.write,
            dry_run: args.dry_run,
            completeness: crate::core_command_router::summary_completeness(&first.inventory_facts),
        },
    )
    .map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            format!("failed to build refresh command summary: {error}"),
        )
    })?;
    crate::core_command_router::write_summary_artifact(first.root, &core_summary)?;
    if args.format == HumanJsonFormat::Human {
        eprint!(
            "{}",
            crate::core_command_summary::render_core_command_summary_human(&core_summary)
        );
    }
    let style = if args.output.is_none() {
        crate::reporting::output_style()
    } else {
        allow_report::Style::PLAIN
    };
    let written = first
        .written_path
        .map(|path| allow_core::strip_win32_verbatim_prefix(&path.display().to_string()));
    let written_ref = written.as_deref();
    let documents: Vec<String> = inputs
        .iter()
        .map(|input| {
            let context = RefreshContext {
                inventory: source_context.inventory(),
            };
            let render_input = RefreshRenderInput {
                entry: input.entry,
                finding: input.finding,
                previous_last_seen: input.previous_last_seen.clone(),
                drift_message: input.drift_message,
                dry_run: args.dry_run,
                write_requested: args.write,
                written_path: written_ref,
                context,
                mutation_receipt: input.mutation_receipt.clone(),
            };
            match args.format {
                HumanJsonFormat::Human => render_refresh_result_styled(render_input, style),
                HumanJsonFormat::Json => render_refresh_json(render_input),
            }
        })
        .collect();
    // Single-entry output stays byte-identical to the pre-#4651 artifact.
    // Bulk output joins the per-entry reports: human output concatenates
    // them, JSON wraps the documented `cargo-allow.refresh.v1` documents in
    // one parseable array so `--all` never emits a malformed stream (#4651).
    let text = match args.format {
        HumanJsonFormat::Human => documents.concat(),
        HumanJsonFormat::Json => {
            if documents.len() == 1 {
                documents.into_iter().next().ok_or_else(|| {
                    CargoAllowError::with_kind(
                        CargoAllowErrorKind::Internal,
                        "internal error: refresh produced no selections to render",
                    )
                })?
            } else {
                render_bulk_refresh_json(&documents)
            }
        }
    };
    emit_text(args.output.as_deref(), &text)?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn sample_refresh_json_for_contract_test() -> String {
    use allow_core::{Finding, FindingKind, LastSeen, Selector, Span, StructuralIdentity};
    let entry = allow_core::AllowEntry {
        id: "allow-0250".to_string(),
        kind: FindingKind::LintException,
        family: Some("expect".to_string()),
        path: Some("src/lib.rs".into()),
        glob: None,
        owner: "lint".to_string(),
        classification: "reviewed_lint_exception".to_string(),
        reason: "Fixture refresh receipt".to_string(),
        evidence: vec!["test:refresh-receipt-fixture".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: allow_core::Lifecycle {
            created: Some("2026-05-09".to_string()),
            review_after: Some("2026-09-09".to_string()),
            expires: None,
        },
        selector: Selector::default(),
        last_seen: Some(LastSeen {
            line: 22,
            column: 4,
        }),
    };
    let finding = Finding {
        kind: FindingKind::LintException,
        family: Some("expect".to_string()),
        path: "src/lib.rs".into(),
        identity: StructuralIdentity::new("rust", "attribute"),
        message: "fixture refresh drift".to_string(),
        ledger: None,
        span: Some(Span {
            line: 22,
            column: 4,
        }),
    };
    render_refresh_json(RefreshRenderInput {
        entry: &entry,
        finding: &finding,
        previous_last_seen: Some(LastSeen {
            line: 14,
            column: 8,
        }),
        drift_message: "allow-drift last_seen changed from 14:8 to 22:4",
        dry_run: true,
        write_requested: false,
        written_path: None,
        context: RefreshContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "filesystem_include_untracked",
                Some("tests/fixtures/refresh/advisory-drift"),
                Some(2),
            ),
        },
        mutation_receipt: allow_report::MutationReceipt {
            operation: "refresh",
            tool_version: env!("CARGO_PKG_VERSION"),
            repo_root: Some("tests/fixtures/refresh/advisory-drift"),
            config_source: Some("policy/allow.toml"),
            ledger_ids: Vec::new(),
            changed_allow_ids: vec!["allow-0250"],
            before_fingerprints: vec![Some(
                "sha256:v1:0000000000000000000000000000000000000000000000000000000000000000"
                    .to_string(),
            )],
            after_fingerprints: vec![Some(
                "sha256:v1:1111111111111111111111111111111111111111111111111111111111111111"
                    .to_string(),
            )],
            result: "stdout",
            next_commands: vec!["cargo-allow check --mode no-new".to_string()],
        },
    })
}

#[cfg(test)]
#[path = "refresh_tests.rs"]
mod tests;
