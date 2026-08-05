use allow_core::{
    AllowEntry, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, Finding, FindingKind,
    MatchOutcome, SimpleDate, json_escape,
};
use allow_match::{CheckMode, evaluate};
use allow_policy::{render_policy, validate_policy};

#[path = "add_args.rs"]
mod add_args;
#[path = "add_entry.rs"]
mod add_entry;
#[path = "add_from_plan.rs"]
mod add_from_plan;
#[path = "add_render.rs"]
mod add_render;
#[path = "add_types.rs"]
mod add_types;
pub(crate) use add_args::AddArgs;
pub(crate) use add_entry::select_add_finding;
use add_entry::{
    AddBroadRequest, AddEntryRequest, allow_entry_broad, allow_entry_from_finding,
    count_in_scope_findings, ensure_addable_outcome, next_allow_id,
};
#[cfg(test)]
use add_render::render_add_summary;
use add_render::{add_mutation_receipt, render_add_summary_json, render_add_summary_styled};
pub(super) use add_types::AddContext;

use crate::{
    HumanJsonFormat, MutationLock, SourceTreeReportContext, config_path, current_dir,
    emit_stderr_text,
    evidence_inventory::{
        current_evidence_source_tree_files, validate_evidence_references_for_source_tree,
    },
    git_relative_config_path, load_world, parse_kind_filter, portable_relative_under_root,
    require_json_summary_output, resolve_source_tree_root,
};
use repo_edit::{SingleTargetApplyMode, SingleTargetApplyRequest, apply_single_target};

const ADD_REVIEW_AFTER_DEFAULT_DAYS: i64 = 90;

#[cfg(test)]
use std::path::PathBuf;

