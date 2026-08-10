//! Removal-window policy validator (#3368 / #2601 step 10).
//!
//! Loads `policy/removal-window-policy.toml` and proves every legacy
//! spec-system compatibility operation has a machine-checkable removal window
//! with independent alias retirement, and that historical read-only surfaces
//! can never grant current authority.

use serde::Deserialize;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_policy(name: &str) -> Result<String, String> {
    let root = workspace_root();
    let path = root.join("policy").join(name);
    std::fs::read_to_string(&path).map_err(|e| format!("read policy/{name}: {e}"))
}

/// Current cargo-allow product version line (major.minor).
const CURRENT_RELEASE: &str = "0.2.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Disposition {
    DelegateToIntentOperation,
    RenderMigrationOnly,
    HistoricalReadOnly,
    UnsupportedAndRemoved,
    AliasUntilGeneration,
}

impl Disposition {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "DelegateToIntentOperation" => Ok(Disposition::DelegateToIntentOperation),
            "RenderMigrationOnly" => Ok(Disposition::RenderMigrationOnly),
            "HistoricalReadOnly" => Ok(Disposition::HistoricalReadOnly),
            "UnsupportedAndRemoved" => Ok(Disposition::UnsupportedAndRemoved),
            "AliasUntilGeneration" => Ok(Disposition::AliasUntilGeneration),
            other => Err(format!("unknown disposition `{other}`")),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RemovalWindowRegistryToml {
    schema_id: String,
    schema_version: u32,
    controlling_issue: u32,
    #[serde(default)]
    generated_by: Option<String>,
    #[serde(rename = "operation")]
    operations: Vec<RemovalWindowRecordToml>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RemovalWindowRecordToml {
    operation_id: String,
    disposition: String,
    introduced_release: String,
    earliest_deprecation_release: String,
    latest_supported_release: String,
    latest_supported_generation: u32,
    migration_docs: String,
    removal_issue: String,
    removal_condition: String,
    rollback_route: String,
    independent_removal: bool,
}

#[derive(Debug, Clone)]
struct RemovalWindowRecord {
    operation_id: String,
    disposition: Disposition,
    introduced_release: String,
    earliest_deprecation_release: String,
    latest_supported_release: String,
    latest_supported_generation: u32,
    migration_docs: String,
    removal_issue: String,
    removal_condition: String,
    rollback_route: String,
    independent_removal: bool,
}

fn load_registry() -> Result<Vec<RemovalWindowRecord>, String> {
    let text = read_policy("removal-window-policy.toml")?;
    let raw: RemovalWindowRegistryToml =
        toml::from_str(&text).map_err(|e| format!("parse removal-window-policy.toml: {e}"))?;
    if raw.schema_id != "cargo-allow.removal-window-policy.v1" {
        return Err(format!(
            "unexpected schema_id `{}`; expected cargo-allow.removal-window-policy.v1",
            raw.schema_id
        ));
    }
    if raw.schema_version != 1 {
        return Err(format!(
            "unexpected schema_version {}; expected 1",
            raw.schema_version
        ));
    }
    if raw.controlling_issue != 2601 {
        return Err(format!(
            "unexpected controlling_issue {}; expected 2601",
            raw.controlling_issue
        ));
    }
    // generated_by is a provenance field; we only assert it is present and
    // points at the slice that authored the manifest.
    match &raw.generated_by {
        Some(value) if value.contains("#3368") => {}
        other => {
            return Err(format!(
                "generated_by must reference #3368; found `{}`",
                other.as_deref().unwrap_or("(missing)")
            ));
        }
    }
    raw.operations
        .into_iter()
        .map(|r| {
            let disposition = Disposition::parse(&r.disposition)?;
            Ok(RemovalWindowRecord {
                operation_id: r.operation_id,
                disposition,
                introduced_release: r.introduced_release,
                earliest_deprecation_release: r.earliest_deprecation_release,
                latest_supported_release: r.latest_supported_release,
                latest_supported_generation: r.latest_supported_generation,
                migration_docs: r.migration_docs,
                removal_issue: r.removal_issue,
                removal_condition: r.removal_condition,
                rollback_route: r.rollback_route,
                independent_removal: r.independent_removal,
            })
        })
        .collect()
}

/// Parse `command = "..."` and `surface = "..."` keys from the legacy
/// spec-system inventory so we can cross-check coverage.
fn inventory_operation_ids() -> Result<Vec<String>, String> {
    let text = read_policy("legacy-spec-system-inventory.toml")?;
    let mut ids = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("command = ") {
            let value = rest.trim_matches('"').trim();
            if !value.is_empty() {
                ids.push(value.to_string());
            }
        } else if let Some(rest) = trimmed.strip_prefix("surface = ") {
            let value = rest.trim_matches('"').trim();
            if !value.is_empty() {
                ids.push(value.to_string());
            }
        }
    }
    if ids.is_empty() {
        return Err("legacy-spec-system-inventory.toml yielded no operation ids".to_string());
    }
    Ok(ids)
}

/// Compare two `MAJOR.MINOR[.PATCH]` release strings. Returns true if `a < b`.
fn release_less_than(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> {
        s.split('.')
            .filter_map(|p| p.split('-').next())
            .filter_map(|p| p.parse::<u32>().ok())
            .collect()
    };
    let av = parse(a);
    let bv = parse(b);
    for i in 0..av.len().max(bv.len()) {
        let ai = av.get(i).copied().unwrap_or(0);
        let bi = bv.get(i).copied().unwrap_or(0);
        if ai != bi {
            return ai < bi;
        }
    }
    false
}

/// A record is past its latest-supported release when the current release
/// strictly exceeds it.
fn record_is_expired(record: &RemovalWindowRecord, current: &str) -> bool {
    release_less_than(&record.latest_supported_release, current)
}

/// Historical read-only surfaces must never claim to grant current authority.
/// A rollback route that promises to restore authority (rather than "none")
/// is treated as an authority-granting claim.
fn grants_current_authority(record: &RemovalWindowRecord) -> bool {
    if record.disposition != Disposition::HistoricalReadOnly {
        return false;
    }
    // "none — ..." means no restoration path. Anything else implies authority
    // can be re-granted, which HistoricalReadOnly must never allow.
    !record
        .rollback_route
        .trim_start()
        .to_lowercase()
        .starts_with("none")
}

#[test]
fn manifest_loads_and_has_required_fields() -> Result<(), String> {
    let records = load_registry()?;
    if records.is_empty() {
        return Err("removal-window-policy.toml has no operations".into());
    }
    for r in &records {
        for (label, value) in [
            ("operation_id", r.operation_id.as_str()),
            ("disposition", r.disposition_dbg()),
            ("introduced_release", r.introduced_release.as_str()),
            (
                "earliest_deprecation_release",
                r.earliest_deprecation_release.as_str(),
            ),
            (
                "latest_supported_release",
                r.latest_supported_release.as_str(),
            ),
            ("migration_docs", r.migration_docs.as_str()),
            ("removal_issue", r.removal_issue.as_str()),
            ("removal_condition", r.removal_condition.as_str()),
            ("rollback_route", r.rollback_route.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!(
                    "operation `{}` has empty required field `{}`",
                    r.operation_id, label
                ));
            }
        }
        if r.latest_supported_generation == 0 {
            return Err(format!(
                "operation `{}` has latest_supported_generation 0",
                r.operation_id
            ));
        }
        // introduced must precede deprecation must precede latest-supported.
        if !release_less_than(&r.introduced_release, &r.earliest_deprecation_release) {
            return Err(format!(
                "operation `{}` introduced_release `{}` must precede earliest_deprecation_release `{}`",
                r.operation_id, r.introduced_release, r.earliest_deprecation_release
            ));
        }
        if !release_less_than(&r.earliest_deprecation_release, &r.latest_supported_release) {
            return Err(format!(
                "operation `{}` earliest_deprecation_release `{}` must precede latest_supported_release `{}`",
                r.operation_id, r.earliest_deprecation_release, r.latest_supported_release
            ));
        }
    }
    Ok(())
}

