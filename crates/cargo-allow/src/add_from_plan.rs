//! `cargo-allow add --from-plan <path>`: apply a versioned add-finding plan
//! (produced by `why --plan`) to the live ledger as a fail-closed transaction.
//!
//! The plan carries **coordinates and identity bindings only** — never operator
//! judgment. This command re-scans the live source tree, recomputes every
//! binding, and refuses to touch policy unless the exact finding recorded in the
//! plan is still uniquely `New` and every recomputed digest matches. The allow
//! entry itself is constructed canonically from the freshly re-selected finding
//! plus the operator's CLI judgment fields, never by deserializing approval
//! metadata out of the plan.
//!
//! Policy is written exactly once, at the very end, after full validation. Any
//! stale or malformed input returns early and leaves the ledger untouched.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult, sha256_v1_bytes};
use allow_match::{CheckMode, evaluate};
use allow_policy::{render_policy, validate_policy};
use allow_report::{
    ADD_FINDING_PLAN_SCHEMA_ID, ADD_FINDING_PLAN_SCHEMA_VERSION, AddPlanApplicationV1,
    render_add_plan_application_json,
};
use serde::Deserialize;
use serde_json::Value;

use super::{
    AddArgs, AddEntryRequest, allow_entry_from_finding, default_add_review_after,
    ensure_addable_outcome, ensure_unique_allow_id, next_allow_id, require_add_evidence,
    select_add_finding,
};
use crate::plan_bindings::{PlanFindingBindings, compute_plan_finding_bindings, read_bound_file};
use crate::{
    MutationLock, SourceTreeReportContext, config_path, current_dir, emit_stderr_text,
    evidence_inventory::{
        current_evidence_source_tree_files, validate_evidence_references_for_source_tree,
    },
    git_relative_config_path, load_world, parse_kind_filter, resolve_source_tree_root,
};
use effortless_repo_edit::{SingleTargetApplyMode, SingleTargetApplyRequest, apply_single_target};

/// Strictly-parsed `cargo-allow.add-finding-plan.v1` envelope. `deny_unknown_fields`
/// The parse models exactly the fields the transaction reads; other v1 fields
/// (proof plans, candidates, human-readable inventory, claim boundary) are
/// intentionally ignored here — the load-bearing strictness is the explicit
/// generation check in [`validate_plan_generation`] plus recomputing and
/// comparing every binding, not tolerating a display field we never consult.
/// Missing or mistyped required fields still fail the parse.
#[derive(Deserialize)]
struct LoadedPlan {
    schema_version: u32,
    schema_id: String,
    tool: String,
    command: String,
    repository: LoadedRepository,
    inventory_basis_identity: String,
    policy: LoadedPolicy,
    finding: LoadedFinding,
    outcome: LoadedOutcome,
}

#[derive(Deserialize)]
struct LoadedRepository {
    identity: String,
    root: String,
}

#[derive(Deserialize)]
struct LoadedPolicy {
    path: String,
    digest: String,
}

