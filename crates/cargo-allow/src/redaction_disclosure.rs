//! Fail-closed disclosure when identity redaction meets structural ledger
//! entries (#4628).
//!
//! `CARGO_ALLOW_REDACT_IDENTITY=1` clears the source-text identity fields at
//! scan time (allow-core `redact_source_text_fields`) *before* matching, and
//! #4629 extends that to raw-text receiver/target fingerprints. Every pinned
//! selector field is a hard equality gate in `classify_match`
//! (allow-match `scoring.rs`), so a ledger entry that pins one of the cleared
//! fields can never reach a unique-winner tier under redaction — not even the
//! `ExactOccurrence` tier, whose snippet-hash gate runs in the same
//! conjunction. Before this guard, such entries silently reported `matched:0`
//! and stale, and a redacted `check --mode no-new` failed with zero
//! diagnostics (or, with no findings left, passed green while the structural
//! ledger was never evaluated at all).
//!
//! #4628 semantics decision — hard typed error, raised in the enforcing gate
//! only:
//!
//! * The guard is a hard [`CargoAllowErrorKind::Unsupported`] error rather
//!   than a listing because the alternative is silent assurance loss: a
//!   redacted run cannot evaluate these entries either way, so reporting a
//!   verdict (green or stale-driven red) without saying why would launder an
//!   unevaluable ledger into a passing receipt.
//! * It is raised in `check` (source-tree, compat, and staged lanes) — the
//!   point where entries meet the redacted scan and the earliest point that
//!   can name the dead entry ids — and deliberately NOT at world load:
//!   `add` must stay usable under redaction so the operator can re-receipt
//!   occurrences into snippet-hash-anchored entries that match redacted scans
//!   (a world-load hard error would make that migration impossible), and the
//!   read-only inspectors (`list`, `explain`, `why`) stay available for
//!   auditing the ledger in redacted CI. `add` instead prints the advisory
//!   warning from [`redaction_dead_entry_add_warning`] when it generates an
//!   entry that only matches unredacted scans (#4628).

use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, Selector,
};

/// The maximum number of dead entry ids spelled out in the disclosure before
/// the rest are summarized by count.
const MAX_DISCLOSED_ENTRY_IDS: usize = 8;

/// The selector fields whose finding-side value is guaranteed cleared before
/// matching when identity redaction is active (#4628, #4629).
///
/// `container`, `callee`, `macro_name`, `lint`, and `symbol` are always
/// `None` on a redacted finding, so an entry pinning any of them fails its
/// hard gate on every candidate. A raw-text receiver fingerprint (and every
/// target fingerprint — targets have no slot form) is cleared by #4629 the
/// same way. `ast_kind`, `normalized_snippet_hash`, and `param:N` slot-form
/// fingerprints are preserved and stay live, so they are never listed.
pub(crate) fn redaction_dead_selector_fields(selector: &Selector) -> Vec<&'static str> {
    let mut dead = Vec::new();
    let named: [(&str, &Option<String>); 5] = [
        ("container", &selector.container),
        ("callee", &selector.callee),
        ("macro_name", &selector.macro_name),
        ("lint", &selector.lint),
        ("symbol", &selector.symbol),
    ];
    for (name, value) in named {
        if value.is_some() {
            dead.push(name);
        }
    }
    // `param:N` fingerprints are structural slots, not source text; they
    // survive redaction (#4629) and keep matching.
    if let Some(receiver) = &selector.receiver_fingerprint
        && !allow_core::is_slot_form_fingerprint(receiver)
    {
        dead.push("receiver_fingerprint");
    }
    if selector.target_fingerprint.is_some() {
        dead.push("target_fingerprint");
    }
    dead
}

/// The ledger entries that can never match a redacted scan, with the dead
/// selector fields each one pins (#4628). Sorted by entry id for a stable
/// disclosure.
pub(crate) fn redaction_dead_entries(cfg: &AllowConfig) -> Vec<(&str, Vec<&'static str>)> {
    let mut dead = cfg
        .allow
        .iter()
        .map(|entry| {
            (
                entry.id.as_str(),
                redaction_dead_selector_fields(&entry.selector),
            )
        })
        .filter(|(_, fields)| !fields.is_empty())
        .collect::<Vec<_>>();
    dead.sort_by_key(|(id, _)| *id);
    dead
}

/// Build the typed disclosure error for `dead` entries (#4628). Split from
/// [`reject_redaction_dead_entries`] so the message shape is executable — and
/// coverage-measured — without setting the process-global redaction env var.
pub(crate) fn redaction_dead_entries_error(dead: &[(&str, Vec<&'static str>)]) -> CargoAllowError {
    let detail = dead
        .iter()
        .take(MAX_DISCLOSED_ENTRY_IDS)
        .map(|(id, fields)| format!("`{id}` ({})", fields.join(", ")))
        .collect::<Vec<_>>()
        .join(", ");
    let remainder = dead.len().saturating_sub(MAX_DISCLOSED_ENTRY_IDS);
    let remainder_note = if remainder > 0 {
        format!(", and {remainder} more")
    } else {
        String::new()
    };
    CargoAllowError::with_kind(
        CargoAllowErrorKind::Unsupported,
        format!(
            "CARGO_ALLOW_REDACT_IDENTITY=1 is active, but {} ledger entry(s) pin selector fields \
             that redaction clears before matching and can never match a redacted scan: \
             {detail}{remainder_note}",
            dead.len()
        ),
    )
    .with_next_step(
        "re-run without CARGO_ALLOW_REDACT_IDENTITY=1 to evaluate these structural entries, \
         or re-receipt the occurrences under redaction so the entries anchor on \
         normalized_snippet_hash instead of source-text selector fields",
    )
}

/// Fail closed when identity redaction is active and the loaded ledger pins
/// selector fields that cannot match a redacted scan (#4628). A no-op (and
/// the normal case) when redaction is off or every entry stays live.
pub(crate) fn reject_redaction_dead_entries(cfg: &AllowConfig) -> CargoAllowResult<()> {
    if !allow_core::identity_redaction_requested() {
        return Ok(());
    }
    let dead = redaction_dead_entries(cfg);
    if dead.is_empty() {
        return Ok(());
    }
    Err(redaction_dead_entries_error(&dead))
}

/// The `add` advisory for an entry that only matches unredacted scans (#4628).
///
/// `add` never refuses (the operator may be receipting for an unredacted
/// pipeline); it prints one stderr warning line when redaction is active and
/// the generated entry pins dead selector fields. `None` when redaction is
/// off or the entry stays live. Split from the env-gated
/// [`redaction_dead_entry_add_warning`] so the message branch is executable
/// without the process-global env var.
pub(crate) fn redaction_dead_entry_add_warning_message(entry: &AllowEntry) -> Option<String> {
    let fields = redaction_dead_selector_fields(&entry.selector);
    if fields.is_empty() {
        return None;
    }
    Some(format!(
        "warning: entry `{}` pins selector field(s) ({}) that CARGO_ALLOW_REDACT_IDENTITY=1 \
         clears before matching; the entry will only match unredacted scans",
        entry.id,
        fields.join(", ")
    ))
}

/// The env-gated form of [`redaction_dead_entry_add_warning_message`] used at
/// the `add` call site (#4628).
pub(crate) fn redaction_dead_entry_add_warning(entry: &AllowEntry) -> Option<String> {
    if !allow_core::identity_redaction_requested() {
        return None;
    }
    redaction_dead_entry_add_warning_message(entry)
}

#[cfg(test)]
#[path = "redaction_disclosure_tests.rs"]
mod tests;
