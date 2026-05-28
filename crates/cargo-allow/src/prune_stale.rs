use super::PruneCandidate;
use allow_core::{AllowConfig, MatchOutcome, MatchStatus};

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
