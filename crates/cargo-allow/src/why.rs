use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, Finding,
    MatchStatus, normalize_path,
};
use allow_match::{CheckMode, evaluate, explain_match_failure, score_match};
use allow_rust::RustFileScanOutcome;

use crate::{
    HumanJsonFormat, SourceTreeReportContext, current_dir, emit_text, load_world_for_path,
    load_world_from_resolved_policy_with_options, parse_kind_filter, resolve_source_tree_root,
};

#[path = "why_args.rs"]
mod why_args;
#[path = "why_plan.rs"]
mod why_plan;
#[path = "why_render.rs"]
mod why_render;
#[path = "why_shell.rs"]
mod why_shell;

pub(crate) use why_args::WhyArgs;
#[cfg(test)]
use why_render::render_why_text;
use why_render::{
    WhyCandidate, render_why_json_with_evaluation_and_scanner_completeness,
    render_why_target_scan_json, render_why_target_scan_text,
    render_why_text_styled_with_evaluation_and_scanner_completeness,
};
#[cfg(test)]
use why_render::{render_why_json, render_why_text_styled};

const MAX_CANDIDATES: usize = 8;

fn missing_evaluation_outcome_error(path: &std::path::Path, line: u32) -> CargoAllowError {
    CargoAllowError::with_kind(
        CargoAllowErrorKind::Internal,
        format!(
            "no evaluation outcome for finding at {}:{}",
            normalize_path(path),
            line
        ),
    )
}

