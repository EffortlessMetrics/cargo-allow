use allow_core::{AllowConfig, Finding, MatchOutcome, MatchStatus, SimpleDate};
use std::collections::{BTreeMap, BTreeSet};

use crate::classification::classify_matched;
use crate::lifecycle::unused_entry_status;
use crate::messages::{family_suffix, finding_location};
use crate::mode::CheckMode;
use crate::scoring::classify_match;

/// Per-entry occurrence accounting produced by [`evaluate_detailed`].
///
/// `observed_count` includes findings that matched the entry after its limit
/// was exhausted. That makes `exceeded_count` a direct signal from the match
/// layer rather than a value a caller has to reconstruct from messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OccurrenceAccounting {
    pub allow_id: String,
    pub observed_count: u32,
    pub occurrence_limit: u32,
    pub headroom: u32,
    pub exceeded_count: u32,
}

/// Detailed result for consumers that need occurrence-limit state in addition
/// to the existing match outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchEvaluation {
    pub outcomes: Vec<MatchOutcome>,
    pub occurrence_accounting: Vec<OccurrenceAccounting>,
}

pub fn evaluate(cfg: &AllowConfig, findings: &[Finding], mode: CheckMode) -> Vec<MatchOutcome> {
    evaluate_detailed(cfg, findings, mode).outcomes
}