pub(crate) fn cmd_add(args: &AddArgs) -> CargoAllowResult<()> {
    require_json_summary_output(args.summary_format, args.summary_output.as_deref())?;
    if let Some(plan_path) = args.from_plan.as_deref() {
        return add_from_plan::cmd_add_from_plan(args, plan_path);
    }
    let kind = args.kind.as_deref().ok_or_else(|| {
        CargoAllowError::with_kind(CargoAllowErrorKind::Usage, "--kind is required")
    })?;
    let parsed_kind = parse_kind_filter(kind)?;
    let cwd = current_dir()?;
    let mutation_root = resolve_source_tree_root(args.root.root.as_deref(), &cwd)?;
    // Clap enforces this mutual exclusion at parse time via
    // `conflicts_with = "write"` on `--update`. This guard is the direct-call
    // safety net for callers that construct `AddArgs` without going through the
    // parser (e.g. tests), so the update branch below can never run alongside a
    // `--write` target.
    if args.update && args.write.is_some() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "pass either --update or --write, not both",
        ));
    }
    // When --update is set, the operator expects an in-place ledger mutation.
    // Pre-check for a discovered policy config before load_world so the
    // no-policy error mentions --update explicitly (matching add_from_plan's
    // message at add_from_plan.rs:127), instead of the generic load_world
    // error that doesn't reference the update operation.
    if args.update && config_path(&mutation_root, args.config.as_deref()).is_none() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            "no policy config found to update; run `cargo-allow init` or pass --config",
        ));
    }
    let mutation_target = args
        .write
        .as_deref()
        .map(|path| {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                cwd.join(path)
            }
        })
        .or_else(|| config_path(&mutation_root, args.config.as_deref()));
    // #2487: assert the mutation target is within the source-tree root
    // before acquiring the lock, preventing out-of-tree writes.
    if let Some(target) = &mutation_target {
        crate::policy_config::assert_path_within_root(&mutation_root, target)?;
    }
    let _mutation_lock = mutation_target
        .as_ref()
        .map(MutationLock::acquire)
        .transpose()?;
    let (root, mut cfg, findings, inventory_facts, _federation) = load_world(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        Some(kind),
        args.include_untracked,
    )?;
    let id = args.id.clone().unwrap_or_else(|| next_allow_id(&cfg));
    ensure_unique_allow_id(cfg.allow.iter().map(|entry| entry.id.as_str()), &id)?;
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let context = AddContext {
        inventory: source_context.inventory(),
        repo_root: Some(root.display().to_string()),
        config_source: config_path(&root, args.config.as_deref())
            .map(|path| path.display().to_string()),
    };
    // For the mutation receipt's `result` field: --update writes the live
    // ledger, so report the discovered config path; --write reports its target;
    // otherwise stdout (None).
    let policy_output: Option<String> = if args.update {
        config_path(&root, args.config.as_deref()).map(|path| path.display().to_string())
    } else {
        args.write.as_deref().map(|path| path.display().to_string())
    };

    let (entry, summary) = if let Some(glob) = args.glob.clone() {
        // --glob: broad-scope baseline. Build a broad selector, count current
        // in-scope findings, and pin that count as occurrence_limit so the
        // entry is a ratchet floor (#2056).
        if args.path.is_some() || args.line.is_some() {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                "--glob is mutually exclusive with --path/--line",
            ));
        }
        require_add_evidence_for_kind(parsed_kind.kind, &args.evidence)?;
        let mut broad = allow_entry_broad(AddBroadRequest {
            id,
            kind: parsed_kind.kind,
            family: args.family.clone(),
            callee: args.callee.clone(),
            glob: glob.clone(),
            owner: args.owner.clone(),
            classification: args.classification.clone(),
            reason: args.reason.clone(),
            evidence: args.evidence.clone(),
            review_after: args
                .review_after
                .clone()
                .unwrap_or_else(default_add_review_after),
            expires: args.expires.clone(),
        });
        let count = count_in_scope_findings(&findings, &broad);
        if count == 0 {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                format!(
                    "no current {} findings match glob `{}`; cannot baseline an empty scope",
                    kind, glob
                ),
            ));
        }
        broad.occurrence_limit = Some(count);
        let summary = match args.summary_format {
            HumanJsonFormat::Human => {
                let style = if args.summary_output.is_none() {
                    crate::reporting::output_style()
                } else {
                    allow_report::Style::PLAIN
                };
                render_add_summary_broad_human(&broad, policy_output.as_deref(), style)
            }
            HumanJsonFormat::Json => render_add_summary_broad_json(
                &broad,
                policy_output.as_deref(),
                args.force,
                &context,
            ),
        };
        (broad, summary)
    } else {
        // --path/--line: receipt one specific occurrence (structurally anchored).
        let path = args.path.as_ref().ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                "either --glob or --path (with --line) is required",
            )
        })?;
        let line = args.line.ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                "--line is required with --path (or use --glob)",
            )
        })?;
        let outcomes = evaluate(&cfg, &findings, CheckMode::Audit);
        let (finding_index, finding) = select_add_finding(&findings, parsed_kind, path, line)?;
        let selected_outcome = selected_add_outcome(&outcomes, finding_index)?;
        ensure_addable_outcome(selected_outcome.status)?;
        require_add_evidence(finding, &args.evidence)?;
        let entry = allow_entry_from_finding(AddEntryRequest {
            finding,
            id,
            owner: args.owner.clone(),
            classification: args.classification.clone(),
            reason: args.reason.clone(),
            evidence: args.evidence.clone(),
            review_after: args
                .review_after
                .clone()
                .unwrap_or_else(default_add_review_after),
            expires: args.expires.clone(),
        });
        let summary = match args.summary_format {
            HumanJsonFormat::Human => {
                let style = if args.summary_output.is_none() {
                    crate::reporting::output_style()
                } else {
                    allow_report::Style::PLAIN
                };
                render_add_summary_styled(&entry, finding, policy_output.as_deref(), context, style)
            }
            HumanJsonFormat::Json => render_add_summary_json(
                &entry,
                finding,
                policy_output.as_deref(),
                args.force,
                context,
            ),
        };
        (entry, summary)
    };

    cfg.allow.push(entry);
    validate_policy(&cfg)?;
    let evidence_source_tree_files =
        current_evidence_source_tree_files(&root, args.include_untracked);
    validate_evidence_references_for_source_tree(&root, &cfg, evidence_source_tree_files.as_ref())?;
    let rendered = render_policy(&cfg);
    if args.update {
        let policy_target = git_relative_config_path(&root, args.config.as_deref())?;
        apply_single_target(SingleTargetApplyRequest {
            repository_root: &root,
            target: &policy_target,
            contents: &rendered,
            caller_reference: Some("cargo-allow:add"),
            lock_identity: Some(
                policy_target
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/"),
            ),
            mode: SingleTargetApplyMode::AtomicReplace,
        })
        .into_result()?;
    } else if args.write.is_some() {
        let absolute_target = mutation_target.as_ref().ok_or_else(|| {
            CargoAllowError::new("internal error: --write target missing after containment check")
        })?;
        let target = portable_relative_under_root(&mutation_root, absolute_target)?;
        let mode = if args.force {
            SingleTargetApplyMode::ReplaceWithBackup
        } else {
            SingleTargetApplyMode::CreateNewOnly
        };
        apply_single_target(SingleTargetApplyRequest {
            repository_root: &mutation_root,
            target: &target,
            contents: &rendered,
            caller_reference: Some("cargo-allow:add"),
            lock_identity: Some(
                target
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/"),
            ),
            mode,
        })
        .into_result()?;
    } else {
        println!("{rendered}");
        eprintln!(
            "Nothing was persisted. Rerun with --update to write this entry into the live policy."
        );
    }
    emit_stderr_text(args.summary_output.as_deref(), &summary)?;
    Ok(())
}