#[derive(Deserialize)]
struct LoadedFinding {
    kind: String,
    family: Option<String>,
    path: String,
    line: Option<usize>,
    column: Option<usize>,
    identity: BTreeMap<String, Value>,
    digest: String,
    source_file_digest: String,
    selector: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct LoadedOutcome {
    status: String,
}

fn stale(message: impl Into<String>) -> CargoAllowError {
    CargoAllowError::with_kind(
        CargoAllowErrorKind::Usage,
        format!(
            "add --from-plan rejected: {} (policy unchanged)",
            message.into()
        ),
    )
}

fn plan_input_error(error: CargoAllowError) -> CargoAllowError {
    error.with_kind_preserving_metadata(CargoAllowErrorKind::Usage)
}

/// Append a plan-regeneration hint to an add --from-plan rejection error. This
/// prevents the operator from being stuck: they know exactly how to regenerate
/// the plan with `cargo-allow why --plan`.
fn enrich_with_regen_hint(
    error: CargoAllowError,
    plan_path: &Path,
    plan: &LoadedPlan,
) -> CargoAllowError {
    let kind = &plan.finding.kind;
    let path = &plan.finding.path;
    let hint = match plan.finding.line {
        Some(line) => format!(
            "; regenerate with cargo-allow why --plan {} --kind {kind} --path {path} --line {line}",
            plan_path.display()
        ),
        None => format!(
            "; regenerate with cargo-allow why --plan {} --kind {kind} --path {path}",
            plan_path.display()
        ),
    };
    let message = error.to_string();
    if message.contains("(policy unchanged)") && !message.contains("regenerate with") {
        error.with_message_suffix(hint)
    } else {
        error
    }
}

pub(super) fn cmd_add_from_plan(args: &AddArgs, plan_path: &Path) -> CargoAllowResult<()> {
    reject_conflicting_from_plan_flags(args)?;

    let plan_bytes = read_bound_file(plan_path, "add-finding plan").map_err(plan_input_error)?;
    let plan_digest = sha256_v1_bytes(&plan_bytes);
    let plan = parse_plan_strict(&plan_bytes)?;
    validate_plan_generation(&plan)?;

    // Acquire the live-ledger mutation lock before the scan, so the recompute,
    // validate, and atomic replace are serialized against a concurrent writer.
    let cwd = current_dir()?;
    let mutation_root = resolve_source_tree_root(args.root.root.as_deref(), &cwd)?;
    let mutation_target = config_path(&mutation_root, args.config.as_deref());
    if let Some(target) = &mutation_target {
        crate::policy_config::assert_path_within_root(&mutation_root, target)?;
    }
    let _mutation_lock = mutation_target
        .as_ref()
        .map(|target| {
            let resolved = effortless_repo_edit::resolve_mutation_target(target, &mutation_root)?;
            MutationLock::acquire_for_target(&resolved)
        })
        .transpose()?;

    let kind_filter = parse_kind_filter(&plan.finding.kind)?;
    let (root, mut cfg, findings, inventory_facts, _federation) = load_world(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        Some(plan.finding.kind.as_str()),
        args.include_untracked,
    )?;

    let policy_path = config_path(&root, args.config.as_deref())
        .ok_or_else(crate::policy_config::missing_plan_update_policy_config_error)?;
    let policy_before = read_bound_file(&policy_path, "policy")?;
    let policy_before_digest = sha256_v1_bytes(&policy_before);

    // Re-select the finding by the plan's recorded coordinates against the live
    // scan. A finding that moved or vanished, or a now-ambiguous location, fails
    // here before any binding comparison.
    let finding_line = plan
        .finding
        .line
        .ok_or_else(|| stale("plan finding has no source line to locate"))
        .map_err(|error| enrich_with_regen_hint(error, plan_path, &plan))?;
    let finding_path = PathBuf::from(&plan.finding.path);
    let (finding_index, finding) =
        select_add_finding(&findings, kind_filter, &finding_path, finding_line as u32)
            .map_err(|error| stale(error.to_string()))
            .map_err(|error| enrich_with_regen_hint(error, plan_path, &plan))?;

    // The finding must still be uniquely `New`; a receipted, blocked, or
    // otherwise non-New posture (including replay after a prior successful
    // application) is rejected without mutating policy.
    let outcomes = evaluate(&cfg, &findings, CheckMode::Audit);
    let selected = outcomes
        .iter()
        .find(|outcome| outcome.finding_index == Some(finding_index))
        .ok_or_else(|| stale("selected finding produced no evaluation outcome"))
        .map_err(|error| enrich_with_regen_hint(error, plan_path, &plan))?;
    ensure_addable_outcome(selected.status)
        .map_err(|error| stale(error.to_string()))
        .map_err(|error| enrich_with_regen_hint(error, plan_path, &plan))?;

    // Recompute every binding from the live scan and require an exact match.
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let bindings = compute_plan_finding_bindings(
        &root,
        args.config.as_deref(),
        &cfg,
        args.include_untracked,
        finding,
    )?;
    verify_bindings(&plan, &bindings, source_context.source_tree_root())
        .map_err(|error| enrich_with_regen_hint(error, plan_path, &plan))?;

    // Construct the entry canonically from the live finding plus operator
    // judgment. Approval metadata is never read from the plan.
    require_add_evidence(finding, &args.evidence)?;
    let id = args.id.clone().unwrap_or_else(|| next_allow_id(&cfg));
    ensure_unique_allow_id(cfg.allow.iter().map(|entry| entry.id.as_str()), &id)?;
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
    let added_allow_id = entry.id.clone();
    cfg.allow.push(entry);

    // Validate the complete policy, then atomically replace the discovered
    // ledger. This is the single mutation point in the whole command.
    validate_policy(&cfg)?;
    let evidence_source_tree_files =
        current_evidence_source_tree_files(&root, args.include_untracked);
    validate_evidence_references_for_source_tree(&root, &cfg, evidence_source_tree_files.as_ref())?;
    let rendered = render_policy(&cfg);
    let policy_target = git_relative_config_path(&root, args.config.as_deref())?;
    apply_single_target(SingleTargetApplyRequest {
        repository_root: &root,
        target: &policy_target,
        contents: &rendered,
        caller_reference: Some("cargo-allow:add-from-plan"),
        lock_identity: Some(
            policy_target
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/"),
        ),
        mode: SingleTargetApplyMode::AtomicReplace,
    })
    .into_result()?;
    let policy_after_digest = sha256_v1_bytes(rendered.as_bytes());

    // Targeted recheck: re-evaluate the target finding against the mutated
    // policy (already in memory) to confirm the receipt actually landed. This
    // is NOT a full check — it reuses the loaded findings and the mutated cfg.
    // The operator still needs to run the full check argv for CI-grade proof.
    let recheck_outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);
    let targeted_recheck = match recheck_outcomes
        .iter()
        .find(|outcome| outcome.finding_index == Some(finding_index))
    {
        Some(outcome) => match outcome.status {
            allow_core::MatchStatus::Matched | allow_core::MatchStatus::LocationDrift => {
                "matched".to_string()
            }
            allow_core::MatchStatus::New => "still_new".to_string(),
            other => format!("unexpected:{}", other.as_str()),
        },
        None => "no_outcome".to_string(),
    };

