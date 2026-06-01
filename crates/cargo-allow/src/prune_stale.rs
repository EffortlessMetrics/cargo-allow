use super::PruneCandidate;
use allow_core::{AllowConfig, MatchOutcome, MatchStatus};
use std::collections::BTreeSet;

pub(super) fn prune_stale_candidates(
    cfg: &AllowConfig,
    outcomes: &[MatchOutcome],
) -> Vec<PruneCandidate> {
    outcomes
        .iter()
        .filter(|outcome| outcome.status == MatchStatus::Stale)
        .filter_map(|outcome| {
            let id = outcome.allow_id.as_deref()?;
            let entry = cfg.allow.iter().find(|entry| entry.id == id)?;
            Some(PruneCandidate {
                id: entry.id.clone(),
                kind: entry.kind,
                family: entry.family.clone(),
                owner: entry.owner.clone(),
                classification: entry.classification.clone(),
                scope: entry.path_or_glob(),
                reason: entry.reason.clone(),
            })
        })
        .collect()
}

pub(super) fn config_without_prune_candidates(
    cfg: &AllowConfig,
    candidates: &[PruneCandidate],
) -> AllowConfig {
    let mut pruned = cfg.clone();
    pruned
        .allow
        .retain(|entry| !candidates.iter().any(|candidate| candidate.id == entry.id));
    pruned
}

pub(super) fn removed_toml_blocks(
    rendered_policy: &str,
    candidates: &[PruneCandidate],
) -> Vec<String> {
    let ids = candidates
        .iter()
        .map(|candidate| candidate.id.as_str())
        .collect::<BTreeSet<_>>();
    allow_blocks(rendered_policy)
        .into_iter()
        .filter(|block| ids.iter().any(|id| block_contains_allow_id(block, id)))
        .map(str::to_string)
        .collect()
}

fn allow_blocks(rendered_policy: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut start = None;
    let mut offset = 0;
    for line in rendered_policy.split_inclusive('\n') {
        let line_text = line.trim_end_matches('\n').trim_end_matches('\r');
        if line_text == "[[allow]]" {
            if let Some(previous) = start.replace(offset) {
                if let Some(block) = rendered_policy.get(previous..offset) {
                    blocks.push(block.trim_end());
                }
            }
        }
        offset += line.len();
    }
    if let Some(previous) = start {
        if let Some(block) = rendered_policy.get(previous..) {
            blocks.push(block.trim_end());
        }
    }
    blocks
}

fn block_contains_allow_id(block: &str, id: &str) -> bool {
    block
        .lines()
        .any(|line| line.trim() == format!("id = \"{}\"", escape_toml_basic(id)))
}

fn escape_toml_basic(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}