fn ensure_unique_allow_id<'a>(
    existing_ids: impl IntoIterator<Item = &'a str>,
    id: &str,
) -> CargoAllowResult<()> {
    if existing_ids
        .into_iter()
        .any(|existing_id| existing_id == id)
    {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            format!(
                "allow entry id `{id}` already exists; pass a unique --id or omit --id to auto-assign"
            ),
        ));
    }
    Ok(())
}

fn require_add_evidence_for_kind(kind: FindingKind, evidence: &[String]) -> CargoAllowResult<()> {
    let label = match kind {
        FindingKind::Unsafe => Some("unsafe"),
        _ => None,
    };
    let Some(label) = label else {
        return Ok(());
    };
    if evidence.is_empty() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            format!("{label} allow entries require at least one --evidence reference"),
        ));
    }
    if evidence
        .iter()
        .any(|reference| evidence_reference_is_typed(reference))
    {
        return Ok(());
    }
    Err(CargoAllowError::with_kind(
        CargoAllowErrorKind::Usage,
        format!(
            "{label} allow entries require at least one typed --evidence reference with a recognized non-empty prefix:value target"
        ),
    ))
}

fn render_add_summary_broad_human(
    entry: &AllowEntry,
    policy_output: Option<&str>,
    style: allow_report::Style,
) -> String {
    let target = policy_output.unwrap_or("stdout");
    format!(
        "added {} {} (kind={}, scope={}, occurrence_limit={}); policy written to {}\n",
        style.status("baseline_debt", "broad baseline"),
        entry.id,
        entry.kind,
        entry.path_or_glob(),
        entry.occurrence_limit.unwrap_or(0),
        target
    )
}

fn render_add_summary_broad_json(
    entry: &AllowEntry,
    policy_output: Option<&str>,
    force: bool,
    context: &AddContext<'_>,
) -> String {
    let action = if force { "overwrite" } else { "write" };
    let mutation_receipt = add_mutation_receipt(entry, context, policy_output);
    format!(
        "{{\"id\":\"{}\",\"kind\":\"{}\",\"scope\":\"{}\",\"occurrence_limit\":{},\"policy_output\":\"{}\",\"action\":\"{}\",\"mutation_receipt\":{}}}",
        json_escape(&entry.id),
        json_escape(&entry.kind.to_string()),
        json_escape(&entry.path_or_glob()),
        entry.occurrence_limit.unwrap_or(0),
        json_escape(policy_output.unwrap_or("stdout")),
        json_escape(action),
        allow_report::render_mutation_receipt_json(&mutation_receipt, ""),
    )
}

