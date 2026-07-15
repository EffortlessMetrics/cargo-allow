use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, MatchStatus,
    normalize_path,
};
use allow_match::{CheckMode, evaluate, explain_match_failure, score_match};

use crate::{
    EvidenceValidationMode, emit_text, load_world_with_evidence_mode, parse_kind_filter,
};

#[path = "why_args.rs"]
mod why_args;
#[path = "why_render.rs"]
mod why_render;

pub(crate) use why_args::WhyArgs;
use why_render::{WhyCandidate, render_why_text};

const MAX_CANDIDATES: usize = 8;

pub(crate) fn cmd_why(args: &WhyArgs) -> CargoAllowResult<()> {
    let parsed_kind = parse_kind_filter(&args.kind)?;
    let (_root, cfg, findings, _inventory_facts, _federation) = load_world_with_evidence_mode(
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

    let text = render_why_text(finding, &outcome, &candidates);
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
#[path = "why_tests.rs"]
mod why_tests;