impl RemovalWindowRecord {
    fn disposition_dbg(&self) -> &'static str {
        match self.disposition {
            Disposition::DelegateToIntentOperation => "DelegateToIntentOperation",
            Disposition::RenderMigrationOnly => "RenderMigrationOnly",
            Disposition::HistoricalReadOnly => "HistoricalReadOnly",
            Disposition::UnsupportedAndRemoved => "UnsupportedAndRemoved",
            Disposition::AliasUntilGeneration => "AliasUntilGeneration",
        }
    }
}

#[test]
fn every_inventory_surface_has_removal_window() -> Result<(), String> {
    let records = load_registry()?;
    let inventory = inventory_operation_ids()?;
    let manifest_ids: std::collections::HashSet<&str> =
        records.iter().map(|r| r.operation_id.as_str()).collect();
    let mut missing: Vec<&String> = inventory
        .iter()
        .filter(|id| !manifest_ids.contains(id.as_str()))
        .collect();
    if !missing.is_empty() {
        missing.sort();
        return Err(format!(
            "removal-window-policy.toml is missing operations present in legacy-spec-system-inventory.toml: {missing:?}"
        ));
    }
    // Every manifest operation must also be in the inventory (no orphans).
    let inventory_set: std::collections::HashSet<&str> =
        inventory.iter().map(|s| s.as_str()).collect();
    let orphans: Vec<&str> = records
        .iter()
        .map(|r| r.operation_id.as_str())
        .filter(|id| !inventory_set.contains(*id))
        .collect();
    if !orphans.is_empty() {
        return Err(format!(
            "removal-window-policy.toml has operations not in legacy-spec-system-inventory.toml: {orphans:?}"
        ));
    }
    Ok(())
}