    let receipt = AddPlanApplicationV1 {
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        inventory: source_context.inventory(),
        plan_digest,
        repository_identity: bindings.repository_identity,
        finding_digest: bindings.finding_digest,
        target_ledger: bindings.policy_path.clone(),
        policy_before_digest,
        policy_after_digest,
        added_allow_id,
        targeted_recheck: targeted_recheck.to_string(),
        full_check_argv: full_check_argv(
            source_context.source_tree_root(),
            &bindings.policy_path,
            args.include_untracked,
        ),
    };
    let receipt_json = render_add_plan_application_json(&receipt);
    emit_stderr_text(args.summary_output.as_deref(), &receipt_json)?;
    Ok(())
}

/// Direct-call safety net for the flag conflicts clap already enforces at parse
/// time, plus the `--update` requirement. Keeps `cmd_add_from_plan` fail-closed
/// for callers that construct `AddArgs` without the parser (e.g. tests).
fn reject_conflicting_from_plan_flags(args: &AddArgs) -> CargoAllowResult<()> {
    if !args.update {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "--from-plan applies to the live ledger and requires --update",
        ));
    }
    if args.write.is_some() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "--from-plan cannot be combined with --write",
        ));
    }
    if args.force {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "--from-plan cannot be combined with --force",
        ));
    }
    if args.kind.is_some() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "--kind cannot be combined with --from-plan; the kind comes from the plan",
        ));
    }
    if args.path.is_some()
        || args.line.is_some()
        || args.glob.is_some()
        || args.family.is_some()
        || args.callee.is_some()
    {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "manual target selectors (--path/--line/--glob/--family/--callee) cannot be combined with --from-plan",
        ));
    }
    Ok(())
}

