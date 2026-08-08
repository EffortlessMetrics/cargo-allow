use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use allow_core::SimpleDate;

use super::config::{
    DrainWindow, FederationConfig, FederationDiagnostic, FederationDiagnosticKind, LedgerEntry,
    LedgerRole, ValidatedFederationConfig, is_native_dialect,
};

pub fn validate_federation_config(config: FederationConfig) -> ValidatedFederationConfig {
    let mut diagnostics = Vec::new();
    diagnostics.extend(detect_duplicate_ids(&config.ledgers));
    diagnostics.extend(detect_duplicate_paths(&config.ledgers));
    diagnostics.extend(detect_mirror_targets(&config.ledgers));
    diagnostics.extend(detect_duplicate_canonical_lanes(&config.ledgers));
    diagnostics.extend(detect_priority_ties(&config.ledgers));
    diagnostics.extend(detect_dialect_issues(&config.ledgers));
    diagnostics.extend(detect_drain_window_issues(
        &config.ledgers,
        &config.drain_windows,
    ));

    let valid = !diagnostics
        .iter()
        .any(|diagnostic| diagnostic.is_blocking());
    ValidatedFederationConfig {
        config,
        diagnostics,
        valid,
    }
}

impl FederationDiagnostic {
    fn is_blocking(&self) -> bool {
        !matches!(self.kind, FederationDiagnosticKind::DialectSkipped)
    }
}

fn detect_duplicate_ids(ledgers: &[LedgerEntry]) -> Vec<FederationDiagnostic> {
    let mut occurrences = BTreeMap::<&str, Vec<(usize, &LedgerEntry)>>::new();
    for (index, ledger) in ledgers.iter().enumerate() {
        occurrences
            .entry(ledger.id.as_str())
            .or_default()
            .push((index, ledger));
    }

    occurrences
        .into_iter()
        .filter(|(_, entries)| entries.len() > 1)
        .map(|(id, entries)| {
            let positions = entries
                .iter()
                .map(|(index, ledger)| format!("ledgers[{index}] path `{}`", ledger.path))
                .collect::<Vec<_>>();
            FederationDiagnostic {
                kind: FederationDiagnosticKind::DuplicateId,
                message: format!(
                    "duplicate federation ledger id `{id}` at {}",
                    positions.join(", ")
                ),
                ledger_ids: entries
                    .iter()
                    .map(|(_, ledger)| ledger.id.clone())
                    .collect(),
            }
        })
        .collect()
}

fn detect_duplicate_paths(ledgers: &[LedgerEntry]) -> Vec<FederationDiagnostic> {
    let mut seen = HashMap::<&str, Vec<&LedgerEntry>>::new();
    for ledger in ledgers {
        seen.entry(ledger.path.as_str()).or_default().push(ledger);
    }
    seen.into_values()
        .filter(|entries| entries.len() > 1)
        .map(|entries| {
            let ids = entries
                .iter()
                .map(|entry| entry.id.clone())
                .collect::<Vec<_>>();
            FederationDiagnostic {
                kind: FederationDiagnosticKind::DuplicatePath,
                message: format!(
                    "duplicate federation ledger path `{}` for ids: {}",
                    entries[0].path,
                    ids.join(", ")
                ),
                ledger_ids: ids,
            }
        })
        .collect()
}

fn detect_mirror_targets(ledgers: &[LedgerEntry]) -> Vec<FederationDiagnostic> {
    let ids = ledgers
        .iter()
        .map(|ledger| ledger.id.as_str())
        .collect::<HashSet<_>>();
    let mut diagnostics = Vec::new();
    for ledger in ledgers {
        if ledger.role != LedgerRole::Mirror {
            continue;
        }
        let Some(target) = ledger.mirrors.as_deref() else {
            diagnostics.push(FederationDiagnostic {
                kind: FederationDiagnosticKind::MirrorMissingTarget,
                message: format!(
                    "mirror ledger `{}` must declare mirrors = \"<canonical ledger id>\"",
                    ledger.id
                ),
                ledger_ids: vec![ledger.id.clone()],
            });
            continue;
        };
        if !ids.contains(target) {
            diagnostics.push(FederationDiagnostic {
                kind: FederationDiagnosticKind::UnknownMirrorTarget,
                message: format!(
                    "mirror ledger `{}` references unknown target `{target}`",
                    ledger.id
                ),
                ledger_ids: vec![ledger.id.clone()],
            });
        }
    }
    diagnostics
}

