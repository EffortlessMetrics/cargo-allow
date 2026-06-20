//! `.allow/revisions/` policy revision record contract.
//!
//! A revision record documents *why* a governed exception or policy entry
//! changed posture. This module owns the parse and validate contract for the
//! records as designed in `CARGO-ALLOW-ADR-0002`. It is a design slice: it
//! defines and validates the record shape and exposes coverage semantics, but
//! it does **not** enforce notes on any command. Diff enforcement
//! (`diff --require-change-note`) lands in a later slice and reads the
//! `posture_delta` already classified by `allow-diff`.
//!
//! The canonical change-kind vocabulary is `allow-diff`'s `PolicyChangeKind`.
//! Because `allow-diff` depends on `allow-policy` (not the reverse), this module
//! validates change-kind *token shape* rather than the concrete enum set;
//! cross-checking a note's `change_kinds` against the change kinds observed in a
//! real diff is an enforcement-path concern.

use allow_core::{CargoAllowError, CargoAllowResult, SimpleDate};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;

/// Current `.allow/revisions/` record schema version.
pub const REVISION_SCHEMA_VERSION: &str = "1.0";

/// Stable-ID prefix for revision records.
pub const REVISION_ID_PREFIX: &str = "CARGO-ALLOW-REV-";

/// Recognized provenance link prefixes for revision records.
pub const REVISION_LINK_PREFIXES: &[&str] = &["issue:", "pr:", "doc:", "commit:", "url:"];

/// A parsed and validated policy revision record.
///
/// Records are append-only: a correction is a new record that `supersedes` the
/// prior one, mirroring the ADR `supersedes` / `superseded_by` chain. Records do
/// not auto-expire on merge; the optional `expires` field bounds advisory
/// freshness only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionRecord {
    pub schema_version: String,
    pub id: String,
    pub created: String,
    pub owner: String,
    pub reason: String,
    pub allow_ids: Vec<String>,
    pub change_kinds: Vec<String>,
    pub links: Vec<String>,
    pub expires: Option<String>,
    pub supersedes: Option<String>,
    pub superseded_by: Option<String>,
}

