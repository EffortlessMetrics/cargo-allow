use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, MatchStatus,
    normalize_path,
};
use allow_match::{CheckMode, evaluate, explain_match_failure, score_match};

use crate::{
    EvidenceValidationMode, SourceTreeReportContext, emit_text, load_world_with_evidence_mode,
    parse_kind_filter,
};

#[path = "why_args.rs"]
mod why_args;
#[path = "why_render.rs"]
mod why_render;

pub(crate) use why_args::WhyArgs;
use why_args::WhyFormat;
use why_render::{WhyCandidate, render_why_json, render_why_text};

const MAX_CANDIDATES: usize = 8;

pub(crate) fn cmd_why(args: &WhyArgs) -> CargoAllowResult<()> {
    let parsed_kind = parse_kind_filter(&args.kind)?;
    let (root, cfg, findings, inventory_facts, _federation) = load_world_with_evidence_mode(
        args.root.root.as_deref(),
        args.config.as_deref(),
        true,
        None,
        args.include_untracked,
        EvidenceValidationMode::ReportOnly,
    )?;
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
    let text = match args.format {
        WhyFormat::Human => render_why_text(finding, &outcome, &candidates),
        WhyFormat::Json => {
            render_why_json(source_context.inventory(), finding, &outcome, &candidates)
        }
    };
    emit_text(args.output.as_deref(), &text)?;
    Ok(())
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
    if let Some(path) = &entry.path {
        if normalize_path(path) == normalize_path(&finding.path) {
            return true;
        }
    }
    if let Some(glob) = &entry.glob {
        if allow_core::glob_matches(glob, &finding.path) {
            return true;
        }
    }
    if let Some(glob) = &entry.selector.glob {
        if allow_core::glob_matches(glob, &finding.path) {
            return true;
        }
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
        family: Some("unwrap".to_string()),
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
#[path = "why_tests.rs"]
mod why_tests;
