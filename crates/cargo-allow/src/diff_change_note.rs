//! `diff --require-change-note` enforcement.
//!
//! Reads the `posture_delta` already classified by `allow-diff` (PR 2) and
//! fails the diff when a governed *weakening* edit — a policy change whose
//! `posture_delta` is `worsened` or `review_required` — is not covered by a
//! matching `.allow/revisions/*.toml` record (`CARGO-ALLOW-ADR-0002`).
//!
//! Coverage is structural on `(allow_id, change_kind)`. For *repeatable*
//! weakening kinds (raising an occurrence limit, extending an expiry, broadening
//! scope) the record must additionally pin the exact transition via an
//! `after_fingerprint` equal to the fingerprint of the head entry's post-edit
//! state, so a note authorizing one increase cannot silently authorize the next.

use allow_core::{AllowConfig, AllowEntry, PostureDelta, stable_hash_hex};
use allow_diff::{PolicyChange, classify_policy_change};
use allow_policy::{RevisionRecord, is_repeatable_change_kind, load_revision_records};
use std::path::Path;

/// A weakening edit that no revision record covers.
pub(crate) struct UncoveredCell {
    pub allow_id: String,
    pub change_kind: String,
    pub posture: &'static str,
    /// `Some` only for repeatable kinds, where a transition fingerprint is required.
    pub after_fingerprint: Option<String>,
}

/// Result of evaluating change-note coverage over a diff's policy changes.
pub(crate) struct ChangeNoteEvaluation {
    pub uncovered: Vec<UncoveredCell>,
    pub weakening_cells: usize,
    pub records_loaded: usize,
}

impl ChangeNoteEvaluation {
    pub fn failed(&self) -> bool {
        !self.uncovered.is_empty()
    }
}

/// Evaluate whether every weakening policy change is covered by a revision note.
///
/// `head_cfg` is the post-edit policy; each weakening change's `allow_id` is
/// looked up there to compute the transition fingerprint for repeatable kinds.
pub(crate) fn evaluate_change_notes(
    root: &Path,
    head_cfg: &AllowConfig,
    policy_changes: &[PolicyChange],
) -> allow_core::CargoAllowResult<ChangeNoteEvaluation> {
    let records = load_revision_records(root)?;
    Ok(evaluate_with_records(head_cfg, policy_changes, &records))
}

/// Pure coverage evaluation over an already-loaded record set.
pub(crate) fn evaluate_with_records(
    head_cfg: &AllowConfig,
    policy_changes: &[PolicyChange],
    records: &[RevisionRecord],
) -> ChangeNoteEvaluation {
    let mut uncovered = Vec::new();
    let mut weakening_cells = 0usize;

    for change in policy_changes {
        let posture = match classify_policy_change(change).posture_delta {
            PostureDelta::Worsened => "worsened",
            PostureDelta::ReviewRequired => "review_required",
            PostureDelta::Improved | PostureDelta::Unchanged => continue,
        };
        weakening_cells += 1;

        let kind = change.kind.as_str();
        let after_fingerprint = head_cfg
            .allow
            .iter()
            .find(|entry| entry.id == change.allow_id)
            .and_then(|entry| change_after_fingerprint(entry, kind));

        let covered = records.iter().any(|record| {
            record.covers_transition(&change.allow_id, kind, after_fingerprint.as_deref())
        });
        if !covered {
            uncovered.push(UncoveredCell {
                allow_id: change.allow_id.clone(),
                change_kind: kind.to_string(),
                posture,
                after_fingerprint,
            });
        }
    }

    ChangeNoteEvaluation {
        uncovered,
        weakening_cells,
        records_loaded: records.len(),
    }
}