fn detect_duplicate_canonical_lanes(ledgers: &[LedgerEntry]) -> Vec<FederationDiagnostic> {
    let mut lane_owners = BTreeMap::<String, Vec<&LedgerEntry>>::new();
    for ledger in ledgers {
        if ledger.role != LedgerRole::Canonical {
            continue;
        }
        for lane in &ledger.lanes {
            lane_owners.entry(lane.clone()).or_default().push(ledger);
        }
    }

    lane_owners
        .into_iter()
        .filter(|(_, owners)| owners.len() > 1)
        .map(|(lane, owners)| {
            let ids = owners
                .iter()
                .map(|entry| entry.id.to_string())
                .collect::<Vec<_>>();
            FederationDiagnostic {
                kind: FederationDiagnosticKind::DuplicateCanonicalLane,
                message: format!(
                    "multiple canonical ledgers claim lane `{lane}`: {}",
                    ids.join(", ")
                ),
                ledger_ids: ids,
            }
        })
        .collect()
}

/// Detect canonical ledgers that share the same lane AND the same priority
/// value — the precedence is undocumented declaration order, which is fragile
/// (#2010). Each such tie produces a diagnostic so operators can assign
/// distinct priorities.
fn detect_priority_ties(ledgers: &[LedgerEntry]) -> Vec<FederationDiagnostic> {
    let mut lane_priority_owners: BTreeMap<(String, u32), Vec<&LedgerEntry>> = BTreeMap::new();
    for ledger in ledgers {
        if ledger.role != LedgerRole::Canonical {
            continue;
        }
        for lane in &ledger.lanes {
            lane_priority_owners
                .entry((lane.clone(), ledger.priority))
                .or_default()
                .push(ledger);
        }
    }

    lane_priority_owners
        .into_iter()
        .filter(|(_, owners)| owners.len() > 1)
        .map(|((lane, priority), owners)| {
            let ids = owners
                .iter()
                .map(|entry| entry.id.to_string())
                .collect::<Vec<_>>();
            FederationDiagnostic {
                kind: FederationDiagnosticKind::PriorityTie,
                message: format!(
                    "canonical ledgers {ids:?} share lane `{lane}` at priority {priority}; \
                     precedence is undocumented declaration order — assign distinct priorities"
                ),
                ledger_ids: ids,
            }
        })
        .collect()
}

fn detect_dialect_issues(ledgers: &[LedgerEntry]) -> Vec<FederationDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut dialects_by_path = BTreeMap::<&str, BTreeSet<&str>>::new();
    for ledger in ledgers {
        dialects_by_path
            .entry(ledger.path.as_str())
            .or_default()
            .insert(ledger.dialect.as_str());
    }

    for ledger in ledgers {
        if is_native_dialect(&ledger.dialect) {
            continue;
        }
        match ledger.role {
            LedgerRole::Imported => diagnostics.push(FederationDiagnostic {
                kind: FederationDiagnosticKind::DialectSkipped,
                message: format!(
                    "foreign dialect `{}` on imported ledger `{}` ({}) will be skipped per federation dialect rules",
                    ledger.dialect, ledger.id, ledger.path
                ),
                ledger_ids: vec![ledger.id.clone()],
            }),
            LedgerRole::Canonical | LedgerRole::Mirror => diagnostics.push(FederationDiagnostic {
                kind: FederationDiagnosticKind::DialectConflict,
                message: format!(
                    "ledger `{}` ({}) uses foreign dialect `{}` for role `{}`",
                    ledger.id,
                    ledger.path,
                    ledger.dialect,
                    ledger.role.as_str()
                ),
                ledger_ids: vec![ledger.id.clone()],
            }),
        }
    }

    for (path, dialects) in dialects_by_path {
        if dialects.len() <= 1 {
            continue;
        }
        let ids = ledgers
            .iter()
            .filter(|ledger| ledger.path == path)
            .map(|ledger| ledger.id.clone())
            .collect::<Vec<_>>();
        diagnostics.push(FederationDiagnostic {
            kind: FederationDiagnosticKind::DialectConflict,
            message: format!(
                "conflicting dialects for path `{path}`: {}",
                dialects.into_iter().collect::<Vec<_>>().join(", ")
            ),
            ledger_ids: ids,
        });
    }

    diagnostics
}