pub fn evaluate_detailed(
    cfg: &AllowConfig,
    findings: &[Finding],
    mode: CheckMode,
) -> MatchEvaluation {
    let mut outcomes = Vec::new();
    let mut used_entries = BTreeSet::new();
    let mut non_live_matched_entries = BTreeSet::new();
    let mut entry_occurrences = BTreeMap::<usize, u32>::new();
    let mut observed_occurrences = BTreeMap::<usize, u32>::new();
    let mut drift_outcomes = BTreeMap::<usize, Vec<usize>>::new();
    let mut anchored_entries = BTreeSet::new();
    let today = SimpleDate::today_utc_approx();

    for (finding_index, finding) in findings.iter().enumerate() {
        let mut candidates = Vec::new();
        for (entry_index, entry) in cfg.allow.iter().enumerate() {
            if let Some(strength) = classify_match(entry, finding) {
                candidates.push((entry_index, strength.as_priority()));
            }
        }
        match candidates.as_slice() {
            [] => outcomes.push(MatchOutcome {
                status: MatchStatus::New,
                allow_id: None,
                candidate_ids: Vec::new(),
                finding_index: Some(finding_index),
                message: format!(
                    "unreceipted {}{} at {}",
                    finding.kind,
                    family_suffix(finding),
                    finding_location(finding)
                ),
                score: 0,
            }),
            [(entry_index, score)] => {
                let Some(entry) = cfg.allow.get(*entry_index) else {
                    continue;
                };
                let observed_count = observed_occurrences.entry(*entry_index).or_default();
                *observed_count = observed_count.saturating_add(1);
                let current_count = entry_occurrences.get(entry_index).copied().unwrap_or(0);
                if entry
                    .occurrence_limit
                    .is_some_and(|limit| current_count >= limit)
                {
                    used_entries.insert(*entry_index);
                    outcomes.push(MatchOutcome {
                        status: MatchStatus::New,
                        allow_id: Some(entry.id.clone()),
                        candidate_ids: vec![entry.id.clone()],
                        finding_index: Some(finding_index),
                        message: format!(
                            "{} occurrence_limit exceeded at {}",
                            entry.id,
                            finding_location(finding)
                        ),
                        score: *score,
                    });
                    continue;
                }
                let (status, message) = classify_matched(entry, finding, *score, today, cfg, mode);
                if status_consumes_entry(status) {
                    used_entries.insert(*entry_index);
                    entry_occurrences.insert(*entry_index, current_count + 1);
                } else {
                    non_live_matched_entries.insert(*entry_index);
                }
                if status == MatchStatus::LocationDrift {
                    drift_outcomes
                        .entry(*entry_index)
                        .or_default()
                        .push(outcomes.len());
                } else if entry
                    .last_seen
                    .as_ref()
                    .zip(finding.span.as_ref())
                    .is_some_and(|(last_seen, span)| {
                        last_seen.line == span.line && last_seen.column == span.column
                    })
                {
                    anchored_entries.insert(*entry_index);
                }
                outcomes.push(MatchOutcome {
                    status,
                    allow_id: Some(entry.id.clone()),
                    candidate_ids: vec![entry.id.clone()],
                    finding_index: Some(finding_index),
                    message,
                    score: *score,
                });
            }
            many => {
                // Find the unique top-scoring candidate. If one entry strictly
                // outscores all others, take it as the match (deterministic
                // tiebreak: highest score wins). Only return Ambiguous when
                // two or more candidates share the max score (#1802).
                let max_score = many.iter().map(|(_, score)| *score).fold(0, u32::max);
                let top_candidates: Vec<_> = many
                    .iter()
                    .filter(|(_, score)| *score == max_score)
                    .collect();

                if top_candidates.len() == 1 {
                    // Unique winner — treat like a single-candidate match.
                    let Some((entry_index, score)) =
                        top_candidates.first().map(|candidate| **candidate)
                    else {
                        continue;
                    };
                    let Some(entry) = cfg.allow.get(entry_index) else {
                        continue;
                    };
                    let observed_count = observed_occurrences.entry(entry_index).or_default();
                    *observed_count = observed_count.saturating_add(1);
                    let current_count = entry_occurrences.get(&entry_index).copied().unwrap_or(0);
                    if entry
                        .occurrence_limit
                        .is_some_and(|limit| current_count >= limit)
                    {
                        used_entries.insert(entry_index);
                        outcomes.push(MatchOutcome {
                            status: MatchStatus::New,
                            allow_id: Some(entry.id.clone()),
                            candidate_ids: many
                                .iter()
                                .filter_map(|(idx, _)| {
                                    cfg.allow.get(*idx).map(|candidate| candidate.id.clone())
                                })
                                .collect(),
                            finding_index: Some(finding_index),
                            message: format!(
                                "{} occurrence_limit exceeded at {}",
                                entry.id,
                                finding_location(finding)
                            ),
                            score,
                        });
                        continue;
                    }
                    used_entries.insert(entry_index);
                    entry_occurrences.insert(entry_index, current_count + 1);
                    let (status, message) =
                        classify_matched(entry, finding, score, today, cfg, mode);
                    outcomes.push(MatchOutcome {
                        status,
                        allow_id: Some(entry.id.clone()),
                        candidate_ids: many
                            .iter()
                            .filter_map(|(idx, _)| {
                                cfg.allow.get(*idx).map(|candidate| candidate.id.clone())
                            })
                            .collect(),
                        finding_index: Some(finding_index),
                        message,
                        score,
                    });
                } else {
                    // Genuine tie — report Ambiguous with candidate IDs.
                    // Mark ALL tied candidates as used so they are NOT
                    // demoted to Stale in the unused-entry scan (#2042).
                    let ids = top_candidates
                        .iter()
                        .filter_map(|(idx, _)| cfg.allow.get(*idx).map(|entry| entry.id.clone()))
                        .collect::<Vec<_>>()
                        .join(", ");
                    for (entry_index, _) in &top_candidates {
                        used_entries.insert(*entry_index);
                    }
                    outcomes.push(MatchOutcome {
                        status: MatchStatus::Ambiguous,
                        allow_id: None,
                        candidate_ids: top_candidates
                            .iter()
                            .filter_map(|(idx, _)| {
                                cfg.allow.get(*idx).map(|entry| entry.id.clone())
                            })
                            .collect(),
                        finding_index: Some(finding_index),
                        message: format!(
                            "finding at {} matched multiple allow entries with equal score: {ids}",
                            finding_location(finding)
                        ),
                        score: max_score,
                    });
                }
            }
        }
    }

    // An entry that matches several occurrences (glob or occurrence_limit)
    // records only one last_seen anchor. When any matched occurrence still
    // sits at the anchor, the other occurrences are ordinary matches, not
    // drift — otherwise every scan flags a drift that refresh can never
    // settle, oscillating between the occurrences forever.
    for (entry_index, outcome_indices) in &drift_outcomes {
        if !anchored_entries.contains(entry_index) {
            continue;
        }
        let Some(entry) = cfg.allow.get(*entry_index) else {
            continue;
        };
        for outcome_index in outcome_indices {
            let Some(outcome) = outcomes.get_mut(*outcome_index) else {
                continue;
            };
            outcome.status = MatchStatus::Matched;
            outcome.message = format!(
                "{} matched with structural score {}; last_seen anchored by another occurrence",
                entry.id, outcome.score
            );
        }
    }

    for (entry_index, entry) in cfg.allow.iter().enumerate() {
        if used_entries.contains(&entry_index) {
            continue;
        }
        let status = if non_live_matched_entries.contains(&entry_index) {
            MatchStatus::Stale
        } else {
            unused_entry_status(entry, today)
        };
        let message = match status {
            MatchStatus::Expired => format!(
                "{} is expired on {}",
                entry.id,
                entry.lifecycle.expires.as_deref().unwrap_or("<missing>")
            ),
            MatchStatus::ReviewDue => format!(
                "{} is due for review after {}",
                entry.id,
                entry
                    .lifecycle
                    .review_after
                    .as_deref()
                    .unwrap_or("<missing>")
            ),
            MatchStatus::Stale => format!(
                "{} is stale: no current finding matched {}",
                entry.id,
                entry.path_or_glob()
            ),
            other => format!("{} has unexpected unused-entry status {other:?}", entry.id),
        };
        outcomes.push(MatchOutcome {
            status,
            allow_id: Some(entry.id.clone()),
            candidate_ids: Vec::new(),
            finding_index: None,
            message,
            score: 0,
        });
    }

    let occurrence_accounting = cfg
        .allow
        .iter()
        .enumerate()
        .filter_map(|(entry_index, entry)| {
            let limit = entry.occurrence_limit?;
            let observed_count = observed_occurrences.get(&entry_index).copied().unwrap_or(0);
            Some(OccurrenceAccounting {
                allow_id: entry.id.clone(),
                observed_count,
                occurrence_limit: limit,
                headroom: limit.saturating_sub(observed_count),
                exceeded_count: observed_count.saturating_sub(limit),
            })
        })
        .collect();

    MatchEvaluation {
        outcomes,
        occurrence_accounting,
    }
}

fn status_consumes_entry(status: MatchStatus) -> bool {
    matches!(
        status,
        MatchStatus::Matched
            | MatchStatus::LocationDrift
            | MatchStatus::ReviewDue
            | MatchStatus::BaselineDebt
    )
}