#[test]
fn aliases_removable_independently() -> Result<(), String> {
    let records = load_registry()?;
    // Delegate + alias surfaces must each be removable on their own; the
    // command tree is not retained until the last historical reader is gone.
    let not_independent: Vec<&str> = records
        .iter()
        .filter(|r| {
            matches!(
                r.disposition,
                Disposition::DelegateToIntentOperation | Disposition::AliasUntilGeneration
            ) && !r.independent_removal
        })
        .map(|r| r.operation_id.as_str())
        .collect();
    if !not_independent.is_empty() {
        return Err(format!(
            "delegate/alias operations must set independent_removal = true: {not_independent:?}"
        ));
    }
    Ok(())
}

#[test]
fn validator_flags_operation_past_latest_supported_release() -> Result<(), String> {
    let records = load_registry()?;
    // No live record should be expired against the current release — that
    // would mean a compatibility operation has outlived its window.
    let expired: Vec<&str> = records
        .iter()
        .filter(|r| record_is_expired(r, CURRENT_RELEASE))
        .map(|r| r.operation_id.as_str())
        .collect();
    if !expired.is_empty() {
        return Err(format!(
            "operations past their latest_supported_release against {}: {expired:?}",
            CURRENT_RELEASE
        ));
    }
    // Seeded fixture: a record with latest_supported_release below the
    // current release MUST be flagged. Proves the comparator is real.
    let seeded = RemovalWindowRecord {
        operation_id: "seed-expired-fixture".to_string(),
        disposition: Disposition::DelegateToIntentOperation,
        introduced_release: "0.1.0".to_string(),
        earliest_deprecation_release: "0.1.2".to_string(),
        latest_supported_release: "0.1.5".to_string(),
        latest_supported_generation: 1,
        migration_docs: "docs/migration/spec-system-to-cargo-intent.md".to_string(),
        removal_issue: "#fixture".to_string(),
        removal_condition: "fixture".to_string(),
        rollback_route: "fixture".to_string(),
        independent_removal: true,
    };
    if !record_is_expired(&seeded, CURRENT_RELEASE) {
        return Err(format!(
            "seeded fixture with latest_supported_release 0.1.5 was not flagged as expired against {}",
            CURRENT_RELEASE
        ));
    }
    Ok(())
}

#[test]
fn historical_readonly_cannot_grant_current_authority() -> Result<(), String> {
    let records = load_registry()?;
    let granting: Vec<&str> = records
        .iter()
        .filter(|r| grants_current_authority(r))
        .map(|r| r.operation_id.as_str())
        .collect();
    if !granting.is_empty() {
        return Err(format!(
            "HistoricalReadOnly operations must not grant current authority (rollback_route must be `none`): {granting:?}"
        ));
    }
    // Seeded stale-authority fixture: a HistoricalReadOnly record that
    // promises to restore authority must be rejected.
    let seeded = RemovalWindowRecord {
        operation_id: "seed-stale-authority-fixture".to_string(),
        disposition: Disposition::HistoricalReadOnly,
        introduced_release: "0.1.0".to_string(),
        earliest_deprecation_release: "0.2.0".to_string(),
        latest_supported_release: "1.0.0".to_string(),
        latest_supported_generation: 2,
        migration_docs: "docs/migration/spec-system-to-cargo-intent.md".to_string(),
        removal_issue: "#fixture".to_string(),
        removal_condition: "fixture".to_string(),
        rollback_route: "restore embedded evaluator authority".to_string(),
        independent_removal: true,
    };
    if !grants_current_authority(&seeded) {
        return Err(
            "seeded HistoricalReadOnly fixture with an authority-restoring rollback_route was not rejected"
                .to_string(),
        );
    }
    Ok(())
}

#[test]
fn removal_does_not_require_last_historical_reader() -> Result<(), String> {
    let records = load_registry()?;
    // HistoricalReadOnly readers (legacy config, legacy goal) still exist,
    // but DelegateToIntentOperation operations must be removable without
    // waiting for them.
    let has_historical_readonly = records
        .iter()
        .any(|r| r.disposition == Disposition::HistoricalReadOnly);
    if !has_historical_readonly {
        return Err("expected at least one HistoricalReadOnly reader to prove independence".into());
    }
    let delegate_blocked_by_reader: Vec<&str> = records
        .iter()
        .filter(|r| {
            r.disposition == Disposition::DelegateToIntentOperation && !r.independent_removal
        })
        .map(|r| r.operation_id.as_str())
        .collect();
    if !delegate_blocked_by_reader.is_empty() {
        return Err(format!(
            "delegate operations must be removable without waiting for last historical reader: {delegate_blocked_by_reader:?}"
        ));
    }
    Ok(())
}