/// Deterministic fingerprint of the head entry's post-edit state for a
/// repeatable weakening `kind`. `None` for non-repeatable kinds (which are
/// pinned adequately by `(allow_id, change_kind)` alone).
///
/// The same function is used by enforcement and by the change-note template
/// writer, so the operator's recorded `after_fingerprint` matches what
/// enforcement recomputes for the committed head state — until a *further* edit
/// changes the fingerprint, at which point the old record no longer covers.
fn change_after_fingerprint(entry: &AllowEntry, kind: &str) -> Option<String> {
    if !is_repeatable_change_kind(kind) {
        return None;
    }
    let material = match kind {
        "occurrence_limit_loosened" => format!("occurrence_limit={:?}", entry.occurrence_limit),
        "expiry_extended" => format!("expires={:?}", entry.lifecycle.expires),
        "review_after_extended" => format!("review_after={:?}", entry.lifecycle.review_after),
        "scope_broadened" => {
            // Slash-normalize the path so the fingerprint is platform-independent
            // (a note written on Windows must verify on a Linux CI runner).
            let path = entry
                .path
                .as_ref()
                .map(|p| p.to_string_lossy().replace('\\', "/"));
            format!(
                "path={:?}|glob={:?}|selector={:?}",
                path, entry.glob, entry.selector
            )
        }
        "selector_precision_decreased" => format!("selector={:?}", entry.selector),
        _ => return None,
    };
    Some(format!("v1:{}", stable_hash_hex(&material)))
}

/// Human/Markdown section describing change-note coverage and remediation.
pub(crate) fn render_change_note_section(eval: &ChangeNoteEvaluation) -> String {
    let mut out = String::new();
    out.push_str("\nChange-note enforcement (--require-change-note):\n");
    out.push_str(&format!("  weakening edits: {}\n", eval.weakening_cells));
    out.push_str(&format!("  revision records: {}\n", eval.records_loaded));
    if eval.uncovered.is_empty() {
        out.push_str("  status: all weakening edits are covered by a revision note.\n");
    } else {
        out.push_str(&format!(
            "  status: FAIL — {} uncovered weakening edit(s):\n",
            eval.uncovered.len()
        ));
        for cell in &eval.uncovered {
            out.push_str(&format!(
                "    - {} {} ({}) requires a revision note\n",
                cell.allow_id, cell.change_kind, cell.posture
            ));
        }
        out.push_str(
            "  add a .allow/revisions/<id>.toml note covering each edit \
             (see --write-change-note-template).\n",
        );
    }
    out
}

/// Generate a starter `.allow/revisions/` record covering the uncovered cells.
///
/// The template aggregates every uncovered `allow_id` and `change_kind`. Because
/// each *repeatable* transition needs its own `after_fingerprint`, those are
/// emitted as comments for the operator to place on per-transition records
/// rather than silently collapsed.
pub(crate) fn change_note_template(uncovered: &[UncoveredCell]) -> String {
    if uncovered.is_empty() {
        return "# No uncovered weakening edits in this diff; no change note is required.\n"
            .to_string();
    }
    let mut allow_ids: Vec<&str> = uncovered.iter().map(|c| c.allow_id.as_str()).collect();
    allow_ids.sort_unstable();
    allow_ids.dedup();
    let mut kinds: Vec<&str> = uncovered.iter().map(|c| c.change_kind.as_str()).collect();
    kinds.sort_unstable();
    kinds.dedup();

    let quote_join = |items: &[&str]| {
        items
            .iter()
            .map(|item| format!("\"{item}\""))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let mut out = String::new();
    out.push_str(
        "# Generated change-note template. Fill in id, created, owner, and reason,\n\
         # then move this file to .allow/revisions/<id>.toml. Records are append-only.\n\n",
    );
    out.push_str("schema_version = \"1.0\"\n");
    out.push_str("id = \"CARGO-ALLOW-REV-XXXX\"\n");
    out.push_str("created = \"YYYY-MM-DD\"\n");
    out.push_str("owner = \"TODO-owner\"\n");
    out.push_str("reason = \"TODO: why these weakening edits are justified.\"\n\n");
    out.push_str(&format!("allow_ids = [{}]\n", quote_join(&allow_ids)));
    out.push_str(&format!("change_kinds = [{}]\n", quote_join(&kinds)));

    let repeatable: Vec<&UncoveredCell> = uncovered
        .iter()
        .filter(|cell| cell.after_fingerprint.is_some())
        .collect();
    if !repeatable.is_empty() {
        out.push_str(
            "\n# Repeatable weakening kinds must pin the transition with after_fingerprint.\n\
             # A record with more than one repeatable transition must be split into one\n\
             # file per transition, each carrying the matching fingerprint below:\n",
        );
        for cell in repeatable {
            out.push_str(&format!(
                "#   after_fingerprint = \"{}\"  # {} / {}\n",
                cell.after_fingerprint.as_deref().unwrap_or_default(),
                cell.allow_id,
                cell.change_kind
            ));
        }
    }
    out
}

#[cfg(test)]
#[path = "diff_change_note/tests.rs"]
mod tests;