fn detect_drain_window_issues(
    ledgers: &[LedgerEntry],
    drain_windows: &[DrainWindow],
) -> Vec<FederationDiagnostic> {
    let ledger_by_id = ledgers
        .iter()
        .map(|ledger| (ledger.id.as_str(), ledger))
        .collect::<HashMap<_, _>>();
    let mut diagnostics = Vec::new();
    for (window_index, drain) in drain_windows.iter().enumerate() {
        // `expiry` is required: a drain window without one never reports
        // DrainExpired (a None expiry never "passes"), so the mirror ledger
        // would live forever. (#2006)
        if drain.drain_owner.trim().is_empty()
            || drain.drain_reason.trim().is_empty()
            || drain.review_after.trim().is_empty()
            || drain.linked_closeout.trim().is_empty()
            || drain
                .expiry
                .as_ref()
                .map(|expiry| expiry.trim().is_empty())
                .unwrap_or(true)
        {
            diagnostics.push(FederationDiagnostic {
                kind: FederationDiagnosticKind::DrainWindowMissingField,
                message: format!(
                    "drain window for mirror `{}` requires drain_owner, drain_reason, review_after, expiry, and linked_closeout",
                    drain.mirror_ledger
                ),
                ledger_ids: vec![drain.mirror_ledger.clone()],
            });
        }
        // Validate lifecycle dates are syntactically well-formed. The deadline
        // predicates (`has_passed_date_str`/`is_due_date_str`) intentionally
        // treat a malformed/missing date as "not yet due", so a typo'd format
        // would silently disable the drain deadline. Only check non-empty
        // values; a missing/empty field is already reported by the
        // missing-field diagnostic above. (#2007)
        if let Some(expiry) = drain.expiry.as_deref()
            && !expiry.trim().is_empty()
            && SimpleDate::parse(expiry).is_none()
        {
            diagnostics.push(FederationDiagnostic {
                    kind: FederationDiagnosticKind::DrainWindowInvalidDate,
                    message: format!(
                        "drain window {window_index} has invalid expiry `{expiry}`; expected YYYY-MM-DD",
                    ),
                    ledger_ids: vec![drain.mirror_ledger.clone()],
                });
        }
        if !drain.review_after.trim().is_empty() && SimpleDate::parse(&drain.review_after).is_none()
        {
            diagnostics.push(FederationDiagnostic {
                kind: FederationDiagnosticKind::DrainWindowInvalidDate,
                message: format!(
                    "drain window {window_index} has invalid review_after `{}`; expected YYYY-MM-DD",
                    drain.review_after
                ),
                ledger_ids: vec![drain.mirror_ledger.clone()],
            });
        }
        let Some(mirror) = ledger_by_id.get(drain.mirror_ledger.as_str()).copied() else {
            diagnostics.push(FederationDiagnostic {
                kind: FederationDiagnosticKind::UnknownDrainMirrorLedger,
                message: format!(
                    "drain window references unknown mirror ledger `{}`",
                    drain.mirror_ledger
                ),
                ledger_ids: vec![drain.mirror_ledger.clone()],
            });
            continue;
        };
        if mirror.role != LedgerRole::Mirror {
            diagnostics.push(FederationDiagnostic {
                kind: FederationDiagnosticKind::DrainWindowNotMirror,
                message: format!(
                    "drain window mirror ledger `{}` must have role mirror",
                    drain.mirror_ledger
                ),
                ledger_ids: vec![drain.mirror_ledger.clone()],
            });
        }
    }
    diagnostics
}
