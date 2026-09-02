use crate::{
    EvidenceValidationMode, HumanJsonFormat, MutationLock, SourceTreeReportContext, current_dir,
    emit_stderr_text, load_world_from_resolved_policy_with_options,
    load_world_without_policy_after_selection, portable_relative_under_root,
    require_json_summary_output,
};
use allow_core::{CargoAllowError, CargoAllowResult, FindingKind, MatchStatus};
use allow_match::{CheckMode, evaluate};
use allow_policy::{
    generated_entry_rejection, ledger_self_receipt, receipts_ledger_at, render_policy,
    validate_policy,
};
use allow_report::MutationReceipt;
use effortless_repo_edit::{SingleTargetApplyMode, SingleTargetApplyRequest, apply_single_target};

#[path = "propose_args.rs"]
mod propose_args;
#[path = "propose_baseline.rs"]
mod propose_baseline;
#[path = "propose_render.rs"]
mod propose_render;
#[path = "propose_types.rs"]
mod propose_types;
pub(crate) use propose_args::ProposeArgs;

pub(crate) fn parity_propose_args(
    root: std::path::PathBuf,
    write: std::path::PathBuf,
) -> ProposeArgs {
    ProposeArgs {
        root: crate::RootArgs { root: Some(root) },
        config: None,
        kind: None,
        include_untracked: true,
        expires: Some(
            allow_core::SimpleDate::today_utc_approx()
                .add_days(30)
                .to_string(),
        ),
        write: Some(write),
        force: false,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
        max: 50,
    }
}
use propose_baseline::{default_baseline_expiry, entry_from_finding};
#[cfg(test)]
use propose_render::render_propose_summary;
use propose_render::{render_propose_summary_json, render_propose_summary_styled};
pub(super) use propose_types::ProposeContext;

#[cfg(test)]
use allow_core::{Finding, SimpleDate};
#[cfg(test)]
use propose_baseline::BASELINE_DEBT_DEFAULT_DAYS;

/// First `allow-NNNN` id not already taken by `entries`.
///
/// Generated entry ids come from the position of a finding in the `new` set,
/// not from how many entries were actually kept, so any skipped finding leaves
/// a hole and `entries.len() + 1` can land on an id already in use. Allocating
/// above the highest existing number is correct whatever the loop skipped
/// (#3035).
fn next_allow_id(entries: &[allow_core::AllowEntry]) -> String {
    let highest = entries
        .iter()
        .filter_map(|entry| entry.id.strip_prefix("allow-"))
        .filter_map(|number| number.parse::<usize>().ok())
        .max()
        .unwrap_or(0);
    format!("allow-{:04}", highest + 1)
}

const MULTIPLE_UNRECEIPTABLE_REASONS: &str = "multiple policy requirements forbid generated entries; inspect each finding and the active requirements";

fn record_unreceiptable_reason(current: &mut Option<&'static str>, next: &'static str) {
    match *current {
        None => *current = Some(next),
        Some(existing) if existing != next => *current = Some(MULTIPLE_UNRECEIPTABLE_REASONS),
        Some(_) => {}
    }
}