fn parse_plan_strict(bytes: &[u8]) -> CargoAllowResult<LoadedPlan> {
    serde_json::from_slice(bytes).map_err(|error| {
        stale(format!(
            "plan is malformed or not a v1 add-finding plan: {error}"
        ))
    })
}

fn validate_plan_generation(plan: &LoadedPlan) -> CargoAllowResult<()> {
    if plan.schema_id != ADD_FINDING_PLAN_SCHEMA_ID {
        return Err(stale(format!(
            "unsupported plan schema `{}`; expected `{ADD_FINDING_PLAN_SCHEMA_ID}`",
            plan.schema_id
        )));
    }
    if plan.schema_version != ADD_FINDING_PLAN_SCHEMA_VERSION {
        return Err(stale(format!(
            "unsupported plan schema version `{}`; expected `{ADD_FINDING_PLAN_SCHEMA_VERSION}`",
            plan.schema_version
        )));
    }
    if plan.tool != "cargo-allow" {
        return Err(stale(format!(
            "unsupported plan tool `{}`; expected `cargo-allow`",
            plan.tool
        )));
    }
    if plan.command != "why" {
        return Err(stale(format!(
            "plan was not produced by `why` (command `{}`)",
            plan.command
        )));
    }
    if plan.outcome.status != "new" {
        return Err(stale(format!(
            "plan finding was not `new` at generation (status `{}`)",
            plan.outcome.status
        )));
    }
    Ok(())
}

/// Compare every recorded plan binding against the freshly recomputed one. The
/// checks run wrong-target first (repository/policy path), then content drift,
/// so the operator gets the most actionable rejection.
fn verify_bindings(
    plan: &LoadedPlan,
    bindings: &PlanFindingBindings,
    live_root: &str,
) -> CargoAllowResult<()> {
    if plan.repository.root != live_root {
        return Err(stale(format!(
            "plan targets repository root `{}` but the live root is `{live_root}`",
            plan.repository.root
        )));
    }
    if plan.policy.path != bindings.policy_path {
        return Err(stale(format!(
            "plan targets policy `{}` but the live policy path is `{}`",
            plan.policy.path, bindings.policy_path
        )));
    }
    if plan.policy.digest != bindings.policy_digest {
        return Err(stale("policy changed since the plan was generated"));
    }
    if plan.inventory_basis_identity != bindings.inventory_basis_identity {
        return Err(stale(
            "source inventory changed since the plan was generated",
        ));
    }
    if plan.repository.identity != bindings.repository_identity {
        return Err(stale(
            "repository identity changed since the plan was generated",
        ));
    }
    if plan.finding.kind != bindings.finding_kind || plan.finding.family != bindings.finding_family
    {
        return Err(stale(
            "finding kind or family changed since the plan was generated",
        ));
    }
    if plan.finding.path != bindings.finding_path {
        return Err(stale("finding path changed since the plan was generated"));
    }
    if plan.finding.line != bindings.finding_line || plan.finding.column != bindings.finding_column
    {
        return Err(stale(
            "finding location changed since the plan was generated",
        ));
    }
    if plan.finding.digest != bindings.finding_digest {
        return Err(stale(
            "finding identity changed since the plan was generated",
        ));
    }
    if plan.finding.source_file_digest != bindings.source_file_digest {
        return Err(stale("source file changed since the plan was generated"));
    }
    if plan.finding.identity != bindings.finding_identity {
        return Err(stale(
            "finding identity fields changed since the plan was generated",
        ));
    }
    if plan.finding.selector != bindings.selector {
        return Err(stale("selector changed since the plan was generated"));
    }
    Ok(())
}

fn full_check_argv(root: &str, policy_path: &str, include_untracked: bool) -> Vec<String> {
    let mut argv = vec![
        "check".to_string(),
        "--mode".to_string(),
        "no-new".to_string(),
        "--root".to_string(),
        root.to_string(),
        "--config".to_string(),
        policy_path.to_string(),
    ];
    if include_untracked {
        argv.push("--include-untracked".to_string());
    }
    argv
}

#[cfg(test)]
#[path = "add_from_plan_tests.rs"]
mod tests;
