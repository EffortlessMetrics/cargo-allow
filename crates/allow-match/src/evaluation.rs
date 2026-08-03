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

/// Mutable accounting shared across every finding while [`evaluate_detailed`]
/// walks the findings × entries surface.
///
/// Holding it in one place lets the exactly-one-candidate path and the
/// unique-strongest-candidate path funnel through a single selected-entry
/// evaluation ([`EvalState::evaluate_selected_candidate`]) instead of the two
/// divergent copies that let a lifecycle fix land on only one path (#2336).
#[derive(Default)]
struct EvalState {
    outcomes: Vec<MatchOutcome>,
    used_entries: BTreeSet<usize>,
    non_live_matched_entries: BTreeSet<usize>,
    non_live_match_messages: BTreeMap<usize, String>,
    entry_occurrences: BTreeMap<usize, u32>,
    observed_occurrences: BTreeMap<usize, u32>,
    drift_outcomes: BTreeMap<usize, Vec<usize>>,
    anchored_entries: BTreeSet<usize>,
}

/// The per-finding context that a selected-candidate evaluation reads. Grouped
/// so both call sites share one shape and the evaluation helper stays under the
/// argument-count lint.
struct FindingContext<'a> {
    finding: &'a Finding,
    finding_index: usize,
    today: SimpleDate,
    mode: CheckMode,
}

impl EvalState {
    /// Evaluate one already-selected candidate entry against `finding`.
    ///
    /// This is the single shared path for both selected-candidate shapes:
    /// exactly one candidate, and the unique strongest candidate among many
    /// (#2336). Candidate discovery and tie resolution stay with the caller,
    /// which supplies `candidate_ids` for context; everything after a winner is
    /// chosen — observation, occurrence-limit behavior, lifecycle
    /// classification, live/used vs non-live accounting, and drift/anchor
    /// bookkeeping — happens here so a weaker neighboring candidate can never
    /// change whether the selected entry is live, stale, expired, invalid, or
    /// headroom-consuming.
    fn evaluate_selected_candidate(
        &mut self,
        cfg: &AllowConfig,
        ctx: &FindingContext<'_>,
        entry_index: usize,
        score: u32,
        candidate_ids: Vec<String>,
    ) -> Option<MatchStatus> {
        let FindingContext {
            finding,
            finding_index,
            today,
            mode,
        } = *ctx;
        let entry = cfg.allow.get(entry_index)?;

        // Record the structural observation. `observed_count` counts every
        // occurrence, including ones past the limit, and stays distinct from
        // the live/consumed count updated below.
        let observed_count = self.observed_occurrences.entry(entry_index).or_default();
        *observed_count = observed_count.saturating_add(1);

        let current_count = self
            .entry_occurrences
            .get(&entry_index)
            .copied()
            .unwrap_or(0);
        if entry
            .occurrence_limit
            .is_some_and(|limit| current_count >= limit)
        {
            self.used_entries.insert(entry_index);
            self.outcomes.push(MatchOutcome {
                status: MatchStatus::New,
                allow_id: Some(entry.id.clone()),
                candidate_ids,
                finding_index: Some(finding_index),
                message: format!(
                    "{} occurrence_limit exceeded at {}",
                    entry.id,
                    finding_location(finding)
                ),
                score,
            });
            return Some(MatchStatus::New);
        }

        let (status, message) = classify_matched(entry, finding, score, today, cfg, mode);
        // Only live statuses mark the entry used and consume occurrence
        // headroom. Non-live statuses (Expired, EvidenceMissing,
        // InvalidSelector) are recorded so the later unused-entry projection
        // still emits the stale/non-live posture — regardless of whether a
        // weaker neighboring candidate was also present.
        if status_consumes_entry(status) {
            self.used_entries.insert(entry_index);
            self.entry_occurrences
                .insert(entry_index, current_count.saturating_add(1));
        } else {
            self.non_live_matched_entries.insert(entry_index);
            self.non_live_match_messages
                .entry(entry_index)
                .or_insert_with(|| message.clone());
        }

        if status == MatchStatus::LocationDrift {
            let outcome_index = self.outcomes.len();
            self.drift_outcomes
                .entry(entry_index)
                .or_default()
                .push(outcome_index);
        } else if entry
            .last_seen
            .as_ref()
            .zip(finding.span.as_ref())
            .is_some_and(|(last_seen, span)| {
                last_seen.line == span.line && last_seen.column == span.column
            })
        {
            self.anchored_entries.insert(entry_index);
        }

        self.outcomes.push(MatchOutcome {
            status,
            allow_id: Some(entry.id.clone()),
            candidate_ids,
            finding_index: Some(finding_index),
            message,
            score,
        });
        Some(status)
    }
}

pub fn evaluate(cfg: &AllowConfig, findings: &[Finding], mode: CheckMode) -> Vec<MatchOutcome> {
    evaluate_detailed(cfg, findings, mode).outcomes
}