pub(crate) fn cmd_propose(args: &ProposeArgs) -> CargoAllowResult<()> {
    require_json_summary_output(args.summary_format, args.summary_output.as_deref())?;
    let cwd = current_dir()?;
    let write_target = args.write.as_deref().map(|path| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            cwd.join(path)
        }
    });
    let mutation_root = crate::resolve_source_tree_root(args.root.root.as_deref(), &cwd)?;
    let selection =
        crate::policy_config::select_policy_path(&mutation_root, args.config.as_deref());
    let selected_policy_path = selection.as_ref().ok().map(|(path, _)| path.clone());
    if let Some(path) = &selected_policy_path {
        crate::policy_config::assert_path_within_root(&mutation_root, path)?;
    }
    if let Some(target) = &write_target {
        crate::policy_config::assert_path_within_root(&mutation_root, target)?;
    }
    let mut collision_targets = Vec::new();
    if let Some(target) = write_target.as_deref() {
        collision_targets.push(target);
    }
    if let Some(path) = selected_policy_path.as_deref() {
        collision_targets.push(path);
    }
    crate::command_support::reject_legacy_summary_output_collision(
        &mutation_root,
        args.summary_output.as_deref(),
        &collision_targets,
    )?;
    let _mutation_lock = write_target
        .as_ref()
        .map(|target| {
            let resolved = effortless_repo_edit::resolve_mutation_target(target, &mutation_root)?;
            MutationLock::acquire_for_target(&resolved)
        })
        .transpose()
        .map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)?;
    let (root, cfg, findings, inventory_facts, _federation) = match selection {
        Ok((policy_path, federation)) => {
            let (cfg, policy_digest) = crate::policy_config::load_policy_at_path_with_digest(
                policy_path,
                EvidenceValidationMode::ReportOnly,
            )?;
            load_world_from_resolved_policy_with_options(
                &mutation_root,
                cfg,
                Some(policy_digest),
                federation,
                args.include_untracked,
                args.kind.as_deref(),
                true,
            )?
        }
        Err(error) if crate::policy_config::is_missing_policy_config_error(&error) => {
            load_world_without_policy_after_selection(
                &mutation_root,
                args.kind.as_deref(),
                args.include_untracked,
                EvidenceValidationMode::ReportOnly,
            )?
        }
        Err(error) => return Err(error),
    };
    let outcomes = evaluate(&cfg, &findings, CheckMode::Audit);
    let new_findings_total = outcomes
        .iter()
        .filter(|o| o.status == MatchStatus::New)
        .count();
    let mut proposed = cfg.clone();
    // Compute the starting ID suffix from the highest existing numeric
    // allow-NNNN id, not the entry count. Using len()+1 collides when the
    // ledger has gaps (e.g. allow-0001,allow-0002,allow-0005 → len=3 → first
    // proposed id would be allow-0004 which is free, but allow-0001..0003,0005
    // → len=4 → first proposed id allow-0005 collides). (#3231)
    let start = proposed
        .allow
        .iter()
        .filter_map(|entry| entry.id.strip_prefix("allow-"))
        .filter_map(|number| number.parse::<usize>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    let mut proposed_entries = 0;
    let mut unsafe_proposed_entries = 0;
    let mut unreceiptable_new_findings = 0;
    let mut unreceiptable_reason: Option<&'static str> = None;
    let expires = args.expires.clone().unwrap_or_else(default_baseline_expiry);
    // The ledger about to be written gets a durable receipt below, so it must
    // never also be proposed as expiring debt (#3032).
    let ledger_rel = match (&args.write, &write_target) {
        (Some(write_path), target) => {
            let absolute = target.clone().unwrap_or_else(|| root.join(write_path));
            Some(
                portable_relative_under_root(&root, &absolute)?
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/"),
            )
        }
        _ => None,
    };
    for (n, outcome) in outcomes
        .iter()
        .filter(|o| o.status == MatchStatus::New)
        .enumerate()
    {
        // --max limits the number of proposed entries to avoid overwhelming
        // first-hour adopters on noisy codebases (#1815). --max 0 means
        // unlimited.
        if args.max > 0 && n >= args.max {
            break;
        }
        if let Some(finding) = outcome.finding_index.and_then(|idx| findings.get(idx)) {
            let entry = entry_from_finding(finding, start + n, &expires);
            // An already-tracked ledger shows up as its own finding. Leave it
            // to the durable self-receipt below instead of stamping expiring
            // `baseline_debt` on the file that records the policy (#3032).
            if ledger_rel
                .as_deref()
                .is_some_and(|ledger| receipts_ledger_at(&entry, ledger))
            {
                continue;
            }
            // Never propose an entry this policy's own requirements forbid.
            // Doing so made `validate_policy` below fail on a generated id the
            // operator cannot find in their file, so `init` followed by
            // `propose` could not produce a baseline for any tree containing a
            // bare `#[allow(...)]` (#3023). The finding is not suppressed: it
            // stays `new` in `check`, and the summary reports the skip.
            if let Some(reason) = generated_entry_rejection(&proposed.requirements, &entry) {
                unreceiptable_new_findings += 1;
                record_unreceiptable_reason(&mut unreceiptable_reason, reason);
                continue;
            }
            if finding.kind == FindingKind::Unsafe {
                unsafe_proposed_entries += 1;
            }
            proposed.allow.push(entry);
            proposed_entries += 1;
        }
    }
    // A ledger that is about to be written into the source tree must receipt
    // itself, or the first `check --mode no-new` after the adopter commits it
    // fails on `policy/allow.toml` rather than on their code (#3032). Only when
    // the operator is persisting a policy, and never over an existing receipt.
    if let Some(ledger_rel) = &ledger_rel {
        // Look at the operator's own entries, not `proposed`: when the ledger
        // is already tracked and unreceipted, the loop above will have just
        // generated expiring `baseline_debt` for it, and treating that as an
        // existing receipt would leave the exact wrong lifecycle on the file.
        // Those generated entries were dropped, so re-check the source config.
        if !cfg
            .allow
            .iter()
            .any(|entry| receipts_ledger_at(entry, ledger_rel))
        {
            // `unowned` is only legal on `baseline_debt`, and the ledger's own
            // receipt is deliberately not debt, so fall back to the same
            // concrete owner the starter policy uses.
            let owner = proposed
                .owner
                .clone()
                .filter(|owner| owner.trim() != "unowned" && !owner.trim().is_empty())
                .unwrap_or_else(|| "core/policy".into());
            proposed.allow.push(ledger_self_receipt(
                &next_allow_id(&proposed.allow),
                ledger_rel,
                &owner,
            ));
        }
    }
    // Validate the complete policy before writing, matching add/prune/refresh (#2832 audit).
    validate_policy(&proposed)?;
    let rendered = render_policy(&proposed);
    let original_allow_count = cfg.allow.len();
    let changed_allow_ids = proposed
        .allow
        .iter()
        .skip(original_allow_count)
        .map(|entry| entry.id.as_str())
        .collect::<Vec<_>>();
    let after_fingerprints = proposed
        .allow
        .iter()
        .skip(original_allow_count)
        .map(|entry| Some(allow_core::allow_entry_content_fingerprint(entry)))
        .collect::<Vec<_>>();
    let repo_root = root.display().to_string();
    let config_source = selected_policy_path
        .as_deref()
        .map(|path| crate::policy_config::git_relative_selected_config_path(&root, path))
        .transpose()?
        .map(|path| path.display().to_string());
    let write_path = write_target
        .as_ref()
        .map(|path| portable_relative_under_root(&root, path))
        .transpose()?
        .map(|path| {
            path.to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/")
        });
    let mutation_receipt = MutationReceipt {
        operation: "propose",
        tool_version: env!("CARGO_PKG_VERSION"),
        repo_root: Some(repo_root.as_str()),
        config_source: config_source.as_deref(),
        ledger_ids: Vec::new(),
        before_fingerprints: vec![None; changed_allow_ids.len()],
        changed_allow_ids,
        after_fingerprints,
        result: if args.write.is_some() {
            "written"
        } else {
            "stdout"
        },
        next_commands: vec![
            "cargo-allow worklist --baseline-debt --format json".to_string(),
            "cargo-allow check --mode no-new".to_string(),
        ],
    };
    if let Some(path) = &args.write {
        let absolute_target = write_target_after_containment(write_target.as_deref())?;
        let target = portable_relative_under_root(&root, absolute_target)?;
        let mode = if args.force {
            SingleTargetApplyMode::ReplaceWithBackup
        } else {
            SingleTargetApplyMode::CreateNewOnly
        };
        apply_single_target(SingleTargetApplyRequest {
            repository_root: &root,
            target: &target,
            contents: &rendered,
            caller_reference: Some("cargo-allow:propose"),
            lock_identity: Some(
                target
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/"),
            ),
            mode,
        })
        .into_result()
        .map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)?;
        let _ = path;
    } else {
        // When printing the full policy TOML to stdout, warn interactive
        // users that the summary is on stderr so they don't miss it (#3191).
        use std::io::IsTerminal;
        if std::io::stdout().is_terminal() && args.summary_output.is_none() {
            eprintln!("note: proposed policy written to stdout; summary follows below on stderr");
        }
        println!("{rendered}");
    }
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let context = ProposeContext {
        inventory: source_context.inventory(),
        kind_filter: args.kind.as_deref(),
        mutation_receipt,
    };
    // Skipped-because-forbidden findings are not truncation: `--max 0` will
    // never propose them, so they must not be folded into that count (#3023).
    let truncated_new_findings = new_findings_total
        .saturating_sub(proposed_entries)
        .saturating_sub(unreceiptable_new_findings);
    let counts = propose_render::ProposeCounts {
        findings_scanned: findings.len(),
        proposed_entries,
        unsafe_proposed_entries,
        truncated_new_findings,
        unreceiptable_new_findings,
        unreceiptable_reason,
    };
    let core_summary = crate::core_command_summary::core_command_summary_from_propose(
        crate::core_command_summary::ProposeSummaryFactsV1 {
            repository_identity: "local-repository:current".to_string(),
            portable_identity: format!(
                "worktree:propose:{}",
                write_path.as_deref().unwrap_or("stdout")
            ),
            write_path,
            force: args.force,
            completeness: crate::core_command_router::summary_completeness(&inventory_facts),
            proposed_entries,
            unsafe_proposed_entries,
            truncated_new_findings,
            unreceiptable_new_findings,
        },
    )
    .map_err(|error| {
        CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::Internal,
            format!("failed to build propose command summary: {error}"),
        )
    })?;
    crate::core_command_router::write_summary_artifact(&root, &core_summary)?;
    if args.summary_format == HumanJsonFormat::Human {
        eprint!(
            "{}",
            crate::core_command_summary::render_core_command_summary_human(&core_summary)
        );
    }
    let summary = match args.summary_format {
        HumanJsonFormat::Human => {
            let style = if args.summary_output.is_none() {
                crate::reporting::output_style()
            } else {
                allow_report::Style::PLAIN
            };
            render_propose_summary_styled(
                counts,
                expires.as_str(),
                args.write.as_deref(),
                context,
                style,
            )
        }
        HumanJsonFormat::Json => render_propose_summary_json(
            counts,
            expires.as_str(),
            args.write.as_deref(),
            args.force,
            context,
        ),
    };
    emit_stderr_text(args.summary_output.as_deref(), &summary)?;
    Ok(())
}

