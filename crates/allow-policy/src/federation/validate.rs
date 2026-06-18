use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use super::config::{
    FederationConfig, FederationDiagnostic, FederationDiagnosticKind, LedgerEntry, LedgerRole,
    ValidatedFederationConfig, is_native_dialect,
};

pub fn validate_federation_config(config: FederationConfig) -> ValidatedFederationConfig {
    let mut diagnostics = Vec::new();
    diagnostics.extend(detect_duplicate_ids(&config.ledgers));
    diagnostics.extend(detect_duplicate_paths(&config.ledgers));
    diagnostics.extend(detect_mirror_targets(&config.ledgers));
    diagnostics.extend(detect_duplicate_canonical_lanes(&config.ledgers));
    diagnostics.extend(detect_dialect_issues(&config.ledgers));

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
    let mut counts = HashMap::<&str, usize>::new();
    for ledger in ledgers {
        *counts.entry(ledger.id.as_str()).or_default() += 1;
    }
    counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(id, _)| FederationDiagnostic {
            kind: FederationDiagnosticKind::DuplicateId,
            message: format!("duplicate federation ledger id `{id}`"),
            ledger_ids: vec![id.to_string()],
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
