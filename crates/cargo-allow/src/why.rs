use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, MatchStatus,
    normalize_path,
};
use allow_match::{CheckMode, evaluate, explain_match_failure, score_match};

use crate::{
    HumanJsonFormat, SourceTreeReportContext, emit_text, load_world, load_world_for_path,
    parse_kind_filter,
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
    WhyCandidate, render_why_json_with_evaluation, render_why_text_styled_with_evaluation,
};
#[cfg(test)]
use why_render::{render_why_json, render_why_text_styled};

const MAX_CANDIDATES: usize = 8;

pub(crate) fn cmd_why(args: &WhyArgs) -> CargoAllowResult<()> {
    if let (Some(plan_path), Some(output_path)) = (args.plan.as_deref(), args.output.as_deref())
        && same_output_target(plan_path, output_path)?
    {
        return Err(CargoAllowError::new(
            "--plan and --output must name different files",
        ));
    }
    if let Some(plan_path) = args.plan.as_deref()
        && plan_path.exists()
    {
        return Err(CargoAllowError::new(format!(
            "add-finding plan output {} already exists; choose a new --plan path",
            plan_path.display()
        )));
    }
    let parsed_kind = parse_kind_filter(&args.kind)?;
    let scoped_world = load_world_for_path(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        Some(args.kind.as_str()),
        args.include_untracked,
        &args.path,
    )?;
    let scoped_finding =
        crate::add::select_add_finding(&scoped_world.2, parsed_kind, &args.path, args.line)?.1;
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
        scoped_world
    } else {
        load_world(
            args.root.root.as_deref(),
            args.config.as_deref(),
            true,
            Some(args.kind.as_str()),
            args.include_untracked,
        )?
    };
    let (finding_index, finding) =
        crate::add::select_add_finding(&findings, parsed_kind, &args.path, args.line)?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::NoNew);
    let outcome = outcomes
        .into_iter()
        .find(|outcome| outcome.finding_index == Some(finding_index))
        .ok_or_else(|| {
            CargoAllowError::new(format!(
                "no evaluation outcome for finding at {}:{}",
                normalize_path(&args.path),
                args.line
            ))
        })?;

    let candidates = if outcome.status == MatchStatus::New {
        related_mismatch_candidates(&cfg, finding)
    } else {
        Vec::new()
    };

    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    if let Some(plan_path) = args.plan.as_deref() {
        let plan = why_plan::render_add_finding_plan(why_plan::AddFindingPlanInput {
            root: &root,
            config: args.config.as_deref(),
            cfg: &cfg,
            include_untracked: args.include_untracked,
            source_context: &source_context,
            evaluation,
            finding,
            outcome: &outcome,
            candidates: &candidates,
        })?;
        crate::write_file_no_overwrite(plan_path, &plan, false)?;
    }
    let style = if matches!(args.format, HumanJsonFormat::Human) && args.output.is_none() {
        crate::reporting::output_style()
    } else {
        allow_report::Style::PLAIN
    };
    let text = match args.format {
        HumanJsonFormat::Human => render_why_text_styled_with_evaluation(
            finding,
            &outcome,
            &candidates,
            style,
            evaluation,
        ),
        HumanJsonFormat::Json => render_why_json_with_evaluation(
            source_context.inventory(),
            evaluation,
            finding,
            &outcome,
            &candidates,
        ),
    };
    emit_text(args.output.as_deref(), &text)?;
    Ok(())
}

fn same_output_target(left: &std::path::Path, right: &std::path::Path) -> CargoAllowResult<bool> {
    let left = std::path::absolute(left).map_err(|error| {
        CargoAllowError::new(format!(
            "failed to resolve output path {}: {error}",
            left.display()
        ))
    })?;
    let right = std::path::absolute(right).map_err(|error| {
        CargoAllowError::new(format!(
            "failed to resolve output path {}: {error}",
            right.display()
        ))
    })?;
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