fn write_target_after_containment(
    write_target: Option<&std::path::Path>,
) -> CargoAllowResult<&std::path::Path> {
    write_target.ok_or_else(|| {
        CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::Internal,
            "internal error: --write target missing after containment check",
        )
    })
}

#[cfg(test)]
pub(crate) fn sample_propose_json_for_contract_test() -> String {
    use std::path::Path;

    render_propose_summary_json(
        propose_render::ProposeCounts {
            findings_scanned: 12,
            proposed_entries: 3,
            unsafe_proposed_entries: 1,
            truncated_new_findings: 0,
            unreceiptable_new_findings: 0,
            unreceiptable_reason: None,
        },
        "2026-08-01",
        Some(Path::new("policy/allow.proposed.toml")),
        true,
        ProposeContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(51),
            ),
            kind_filter: Some("panic"),
            mutation_receipt: MutationReceipt {
                operation: "propose",
                tool_version: env!("CARGO_PKG_VERSION"),
                repo_root: Some("H:/Code/Rust/cargo-allow"),
                config_source: Some("policy/allow.toml"),
                ledger_ids: Vec::new(),
                changed_allow_ids: vec!["allow-0001", "allow-0002", "allow-0003"],
                before_fingerprints: vec![None, None, None],
                after_fingerprints: vec![
                    Some("sha256:v1:0000000000000000000000000000000000000000000000000000000000000001".to_string()),
                    Some("sha256:v1:0000000000000000000000000000000000000000000000000000000000000002".to_string()),
                    Some("sha256:v1:0000000000000000000000000000000000000000000000000000000000000003".to_string()),
                ],
                result: "written",
                next_commands: vec![
                    "cargo-allow worklist --baseline-debt --format json".to_string(),
                    "cargo-allow check --mode no-new".to_string(),
                ],
            },
        },
    )
}

#[cfg(test)]
#[path = "propose_tests.rs"]
mod tests;