pub(crate) fn cmd_why(args: &WhyArgs) -> CargoAllowResult<()> {
    if let (Some(plan_path), Some(output_path)) = (args.plan.as_deref(), args.output.as_deref())
        && same_output_target(plan_path, output_path)?
    {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "--plan and --output must name different files",
        ));
    }
    if let Some(plan_path) = args.plan.as_deref()
        && plan_path.exists()
    {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            format!(
                "add-finding plan output {} already exists; choose a new --plan path",
                plan_path.display()
            ),
        ));
    }
    let parsed_kind = parse_kind_filter(&args.kind)?;
    let cwd = current_dir()?;
    let source_root = resolve_source_tree_root(args.root.root.as_deref(), cwd.clone())?;
    let target_path = if args.path.is_absolute() {
        args.path.clone()
    } else if args.root.root.is_some() {
        source_root.join(&args.path)
    } else {
        cwd.join(&args.path)
    };
    let scoped_world = load_world_for_path(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        Some(args.kind.as_str()),
        args.include_untracked,
        &target_path,
    )?;
    let target_repo_path = crate::world::normalize_to_repo_relative(&scoped_world.0, &target_path);
    if let Some(target_scan) = scoped_world.5.as_ref()
        && !matches!(target_scan, RustFileScanOutcome::Scanned)
    {
        let (status, reason) = match target_scan {
            RustFileScanOutcome::ParseError => ("parse_error", None),
            RustFileScanOutcome::Skipped { reason } => ("skipped", Some(reason.as_str())),
            RustFileScanOutcome::Scanned => ("scanned", None),
        };
        let evaluation = allow_report::EvaluationContext {
            scope: "scoped",
            locality: "proven",
            reasons: &[],
        };
        let source_context = SourceTreeReportContext::new(&scoped_world.0, scoped_world.3);
        let target_path_text = normalize_path(&target_repo_path);
        let detail_json = render_why_target_scan_json(
            source_context.inventory(),
            evaluation,
            &target_path_text,
            status,
            reason,
        );
        let summary = why_target_summary(
            &detail_json,
            &scoped_world.0,
            &source_context,
            &target_path_text,
            args.line,
            scoped_world.3,
        )?;
        crate::core_command_router::write_summary_artifact(&scoped_world.0, &summary)?;
        let text = match args.format {
            HumanJsonFormat::Human => render_why_target_scan_text(
                evaluation,
                source_context.inventory(),
                &target_path_text,
                status,
                reason,
            ),
            HumanJsonFormat::Json => detail_json,
        };
        emit_text(args.output.as_deref(), &text)?;
        return Ok(());
    }
    let scoped_finding =
        crate::add::select_add_finding(&scoped_world.2, parsed_kind, &target_repo_path, args.line)?
            .1;
    let selected_policy_digest = scoped_world.3.policy_digest_text();
    let selected_policy_path = if args.plan.is_some() {
        scoped_world.6.clone()
    } else {
        None
    };
    let locality_reasons =
        crate::world::scoped_locality_reasons(&scoped_world.1, scoped_finding, &scoped_world.4);
    let evaluation = if locality_reasons.is_empty() {
        allow_report::EvaluationContext {
            scope: "scoped",
            locality: "proven",
            reasons: &locality_reasons,
        }
    } else {
        allow_report::EvaluationContext {
            scope: "full_fallback",
            locality: "global_dependency",
            reasons: &locality_reasons,
        }
    };
    let (root, cfg, findings, inventory_facts, _federation) = if locality_reasons.is_empty() {
        (
            scoped_world.0,
            scoped_world.1,
            scoped_world.2,
            scoped_world.3,
            scoped_world.4,
        )
    } else {
        load_world_from_resolved_policy_with_options(
            &scoped_world.0,
            scoped_world.1.clone(),
            selected_policy_digest.clone(),
            scoped_world.4.clone(),
            args.include_untracked,
            Some(args.kind.as_str()),
            true,
        )?
    };
    let (finding_index, finding) =
        crate::add::select_add_finding(&findings, parsed_kind, &target_repo_path, args.line)?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);
    let outcome = outcomes
        .into_iter()
        .find(|outcome| outcome.finding_index == Some(finding_index))
        .ok_or_else(|| missing_evaluation_outcome_error(&args.path, args.line))?;

    let candidates = if outcome.status == MatchStatus::New {
        related_mismatch_candidates(&cfg, finding)
    } else {
        Vec::new()
    };

    let scanner_completeness = if inventory_facts.rust_files_skipped > 0
        || inventory_facts.rust_files_with_parse_errors > 0
    {
        Some("partial")
    } else {
        Some("complete")
    };
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let mut written_plan_path = None;
    if let Some(plan_path) = args.plan.as_deref() {
        let plan = why_plan::render_add_finding_plan(why_plan::AddFindingPlanInput {
            root: &root,
            config: args.config.as_deref(),
            cfg: &cfg,
            expected_policy_digest: selected_policy_digest.as_deref(),
            expected_policy_path: selected_policy_path.as_deref(),
            include_untracked: args.include_untracked,
            source_context: &source_context,
            evaluation,
            scanner_completeness,
            finding,
            outcome: &outcome,
            candidates: &candidates,
        })?;
        crate::write_file_no_overwrite(plan_path, &plan, false)
            .map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)?;
        written_plan_path = Some(plan_write_path(&root, plan_path));
    }
    let style = if matches!(args.format, HumanJsonFormat::Human) && args.output.is_none() {
        crate::reporting::output_style()
    } else {
        allow_report::Style::PLAIN
    };
    // The JSON artifact is rendered for every format: it supplies the
    // relocation-stable semantic identity the summary reports, and it is the
    // command's own artifact when JSON was requested.
    let detail_json = render_why_json_with_evaluation_and_scanner_completeness(
        source_context.inventory(),
        evaluation,
        finding,
        &outcome,
        &candidates,
        scanner_completeness,
    );
    // Common operator grammar (#3149). The detailed why artifact remains
    // authoritative; this projection is additive and derived from the same
    // in-memory evaluation without re-evaluating the finding.
    let summary = why_summary(
        &detail_json,
        &root,
        &source_context,
        WhyFindingFacts {
            finding,
            outcome: &outcome,
            candidates: &candidates,
            queried_line: args.line,
            plan_path: written_plan_path,
            inventory_facts,
        },
    )?;
    crate::core_command_router::write_summary_artifact(&root, &summary)?;

    let text = match args.format {
        HumanJsonFormat::Human => {
            let mut rendered =
                crate::core_command_summary::render_core_command_summary_human(&summary);
            rendered.push('\n');
            rendered.push_str(
                &render_why_text_styled_with_evaluation_and_scanner_completeness(
                    source_context.inventory(),
                    finding,
                    &outcome,
                    &candidates,
                    style,
                    evaluation,
                    scanner_completeness,
                ),
            );
            rendered
        }
        HumanJsonFormat::Json => detail_json,
    };
    emit_text(args.output.as_deref(), &text)?;
    Ok(())
}

/// Finding-scoped facts `why` has already computed for the common summary.
struct WhyFindingFacts<'a> {
    finding: &'a Finding,
    outcome: &'a allow_core::MatchOutcome,
    candidates: &'a [WhyCandidate<'a>],
    queried_line: u32,
    /// Source-tree-relative path of the add-finding plan this run wrote.
    plan_path: Option<String>,
    inventory_facts: crate::InventoryFacts,
}