impl RevisionRecord {
    /// Whether this note claims coverage for a `(allow_id, change_kind)` cell.
    ///
    /// Coverage is structural, not positional: a record covers a changed entry
    /// when the entry's stable `allow_id` is listed and the observed
    /// `change_kind` is declared. Multiple records may jointly cover one diff;
    /// a weakening diff is fully covered when every weakening cell is claimed by
    /// at least one record.
    pub fn covers(&self, allow_id: &str, change_kind: &str) -> bool {
        self.allow_ids.iter().any(|id| id == allow_id)
            && self.change_kinds.iter().any(|kind| kind == change_kind)
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RevisionRecordToml {
    schema_version: Option<String>,
    id: Option<String>,
    created: Option<String>,
    owner: Option<String>,
    reason: Option<String>,
    #[serde(default)]
    allow_ids: Vec<String>,
    #[serde(default)]
    change_kinds: Vec<String>,
    #[serde(default)]
    links: Vec<String>,
    expires: Option<String>,
    supersedes: Option<String>,
    superseded_by: Option<String>,
}

impl RevisionRecordToml {
    fn into_record(self, path: &Path) -> CargoAllowResult<RevisionRecord> {
        let where_ = || path.display().to_string();

        let schema_version = required(self.schema_version, "schema_version", &where_)?;
        if schema_version != REVISION_SCHEMA_VERSION {
            return Err(CargoAllowError::new(format!(
                "{}: unsupported revision schema_version `{schema_version}` (expected `{REVISION_SCHEMA_VERSION}`)",
                where_()
            )));
        }

        let id = required(self.id, "id", &where_)?;
        if !id.starts_with(REVISION_ID_PREFIX) || id.len() == REVISION_ID_PREFIX.len() {
            return Err(CargoAllowError::new(format!(
                "{}: revision id `{id}` must start with `{REVISION_ID_PREFIX}` and carry a suffix",
                where_()
            )));
        }

        let created = required(self.created, "created", &where_)?;
        if SimpleDate::parse(&created).is_none() {
            return Err(CargoAllowError::new(format!(
                "{}: revision {id} has invalid created date `{created}`",
                where_()
            )));
        }

        let owner = required(self.owner, "owner", &where_)?;
        let reason = required(self.reason, "reason", &where_)?;

        if self.allow_ids.is_empty() {
            return Err(CargoAllowError::new(format!(
                "{}: revision {id} requires at least one allow_id",
                where_()
            )));
        }
        for allow_id in &self.allow_ids {
            if allow_id.trim().is_empty() {
                return Err(CargoAllowError::new(format!(
                    "{}: revision {id} has an empty allow_id",
                    where_()
                )));
            }
        }

        if self.change_kinds.is_empty() {
            return Err(CargoAllowError::new(format!(
                "{}: revision {id} requires at least one change_kind (blanket waivers are not allowed)",
                where_()
            )));
        }
        for kind in &self.change_kinds {
            validate_change_kind_token(&id, kind, &where_)?;
        }

        for link in &self.links {
            if !REVISION_LINK_PREFIXES
                .iter()
                .any(|prefix| link.starts_with(prefix) && link.len() > prefix.len())
            {
                return Err(CargoAllowError::new(format!(
                    "{}: revision {id} link `{link}` must use a recognized prefix ({})",
                    where_(),
                    REVISION_LINK_PREFIXES.join(", ")
                )));
            }
        }

        if let Some(expires) = self.expires.as_deref() {
            if expires != "never" && SimpleDate::parse(expires).is_none() {
                return Err(CargoAllowError::new(format!(
                    "{}: revision {id} has invalid expires date `{expires}`",
                    where_()
                )));
            }
        }

        let supersedes = validate_optional_ref(self.supersedes, &id, "supersedes", &where_)?;
        let superseded_by =
            validate_optional_ref(self.superseded_by, &id, "superseded_by", &where_)?;

        Ok(RevisionRecord {
            schema_version,
            id,
            created,
            owner,
            reason,
            allow_ids: self.allow_ids,
            change_kinds: self.change_kinds,
            links: self.links,
            expires: self.expires,
            supersedes,
            superseded_by,
        })
    }
}

fn required(
    value: Option<String>,
    field: &str,
    where_: &impl Fn() -> String,
) -> CargoAllowResult<String> {
    match value {
        Some(value) if !value.trim().is_empty() => Ok(value),
        _ => Err(CargoAllowError::new(format!(
            "{}: revision record missing required field `{field}`",
            where_()
        ))),
    }
}

fn validate_optional_ref(
    value: Option<String>,
    id: &str,
    field: &str,
    where_: &impl Fn() -> String,
) -> CargoAllowResult<Option<String>> {
    match value {
        Some(value) => {
            if !value.starts_with(REVISION_ID_PREFIX) || value.len() == REVISION_ID_PREFIX.len() {
                return Err(CargoAllowError::new(format!(
                    "{}: revision {id} {field} `{value}` must start with `{REVISION_ID_PREFIX}` and carry a suffix",
                    where_()
                )));
            }
            Ok(Some(value))
        }
        None => Ok(None),
    }
}

/// Validate change-kind token shape: `snake_case` over `[a-z0-9]`.
///
/// Matches the `^[a-z0-9]+(_[a-z0-9]+)*$` shape published in
/// `docs/schemas/revision.schema.json`: lowercase ascii and digits, single
/// underscores between segments, no leading/trailing/consecutive underscores.
fn validate_change_kind_token(
    id: &str,
    kind: &str,
    where_: &impl Fn() -> String,
) -> CargoAllowResult<()> {
    let valid = !kind.is_empty()
        && kind
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !kind.starts_with('_')
        && !kind.ends_with('_')
        && !kind.contains("__");
    if valid {
        Ok(())
    } else {
        Err(CargoAllowError::new(format!(
            "{}: revision {id} has invalid change_kind token `{kind}` (expected snake_case)",
            where_()
        )))
    }
}

/// Parse a single revision record from TOML text.
pub fn parse_revision_record(input: &str) -> CargoAllowResult<RevisionRecord> {
    parse_revision_record_at(Path::new("<revision>"), input)
}

/// Parse a single revision record from TOML text, attributing errors to `path`.
pub fn parse_revision_record_at(path: &Path, input: &str) -> CargoAllowResult<RevisionRecord> {
    let raw = toml::from_str::<RevisionRecordToml>(input).map_err(|e| {
        CargoAllowError::new(format!(
            "failed to parse revision record in {}: {e}",
            path.display()
        ))
    })?;
    raw.into_record(path)
}

/// Validate cross-record invariants for an append-only revision ledger.
///
/// Records carry stable IDs and are append-only, so duplicate IDs indicate a
/// rewritten or copied record and are rejected. Coverage and per-diff matching
/// are enforcement-path concerns handled where a real diff is available.
pub fn validate_revision_ledger(records: &[RevisionRecord]) -> CargoAllowResult<()> {
    let mut seen = BTreeSet::new();
    for record in records {
        if !seen.insert(record.id.as_str()) {
            return Err(CargoAllowError::new(format!(
                "duplicate revision id `{}` (revision records are append-only)",
                record.id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
