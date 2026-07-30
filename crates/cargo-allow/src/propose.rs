use allow_core::{CargoAllowError, CargoAllowResult, FindingKind, MatchStatus};
use allow_match::{CheckMode, evaluate};
use allow_policy::{render_policy, validate_policy};
use allow_report::MutationReceipt;
use repo_edit::{SingleTargetApplyMode, SingleTargetApplyRequest, apply_single_target};
use std::env;

use crate::{
    EvidenceValidationMode, HumanJsonFormat, MutationLock, SourceTreeReportContext,
    emit_stderr_text, load_world_with_evidence_mode, portable_relative_under_root,
    require_json_summary_output,
};

#[path = "propose_args.rs"]
mod propose_args;
#[path = "propose_baseline.rs"]
mod propose_baseline;
#[path = "propose_render.rs"]
mod propose_render;
#[path = "propose_types.rs"]
mod propose_types;
pub(crate) use propose_args::ProposeArgs;
use propose_baseline::{default_baseline_expiry, entry_from_finding};
#[cfg(test)]
use propose_render::render_propose_summary;
use propose_render::{render_propose_summary_json, render_propose_summary_styled};
pub(super) use propose_types::ProposeContext;

#[cfg(test)]
use allow_core::{Finding, SimpleDate};
#[cfg(test)]
use propose_baseline::BASELINE_DEBT_DEFAULT_DAYS;

pub(crate) fn cmd_propose(args: &ProposeArgs) -> CargoAllowResult<()> {
    require_json_summary_output(args.summary_format, args.summary_output.as_deref())?;
    let cwd = env::current_dir()
        .map_err(|error| CargoAllowError::new(format!("failed to read cwd: {error}")))?;
    let write_target = args.write.as_deref().map(|path| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            cwd.join(path)
        }
    });
    if let Some(target) = &write_target {
        let mutation_root = crate::resolve_source_tree_root(args.root.root.as_deref(), &cwd)?;
        crate::policy_config::assert_path_within_root(&mutation_root, target)?;
    }
    let _mutation_lock = write_target
        .as_ref()
        .map(MutationLock::acquire)
        .transpose()?;
    let (root, cfg, findings, inventory_facts, _federation) = load_world_with_evidence_mode(
        args.root.root.as_deref(),
        args.config.as_deref(),
        false,
        args.kind.as_deref(),
        args.include_untracked,
        EvidenceValidationMode::ReportOnly,
    )?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::Audit);
    let new_findings_total = outcomes
        .iter()
        .filter(|o| o.status == MatchStatus::New)
        .count();
    let mut proposed = cfg.clone();
    let start = proposed.allow.len() + 1;
    let mut proposed_entries = 0;
    let mut unsafe_proposed_entries = 0;
    let expires = args.expires.clone().unwrap_or_else(default_baseline_expiry);
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
            if finding.kind == FindingKind::Unsafe {
                unsafe_proposed_entries += 1;
            }
            proposed
                .allow
                .push(entry_from_finding(finding, start + n, &expires));
            proposed_entries += 1;
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
    let config_source = crate::policy_config::config_path(&root, args.config.as_deref())
        .map(|path| path.display().to_string());
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
        let absolute_target = write_target.as_ref().ok_or_else(|| {
            CargoAllowError::new("internal error: --write target missing after containment check")
        })?;
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
        .into_result()?;
        let _ = path;
    } else {
        println!("{rendered}");
    }
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let context = ProposeContext {
        inventory: source_context.inventory(),
        kind_filter: args.kind.as_deref(),
        mutation_receipt,
    };
    let truncated_new_findings = new_findings_total.saturating_sub(proposed_entries);
    let counts = propose_render::ProposeCounts {
        findings_scanned: findings.len(),
        proposed_entries,
        unsafe_proposed_entries,
        truncated_new_findings,
    };
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

#[cfg(test)]
pub(crate) fn sample_propose_json_for_contract_test() -> String {
    use std::path::Path;

    render_propose_summary_json(
        propose_render::ProposeCounts {
            findings_scanned: 12,
            proposed_entries: 3,
            unsafe_proposed_entries: 1,
            truncated_new_findings: 0,
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