/// Build the common operator summary from the evaluation `why` already holds.
///
/// The relocation-stable semantic identity comes from why's own JSON artifact,
/// exactly as `audit`, `check`, and `doctor` derive theirs, so the summary never
/// rescans source or re-evaluates the finding to describe itself.
fn why_summary(
    detail_json: &str,
    root: &std::path::Path,
    source_context: &SourceTreeReportContext,
    facts: WhyFindingFacts<'_>,
) -> CargoAllowResult<crate::core_command_summary::CoreCommandSummaryV1> {
    let semantic_identity =
        crate::core_command_router::canonical_semantic_identity(detail_json, Some(root))?;
    let completeness = crate::core_command_router::summary_completeness(&facts.inventory_facts);
    let coverage_limitation = (completeness != effortless_repo_protocol::CompletenessV1::Complete)
        .then(|| crate::core_command_router::partial_coverage_reason(&facts.inventory_facts));
    let inventory_source = source_context.inventory_source();
    let location = format!(
        "{}:{}",
        normalize_path(&facts.finding.path),
        facts
            .finding
            .span
            .as_ref()
            .map(|span| span.line)
            .unwrap_or(facts.queried_line)
    );

    // The subject is the exact queried location, named in the shared
    // `<subject>:<inventory mode>:current-unpinned` grammar.
    let mut subject = crate::core_command_summary::CoreSourceSubjectV1 {
        kind: crate::core_command_summary::CoreSourceSubjectKindV1::ScopedPath,
        repository_identity: format!("local-repository:{semantic_identity}"),
        portable_identity: format!(
            "scoped:finding:{}:{location}:{inventory_source}:current-unpinned",
            facts.finding.kind.as_str()
        ),
        base: None,
        head: None,
        paths: vec![location.clone()],
        limitations: Vec::new(),
    };
    subject.limitations.push(
        "the current worktree result is not bound to a commit, tree, or Git-index identity"
            .to_string(),
    );

    let next = why_render::why_next_steps(facts.finding, facts.outcome, facts.candidates);
    crate::core_command_summary::core_command_summary_from_why(
        crate::core_command_summary::WhySummaryFactsV1 {
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            subject,
            completeness,
            coverage_limitation,
            location,
            outcome_status: facts.outcome.status,
            matched_allow_id: facts.outcome.allow_id.clone(),
            near_miss_candidate_count: facts.candidates.len(),
            suggested_actions: next.suggested_actions,
            plan_path: facts.plan_path,
            claim_boundary: effortless_repo_protocol::ClaimBoundaryV1::new(
                "cargo-allow explained one source-tree finding against current source-exception ledger posture only",
            )
            .with_limitations(vec![
                "cargo metadata, rustc, Clippy, build scripts, proc macros, tests, and repository code were not invoked"
                    .to_string(),
                "macro expansion, type information, MIR, control flow, and data flow were not analyzed"
                    .to_string(),
                "one receipted finding does not prove the repository passes the no-new gate".to_string(),
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

fn why_target_summary(
    detail_json: &str,
    root: &std::path::Path,
    source_context: &SourceTreeReportContext,
    target_path: &str,
    queried_line: u32,
    inventory_facts: crate::InventoryFacts,
) -> CargoAllowResult<crate::core_command_summary::CoreCommandSummaryV1> {
    let semantic_identity =
        crate::core_command_router::canonical_semantic_identity(detail_json, Some(root))?;
    let completeness = crate::core_command_router::summary_completeness(&inventory_facts);
    let coverage_limitation = (completeness != effortless_repo_protocol::CompletenessV1::Complete)
        .then(|| crate::core_command_router::partial_coverage_reason(&inventory_facts));
    let location = format!("{target_path}:{queried_line}");
    let mut subject = crate::core_command_summary::CoreSourceSubjectV1 {
        kind: crate::core_command_summary::CoreSourceSubjectKindV1::ScopedPath,
        repository_identity: format!("local-repository:{semantic_identity}"),
        portable_identity: format!(
            "scoped:target-scan:{location}:{}:current-unpinned",
            source_context.inventory_source()
        ),
        base: None,
        head: None,
        paths: vec![target_path.to_string()],
        limitations: vec![
            "the current worktree result is not bound to a commit, tree, or Git-index identity"
                .to_string(),
        ],
    };
    if let Some(limitation) = coverage_limitation.as_ref() {
        subject.limitations.push(limitation.clone());
    }
    let suggested_actions = vec![
        format!("Repair or reduce the target so the Rust scanner can inspect `{target_path}`."),
        "Re-run cargo-allow why after the target scan is complete.".to_string(),
    ];
    crate::core_command_summary::core_command_summary_from_why(
        crate::core_command_summary::WhySummaryFactsV1 {
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            subject,
            completeness,
            coverage_limitation: None,
            location,
            outcome_status: MatchStatus::New,
            matched_allow_id: None,
            near_miss_candidate_count: 0,
            suggested_actions,
            plan_path: None,
            claim_boundary: effortless_repo_protocol::ClaimBoundaryV1::new(
                "cargo-allow could not fully scan the selected source target, so no finding or add-finding plan was produced",
            )
            .with_limitations(vec![
                "the result is non-green and does not establish whether the target contains no findings"
                    .to_string(),
                "macro expansion, type information, MIR, control flow, and data flow were not analyzed"
                    .to_string(),
            ]),
        },
    )
    .map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            format!("failed to build target scan summary: {error}"),
        )
    })
}

/// Name the written plan the way an operator can act on it: source-tree
/// relative when it lives under the root, otherwise its normalized path.
fn plan_write_path(root: &std::path::Path, plan_path: &std::path::Path) -> String {
    crate::portable_relative_under_root(root, plan_path)
        .map(|relative| normalize_path(&relative))
        .unwrap_or_else(|_| allow_report::source_tree_path_text(plan_path))
}

fn output_path_resolution_error(
    path: &std::path::Path,
    source: &std::io::Error,
) -> CargoAllowError {
    CargoAllowError::with_kind(
        CargoAllowErrorKind::Artifact,
        format!("failed to resolve output path {}: {source}", path.display()),
    )
    .with_cause(source)
}

fn resolve_output_path_result(
    path: &std::path::Path,
    result: Result<std::path::PathBuf, std::io::Error>,
) -> CargoAllowResult<std::path::PathBuf> {
    result.map_err(|source| output_path_resolution_error(path, &source))
}

fn resolve_output_path(path: &std::path::Path) -> CargoAllowResult<std::path::PathBuf> {
    resolve_output_path_result(path, std::path::absolute(path))
}

fn same_output_target(left: &std::path::Path, right: &std::path::Path) -> CargoAllowResult<bool> {
    let left = resolve_output_path(left)?;
    let right = resolve_output_path(right)?;
    Ok(left == right)
}

fn related_mismatch_candidates<'a>(
    cfg: &'a AllowConfig,
    finding: &Finding,
) -> Vec<WhyCandidate<'a>> {
    let mut related = cfg
        .allow
        .iter()
        .filter(|entry| entry.kind == finding.kind)
        .filter(|entry| score_match(entry, finding).is_none())
        .filter(|entry| entry_is_related(entry, finding))
        .map(|entry| WhyCandidate {
            reasons: explain_match_failure(entry, finding),
            entry,
        })
        .collect::<Vec<_>>();

    if related.is_empty() {
        related = cfg
            .allow
            .iter()
            .filter(|entry| entry.kind == finding.kind)
            .filter(|entry| score_match(entry, finding).is_none())
            .map(|entry| WhyCandidate {
                reasons: explain_match_failure(entry, finding),
                entry,
            })
            .collect();
    }

    related.sort_by(|left, right| {
        left.reasons
            .len()
            .cmp(&right.reasons.len())
            .then_with(|| left.entry.id.cmp(&right.entry.id))
    });
    if related.len() > MAX_CANDIDATES {
        related.truncate(MAX_CANDIDATES);
    }
    related
}

