use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use allow_core::{AllowConfig, AllowEntry, CargoAllowResult, SimpleDate, stable_hash_hex};

use super::config::{DrainWindow, FederationConfig, LedgerEntry, LedgerRole};
use crate::load_policy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FederationDivergenceKind {
    MirrorStale,
    MirrorDivergence,
    DrainExpired,
}

impl FederationDivergenceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MirrorStale => "mirror_stale",
            Self::MirrorDivergence => "mirror_divergence",
            Self::DrainExpired => "drain_expired",
        }
    }

    pub fn is_blocking(self) -> bool {
        matches!(self, Self::DrainExpired)
    }

    pub fn counts_toward_mirror_divergence_deny(self) -> bool {
        matches!(self, Self::MirrorStale | Self::MirrorDivergence)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FederationDivergenceRecord {
    pub kind: FederationDivergenceKind,
    pub message: String,
    pub canonical_ledger_id: String,
    pub mirror_ledger_id: String,
    pub canonical_path: String,
    pub mirror_path: String,
    pub sample_entry_ids: Vec<String>,
    pub canonical_fingerprint: Option<String>,
    pub mirror_fingerprint: Option<String>,
    pub recommended_action: &'static str,
}

pub fn detect_mirror_divergences(
    root: &Path,
    config: &FederationConfig,
) -> CargoAllowResult<Vec<FederationDivergenceRecord>> {
    let today = SimpleDate::today_utc_approx();
    let ledger_by_id = config
        .ledgers
        .iter()
        .map(|ledger| (ledger.id.as_str(), ledger))
        .collect::<BTreeMap<_, _>>();
    let mut records = Vec::new();
    for drain in &config.drain_windows {
        let Some(mirror) = ledger_by_id.get(drain.mirror_ledger.as_str()).copied() else {
            continue;
        };
        let Some(canonical_id) = mirror.mirrors.as_deref() else {
            continue;
        };
        let Some(canonical) = ledger_by_id.get(canonical_id).copied() else {
            continue;
        };
        if mirror.role != LedgerRole::Mirror || canonical.role != LedgerRole::Canonical {
            continue;
        }
        if SimpleDate::has_passed_date_str(drain.expiry.as_deref(), today) {
            records.push(FederationDivergenceRecord {
                kind: FederationDivergenceKind::DrainExpired,
                message: format!(
                    "drain window for mirror `{}` expired on {}; linked_closeout={}",
                    mirror.id,
                    drain.expiry.as_deref().unwrap_or("unknown"),
                    drain.linked_closeout
                ),
                canonical_ledger_id: canonical.id.clone(),
                mirror_ledger_id: mirror.id.clone(),
                canonical_path: canonical.path.clone(),
                mirror_path: mirror.path.clone(),
                sample_entry_ids: Vec::new(),
                canonical_fingerprint: None,
                mirror_fingerprint: None,
                recommended_action: "extend drain window via closeout or retire mirror ledger",
            });
            continue;
        }
        records.extend(compare_mirror_canonical_ledgers(
            root, canonical, mirror, drain,
        )?);
    }
    Ok(records)
}

fn compare_mirror_canonical_ledgers(
    root: &Path,
    canonical: &LedgerEntry,
    mirror: &LedgerEntry,
    drain: &DrainWindow,
) -> CargoAllowResult<Vec<FederationDivergenceRecord>> {
    let canonical_path = root.join(&canonical.path);
    let mirror_path = root.join(&mirror.path);
    let canonical_cfg = load_contained_policy(root, &canonical_path);
    let mirror_cfg = load_contained_policy(root, &mirror_path);
    let canonical_available = canonical_cfg.is_some();
    let mirror_available = mirror_cfg.is_some();
    let (Some(canonical_cfg), Some(mirror_cfg)) = (canonical_cfg, mirror_cfg) else {
        let missing = match (canonical_available, mirror_available) {
            (false, false) => "canonical and mirror policy files are unavailable or invalid",
            (false, true) => "canonical policy file is unavailable or invalid",
            (true, false) => "mirror policy file is unavailable or invalid",
            (true, true) => "policy files could not be compared",
        };
        return Ok(vec![FederationDivergenceRecord {
            kind: FederationDivergenceKind::MirrorDivergence,
            message: format!(
                "mirror `{}` ({}) and canonical `{}` ({}) diverge during drain window: {missing}",
                mirror.id, mirror.path, canonical.id, canonical.path
            ),
            canonical_ledger_id: canonical.id.clone(),
            mirror_ledger_id: mirror.id.clone(),
            canonical_path: canonical.path.clone(),
            mirror_path: mirror.path.clone(),
            sample_entry_ids: Vec::new(),
            canonical_fingerprint: None,
            mirror_fingerprint: None,
            recommended_action: "sync mirror from canonical or document intentional drain posture",
        }]);
    };

    let canonical_fingerprint = ledger_sync_fingerprint(&canonical_cfg);
    let mirror_fingerprint = ledger_sync_fingerprint(&mirror_cfg);
    if canonical_fingerprint == mirror_fingerprint {
        return Ok(Vec::new());
    }

    let canonical_ids = entry_id_set(&canonical_cfg);
    let mirror_ids = entry_id_set(&mirror_cfg);
    let canonical_only = canonical_ids
        .difference(&mirror_ids)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mirror_only = mirror_ids
        .difference(&canonical_ids)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut mismatched_shared = BTreeSet::new();
    for id in canonical_ids.intersection(&mirror_ids) {
        let canonical_entry = canonical_cfg.allow.iter().find(|entry| entry.id == *id);
        let mirror_entry = mirror_cfg.allow.iter().find(|entry| entry.id == *id);
        if let (Some(canonical_entry), Some(mirror_entry)) = (canonical_entry, mirror_entry)
            && entry_sync_fingerprint(canonical_entry) != entry_sync_fingerprint(mirror_entry)
        {
            mismatched_shared.insert(id.clone());
        }
    }

    let mut sample_entry_ids = canonical_only
        .iter()
        .chain(mirror_only.iter())
        .chain(mismatched_shared.iter())
        .take(5)
        .cloned()
        .collect::<Vec<_>>();

    if canonical_only.is_empty() && mirror_only.is_empty() && mismatched_shared.is_empty() {
        return Ok(vec![FederationDivergenceRecord {
            kind: FederationDivergenceKind::MirrorStale,
            message: format!(
                "mirror `{}` fingerprint `{}` differs from canonical `{}` fingerprint `{}` during drain window (owner={}, closeout={})",
                mirror.id,
                mirror_fingerprint,
                canonical.id,
                canonical_fingerprint,
                drain.drain_owner,
                drain.linked_closeout
            ),
            canonical_ledger_id: canonical.id.clone(),
            mirror_ledger_id: mirror.id.clone(),
            canonical_path: canonical.path.clone(),
            mirror_path: mirror.path.clone(),
            sample_entry_ids,
            canonical_fingerprint: Some(canonical_fingerprint),
            mirror_fingerprint: Some(mirror_fingerprint),
            recommended_action: "refresh mirror snapshot from canonical ledger",
        }]);
    }

    let mut message = format!(
        "mirror `{}` and canonical `{}` entry sets diverge during drain window",
        mirror.id, canonical.id
    );
    if !canonical_only.is_empty() {
        message.push_str(&format!(
            "; canonical-only ids: {}",
            join_sample_ids(&canonical_only, 3)
        ));
    }
    if !mirror_only.is_empty() {
        message.push_str(&format!(
            "; mirror-only ids: {}",
            join_sample_ids(&mirror_only, 3)
        ));
    }
    if !mismatched_shared.is_empty() {
        message.push_str(&format!(
            "; mismatched shared ids: {}",
            join_sample_ids(&mismatched_shared, 3)
        ));
    }
    if sample_entry_ids.is_empty() {
        sample_entry_ids = canonical_only
            .iter()
            .chain(mirror_only.iter())
            .chain(mismatched_shared.iter())
            .take(5)
            .cloned()
            .collect();
    }

    Ok(vec![FederationDivergenceRecord {
        kind: FederationDivergenceKind::MirrorDivergence,
        message,
        canonical_ledger_id: canonical.id.clone(),
        mirror_ledger_id: mirror.id.clone(),
        canonical_path: canonical.path.clone(),
        mirror_path: mirror.path.clone(),
        sample_entry_ids,
        canonical_fingerprint: Some(canonical_fingerprint),
        mirror_fingerprint: Some(mirror_fingerprint),
        recommended_action: "sync mirror from canonical or document intentional drain posture",
    }])
}

fn load_contained_policy(root: &Path, candidate: &Path) -> Option<AllowConfig> {
    let canonical_root = root.canonicalize().ok()?;
    let canonical_candidate = candidate.canonicalize().ok()?;
    canonical_candidate.strip_prefix(canonical_root).ok()?;
    load_policy(canonical_candidate).ok()
}

fn entry_id_set(cfg: &AllowConfig) -> BTreeSet<String> {
    cfg.allow.iter().map(|entry| entry.id.clone()).collect()
}

fn ledger_sync_fingerprint(cfg: &AllowConfig) -> String {
    let mut rows = cfg
        .allow
        .iter()
        .map(entry_sync_fingerprint)
        .collect::<Vec<_>>();
    rows.sort();
    stable_hash_hex(&rows.join("\n"))
}

fn entry_sync_fingerprint(entry: &AllowEntry) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}",
        entry.id,
        entry.kind.as_str(),
        entry.classification,
        entry.owner,
        entry.reason,
        entry.path_or_glob()
    )
}

fn join_sample_ids(ids: &BTreeSet<String>, limit: usize) -> String {
    ids.iter()
        .take(limit)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ")
}
