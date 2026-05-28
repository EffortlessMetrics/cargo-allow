use allow_core::{AllowConfig, Finding, MatchOutcome, MatchStatus, SimpleDate};
use std::collections::{BTreeMap, BTreeSet};

mod classification;
mod messages;
mod mode;
mod scoring;
use classification::classify_matched;
use messages::family_suffix;
pub use messages::finding_location;
pub use mode::CheckMode;
pub use scoring::{STRUCTURAL_MATCH_THRESHOLD, score_match};

pub fn evaluate(cfg: &AllowConfig, findings: &[Finding], mode: CheckMode) -> Vec<MatchOutcome> {
    let mut outcomes = Vec::new();
    let mut used_entries = BTreeSet::new();
    let mut entry_occurrences = BTreeMap::<usize, u32>::new();
    let today = SimpleDate::today_utc_approx();

    for (finding_index, finding) in findings.iter().enumerate() {
        let mut candidates = Vec::new();
        for (entry_index, entry) in cfg.allow.iter().enumerate() {
            if let Some(score) = score_match(entry, finding) {
                if score >= STRUCTURAL_MATCH_THRESHOLD {
                    candidates.push((entry_index, score));
                }
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
                let entry = &cfg.allow[*entry_index];
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
                    .map(|(idx, _)| cfg.allow[*idx].id.clone())
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

    for (entry_index, entry) in cfg.allow.iter().enumerate() {
        if used_entries.contains(&entry_index) {
            continue;
        }
        outcomes.push(MatchOutcome {
            status: MatchStatus::Stale,
            allow_id: Some(entry.id.clone()),
            finding_index: None,
            message: format!(
                "{} is stale: no current finding matched {}",
                entry.id,
                entry.path_or_glob()
            ),
            score: 0,
        });
    }
    outcomes
}

#[cfg(test)]
mod tests;