fn selected_add_outcome(
    outcomes: &[MatchOutcome],
    finding_index: usize,
) -> CargoAllowResult<&MatchOutcome> {
    outcomes
        .iter()
        .find(|outcome| outcome.finding_index == Some(finding_index))
        .ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Internal,
                "selected finding did not produce a match outcome",
            )
        })
}

fn require_add_evidence(finding: &Finding, evidence: &[String]) -> CargoAllowResult<()> {
    let Some(label) = add_evidence_required_label(finding) else {
        return Ok(());
    };
    if evidence.is_empty() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            format!("{label} allow entries require at least one --evidence reference"),
        ));
    }
    if evidence
        .iter()
        .any(|reference| evidence_reference_is_typed(reference))
    {
        return Ok(());
    }
    Err(CargoAllowError::with_kind(
        CargoAllowErrorKind::Usage,
        format!(
            "{label} allow entries require at least one typed --evidence reference with a recognized non-empty prefix:value target"
        ),
    ))
}

fn add_evidence_required_label(finding: &Finding) -> Option<String> {
    match (finding.kind, finding.family.as_deref()) {
        (FindingKind::Unsafe, _) => Some("unsafe".to_string()),
        (FindingKind::PolicyException, Some("process_spawn" | "network_destination")) => finding
            .family
            .as_ref()
            .map(|family| format!("policy_exception.{family}")),
        _ => None,
    }
}

fn evidence_reference_is_typed(reference: &str) -> bool {
    let Some((prefix, target)) = reference.split_once(':') else {
        return false;
    };
    let prefix = prefix.trim();
    let target = target.trim();
    !target.is_empty() && allow_policy::recognized_evidence_prefixes().any(|known| known == prefix)
}

fn default_add_review_after() -> String {
    SimpleDate::today_utc_approx()
        .add_days(ADD_REVIEW_AFTER_DEFAULT_DAYS)
        .to_string()
}

#[cfg(test)]
pub(crate) fn sample_add_json_for_contract_test() -> String {
    let add_finding = Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity: allow_core::StructuralIdentity::new("file", "method_call"),
        message: "test finding".to_string(),
        ledger: None,
    };
    let add_entry = allow_entry_from_finding(AddEntryRequest {
        finding: &add_finding,
        id: "allow-add-json".to_string(),
        owner: "parser".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Parser validates the input before unwrapping.".to_string(),
        evidence: vec!["test:parser_validates_input".to_string()],
        review_after: "2026-11-01".to_string(),
        expires: None,
    });
    render_add_summary_json(
        &add_entry,
        &add_finding,
        Some("policy/allow.proposed.toml"),
        false,
        AddContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(48),
            ),
            repo_root: Some("H:/Code/Rust/cargo-allow".to_string()),
            config_source: Some("policy/allow.toml".to_string()),
        },
    )
}

#[cfg(test)]
pub(crate) fn sample_add_plan_application_json_for_contract_test() -> String {
    let digest = "sha256:v1:0000000000000000000000000000000000000000000000000000000000000000";
    let after = "sha256:v1:1111111111111111111111111111111111111111111111111111111111111111";
    allow_report::render_add_plan_application_json(&allow_report::AddPlanApplicationV1 {
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        inventory: allow_report::InventoryContext::source_syntax(
            "git_tracked",
            Some("H:/repo"),
            Some(3),
        )
        .with_completeness("complete"),
        plan_digest: digest.to_string(),
        repository_identity: digest.to_string(),
        finding_digest: digest.to_string(),
        target_ledger: "policy/allow.toml".to_string(),
        policy_before_digest: digest.to_string(),
        policy_after_digest: after.to_string(),
        added_allow_id: "allow-0007".to_string(),
        targeted_recheck: "not_executed".to_string(),
        full_check_argv: vec![
            "check".to_string(),
            "--mode".to_string(),
            "no-new".to_string(),
        ],
    })
}

#[cfg(test)]
#[path = "add_artifact_tests.rs"]
mod artifact_tests;
#[cfg(test)]
#[path = "add_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "add_tests.rs"]
mod tests;
