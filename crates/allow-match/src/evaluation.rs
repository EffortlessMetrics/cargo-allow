use allow_core::{AllowConfig, Finding, MatchOutcome, MatchStatus, SimpleDate};
use std::collections::{BTreeMap, BTreeSet};

use crate::classification::classify_matched;
use crate::lifecycle::unused_entry_status;
use crate::messages::{family_suffix, finding_location};
use crate::mode::CheckMode;
use crate::scoring::classify_match;

pub fn evaluate(cfg: &AllowConfig, findings: &[Finding], mode: CheckMode) -> Vec<MatchOutcome> {
    let mut outcomes = Vec::new();
    let mut used_entries = BTreeSet::new();
    let mut entry_occurrences = BTreeMap::<usize, u32>::new();
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
                let current_count = entry_occurrences.get(entry_index).copied().unwrap_or(0);
                if entry
                    .occurrence_limit
                    .is_some_and(|limit| current_count >= limit)
                {
                    used_entries.insert(*entry_index);
                    outcomes.push(MatchOutcome {
                        status: MatchStatus::New,
                        allow_id: Some(entry.id.clone()),
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
                used_entries.insert(*entry_index);
                entry_occurrences.insert(*entry_index, current_count + 1);
                let (status, message) = classify_matched(entry, finding, *score, today, cfg, mode);
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
                    finding_index: Some(finding_index),
                    message,
                    score: *score,
                });
            }
            many => {
                let ids = many
                    .iter()
                    .filter_map(|(idx, _)| cfg.allow.get(*idx).map(|entry| entry.id.clone()))
                    .collect::<Vec<_>>()
                    .join(", ");
                outcomes.push(MatchOutcome {
                    status: MatchStatus::Ambiguous,
                    allow_id: None,
                    finding_index: Some(finding_index),
                    message: format!(
                        "finding at {} matched multiple allow entries: {ids}",
                        finding_location(finding)
                    ),
                    score: many.iter().map(|(_, score)| *score).max().unwrap_or(0),
                });
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
        let status = unused_entry_status(entry, today);
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
            finding_index: None,
            message,
            score: 0,
        });
    }
    outcomes
}