pub fn evaluate_detailed(
    cfg: &AllowConfig,
    findings: &[Finding],
    mode: CheckMode,
) -> MatchEvaluation {
    let mut state = EvalState::default();
    let today = SimpleDate::today_utc_approx();

    for (finding_index, finding) in findings.iter().enumerate() {
        let ctx = FindingContext {
            finding,
            finding_index,
            today,
            mode,
        };
        let mut candidates = Vec::new();
        for (entry_index, entry) in cfg.allow.iter().enumerate() {
            if let Some(strength) = classify_match(entry, finding) {
                candidates.push((entry_index, strength.as_priority()));
            }
        }
        match candidates.as_slice() {
            [] => state.outcomes.push(MatchOutcome {
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
                let entry_index = *entry_index;
                let Some(entry) = cfg.allow.get(entry_index) else {
                    continue;
                };
                state.evaluate_selected_candidate(
                    cfg,
                    &ctx,
                    entry_index,
                    *score,
                    vec![entry.id.clone()],
                );
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
                    // Unique winner — evaluate it through the same
                    // selected-candidate path as the single-candidate case so
                    // lifecycle and occurrence accounting stay identical
                    // (#2336). The full candidate set is preserved as context.
                    let Some((entry_index, score)) =
                        top_candidates.first().map(|candidate| **candidate)
                    else {
                        continue;
                    };
                    let candidate_ids: Vec<String> = many
                        .iter()
                        .filter_map(|(idx, _)| cfg.allow.get(*idx).map(|entry| entry.id.clone()))
                        .collect();
                    let fallback =
                        fallback_candidate(cfg, finding, many, entry_index, score, today, mode);
                    let winner_status = state.evaluate_selected_candidate(
                        cfg,
                        &ctx,
                        entry_index,
                        score,
                        candidate_ids.clone(),
                    );
                    if winner_status.is_some_and(fallback_allowed_status)
                        && let Some((fallback_index, fallback_score)) = fallback
                    {
                        // MatchOutcome has one finding-level row, so project
                        // coverage through the weaker live candidate while
                        // retaining the stronger non-live entry in
                        // `non_live_matched_entries` for its independent stale
                        // maintenance projection.
                        let _ = state.outcomes.pop();
                        state.evaluate_selected_candidate(
                            cfg,
                            &ctx,
                            fallback_index,
                            fallback_score,
                            candidate_ids,
                        );
                    }
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
                        state.used_entries.insert(*entry_index);
                    }
                    state.outcomes.push(MatchOutcome {
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
    // settle, oscillating between the occurrences forever. This is an honest
    // entry-level limitation, not proof that every occurrence stayed put;
    // exact multi-occurrence anchors remain tracked by #2508.
    for (entry_index, outcome_indices) in &state.drift_outcomes {
        if !state.anchored_entries.contains(entry_index) {
            continue;
        }
        let Some(entry) = cfg.allow.get(*entry_index) else {
            continue;
        };
        for outcome_index in outcome_indices {
            let Some(outcome) = state.outcomes.get_mut(*outcome_index) else {
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
        if state.used_entries.contains(&entry_index) {
            continue;
        }
        let status = if state.non_live_matched_entries.contains(&entry_index) {
            MatchStatus::Stale
        } else {
            unused_entry_status(entry, today)
        };
        let message = match status {
            MatchStatus::Stale if state.non_live_matched_entries.contains(&entry_index) => {
                let detail = state
                    .non_live_match_messages
                    .get(&entry_index)
                    .map(String::as_str)
                    .unwrap_or("the matched finding is not currently authorizing");
                format!(
                    "{} is stale: a matched finding is not currently authorizing ({detail})",
                    entry.id
                )
            }
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
        state.outcomes.push(MatchOutcome {
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
            let observed_count = state
                .observed_occurrences
                .get(&entry_index)
                .copied()
                .unwrap_or(0);
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
        outcomes: state.outcomes,
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

/// Find a strictly weaker live candidate when the unique strongest candidate
/// cannot currently authorize coverage because its lifecycle or evidence is
/// unhealthy. The stronger entry is still evaluated first so its maintenance
/// posture remains visible through the unused-entry projection.
///
/// This is intentionally narrower than a general candidate-health model: only
/// expired and evidence-missing winners may use this compatibility fallback.
/// Invalid selectors and other policy failures remain fail-closed.
fn fallback_candidate(
    cfg: &AllowConfig,
    finding: &Finding,
    candidates: &[(usize, u32)],
    winner_index: usize,
    winner_score: u32,
    today: SimpleDate,
    mode: CheckMode,
) -> Option<(usize, u32)> {
    let winner = cfg.allow.get(winner_index)?;
    let (winner_status, _) = classify_matched(winner, finding, winner_score, today, cfg, mode);
    if !fallback_allowed_status(winner_status) {
        return None;
    }

    let mut live = candidates
        .iter()
        .filter(|(entry_index, score)| *entry_index != winner_index && *score < winner_score)
        .filter_map(|(entry_index, score)| {
            let entry = cfg.allow.get(*entry_index)?;
            let (status, _) = classify_matched(entry, finding, *score, today, cfg, mode);
            if matches!(
                status,
                MatchStatus::Matched | MatchStatus::LocationDrift | MatchStatus::ReviewDue
            ) {
                Some((*entry_index, *score))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let max_score = live.iter().map(|(_, score)| *score).max()?;
    live.retain(|(_, score)| *score == max_score);
    if live.len() == 1 {
        live.into_iter().next()
    } else {
        None
    }
}

fn fallback_allowed_status(status: MatchStatus) -> bool {
    matches!(status, MatchStatus::Expired | MatchStatus::EvidenceMissing)
}