fn entry_is_related(entry: &AllowEntry, finding: &Finding) -> bool {
    if entry.family.is_some() && entry.family == finding.family {
        return true;
    }
    if let Some(path) = &entry.path
        && normalize_path(path) == normalize_path(&finding.path)
    {
        return true;
    }
    if let Some(glob) = &entry.glob
        && allow_core::glob_matches(glob, &finding.path)
    {
        return true;
    }
    if let Some(glob) = &entry.selector.glob
        && allow_core::glob_matches(glob, &finding.path)
    {
        return true;
    }
    false
}

#[cfg(test)]
pub(crate) fn sample_why_json_for_contract_test() -> String {
    use allow_core::{
        FindingKind, Lifecycle, MatchOutcome, MatchStatus, Selector, Span, StructuralIdentity,
    };
    use std::path::PathBuf;

    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.container = Some("load".to_string());
    identity.callee = Some("unwrap".to_string());
    let finding = Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from("src/lib.rs"),
        span: Some(Span {
            line: 10,
            column: 1,
        }),
        identity,
        message: "unwrap call".to_string(),
        ledger: None,
    };
    let entry = AllowEntry {
        id: "allow-near-miss".to_string(),
        kind: FindingKind::Panic,
        family: None,
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "near miss fixture".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            container: Some("load".to_string()),
            callee: Some("expect".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    };
    let outcome = MatchOutcome {
        status: MatchStatus::New,
        allow_id: None,
        candidate_ids: Vec::new(),
        finding_index: Some(0),
        message: "unreceipted panic.unwrap at src/lib.rs:10:1".to_string(),
        score: 0,
    };
    let reasons = explain_match_failure(&entry, &finding);
    render_why_json(
        allow_report::InventoryContext::source_syntax(
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(48),
        ),
        &finding,
        &outcome,
        &[WhyCandidate {
            entry: &entry,
            reasons,
        }],
    )
}

#[cfg(test)]
pub(crate) fn sample_add_finding_plan_json_for_contract_test() -> String {
    why_plan::sample_add_finding_plan_json_for_contract_test()
}

#[cfg(test)]
#[path = "why_tests.rs"]
mod why_tests;
