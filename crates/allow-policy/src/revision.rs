//! `.allow/revisions/` policy revision record contract.
//!
//! A revision record documents *why* a governed exception or policy entry
//! changed posture. This module owns the parse and validate contract for the
//! records as designed in `CARGO-ALLOW-ADR-0002`. It defines and validates the
//! record shape and exposes coverage semantics. Diff enforcement
//! (`diff --require-change-note`) reads the `posture_delta` already classified
//! by `allow-diff` and consumes records via [`RevisionRecord::covers`].
//!
//! The canonical change-kind vocabulary is `allow-diff`'s `PolicyChangeKind`.
//! Because `allow-diff` depends on `allow-policy` (not the reverse), the
//! canonical token set is published in `allow-core`
//! ([`allow_core::POLICY_CHANGE_KIND_TOKENS`]); this module validates a note's
//! `change_kinds` against that shared list.

use allow_core::{CargoAllowError, CargoAllowResult, POLICY_CHANGE_KIND_TOKENS, SimpleDate};
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
///
/// `before_fingerprint` / `after_fingerprint` optionally bind a record to one
/// specific transition of an entry. They are required to cover a *repeatable*
/// weakening kind (see [`is_repeatable_change_kind`]) so that a note authorizing
/// one `occurrence_limit_loosened` does not silently authorize every later
/// increase.
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
    pub before_fingerprint: Option<String>,
    pub after_fingerprint: Option<String>,
}

impl RevisionRecord {
    /// Whether this note claims coverage for a `(allow_id, change_kind)` cell.
    ///
    /// Coverage is structural, not positional: a record covers a changed entry
    /// when the entry's stable `allow_id` is listed and the observed
    /// `change_kind` is declared. Multiple records may jointly cover one diff;
    /// a weakening diff is fully covered when every weakening cell is claimed by
    /// at least one record.
    ///
    /// For repeatable weakening kinds this cell-level match is necessary but not
    /// sufficient — use [`RevisionRecord::covers_transition`] to also require a
    /// matching `after_fingerprint`.
    pub fn covers(&self, allow_id: &str, change_kind: &str) -> bool {
        self.allow_ids.iter().any(|id| id == allow_id)
            && self.change_kinds.iter().any(|kind| kind == change_kind)
    }

    /// Whether this note covers a specific transition of an entry.
    ///
    /// Extends [`RevisionRecord::covers`] with the repeatable-weakening guard:
    /// when `change_kind` is repeatable, the record must also pin the transition
    /// via an `after_fingerprint` equal to `after_fingerprint`. Non-repeatable
    /// kinds ignore the fingerprint. A `None` observed fingerprint (the caller
    /// could not compute one) can never satisfy a repeatable kind.
    pub fn covers_transition(
        &self,
        allow_id: &str,
        change_kind: &str,
        after_fingerprint: Option<&str>,
    ) -> bool {
        if !self.covers(allow_id, change_kind) {
            return false;
        }
        if !is_repeatable_change_kind(change_kind) {
            return true;
        }
        match (self.after_fingerprint.as_deref(), after_fingerprint) {
            (Some(recorded), Some(observed)) => recorded == observed,
            _ => false,
        }
    }
}

/// Whether a change kind can recur, each recurrence weakening posture further.
///
/// Repeatable kinds (raising an occurrence limit, pushing out an expiry,
/// broadening scope) can be applied again and again; a note for one such change
/// must not silently authorize the next. Non-repeatable kinds (removing the
/// owner, removing evidence) are effectively one-shot for a given baseline and
/// are adequately identified by `(allow_id, change_kind)` alone.
pub fn is_repeatable_change_kind(change_kind: &str) -> bool {
    matches!(
        change_kind,
        "scope_broadened"
            | "selector_precision_decreased"
            | "occurrence_limit_loosened"
            | "expiry_extended"
            | "review_after_extended"
    )
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
    before_fingerprint: Option<String>,
    after_fingerprint: Option<String>,
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

        // Trim so an accidental leading/trailing space cannot silently defeat the
        // exact `allow_id` match at diff-evaluation time.
        let allow_ids: Vec<String> = self
            .allow_ids
            .into_iter()
            .map(|allow_id| allow_id.trim().to_string())
            .collect();
        if allow_ids.is_empty() {
            return Err(CargoAllowError::new(format!(
                "{}: revision {id} requires at least one allow_id",
                where_()
            )));
        }
        for allow_id in &allow_ids {
            if allow_id.is_empty() {
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
            allow_ids,
            change_kinds: self.change_kinds,
            links: self.links,
            expires: self.expires,
            supersedes,
            superseded_by,
            before_fingerprint: self.before_fingerprint,
            after_fingerprint: self.after_fingerprint,
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

/// Validate a change-kind token against the canonical `PolicyChangeKind` set.
///
/// The token must be a member of [`allow_core::POLICY_CHANGE_KIND_TOKENS`], the
/// shared source of truth for `allow-diff`'s `PolicyChangeKind`. Enforcing
/// membership (not just `snake_case` shape) means a note can only authorize a
/// change kind the diff can actually emit — typos and invented tokens are
/// rejected at parse time.
fn validate_change_kind_token(
    id: &str,
    kind: &str,
    where_: &impl Fn() -> String,
) -> CargoAllowResult<()> {
    if POLICY_CHANGE_KIND_TOKENS.contains(&kind) {
        Ok(())
    } else {
        Err(CargoAllowError::new(format!(
            "{}: revision {id} has unknown change_kind `{kind}` (not a canonical PolicyChangeKind token)",
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

/// Relative path of the revision-records directory under the source-tree root.
pub const REVISIONS_DIR_REL_PATH: &str = ".allow/revisions";

/// Load and validate every revision record under `<root>/.allow/revisions/`.
///
/// Only `*.toml` files are records; the `revision.schema.json` and `README.md`
/// that share the directory are skipped by the extension filter. Records are
/// returned sorted by file path for determinism, and the append-only ledger
/// invariants ([`validate_revision_ledger`]) are checked before returning. A
/// missing directory is not an error — it yields an empty ledger.
pub fn load_revision_records(root: &Path) -> CargoAllowResult<Vec<RevisionRecord>> {
    let dir = root.join(REVISIONS_DIR_REL_PATH);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|e| {
        CargoAllowError::new(format!(
            "failed to read revisions directory {}: {e}",
            dir.display()
        ))
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| {
            CargoAllowError::new(format!(
                "failed to read revisions directory entry in {}: {e}",
                dir.display()
            ))
        })?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("toml") {
            paths.push(path);
        }
    }
    paths.sort();

    let mut records = Vec::with_capacity(paths.len());
    for path in &paths {
        let text = std::fs::read_to_string(path).map_err(|e| {
            CargoAllowError::new(format!(
                "failed to read revision record {}: {e}",
                path.display()
            ))
        })?;
        records.push(parse_revision_record_at(path, &text)?);
    }

    validate_revision_ledger(&records)?;
    Ok(records)
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
