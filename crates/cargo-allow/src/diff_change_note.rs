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

use allow_core::{AllowConfig, AllowEntry, PostureDelta, Selector, stable_hash_hex};
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
                "path={:?}|glob={:?}|{}",
                path,
                entry.glob,
                selector_scope_material(&entry.selector)
            )
        }
        "selector_precision_decreased" => selector_scope_material(&entry.selector),
        _ => return None,
    };
    Some(format!("v1:{}", stable_hash_hex(&material)))
}

/// Canonical scope/identity material for a selector, excluding the cosmetic
/// `line_hint`. `line_hint` carries no scope meaning and is rewritten by
/// `cargo-allow refresh` whenever surrounding source lines shift, so including it
/// would make a previously-authorized transition spuriously fail after an
/// unrelated refresh.
fn selector_scope_material(selector: &Selector) -> String {
    format!(
        "selector[ast_kind={:?}|container={:?}|callee={:?}|macro_name={:?}|lint={:?}|symbol={:?}|receiver_fingerprint={:?}|target_fingerprint={:?}|normalized_snippet_hash={:?}|glob={:?}]",
        selector.ast_kind,
        selector.container,
        selector.callee,
        selector.macro_name,
        selector.lint,
        selector.symbol,
        selector.receiver_fingerprint,
        selector.target_fingerprint,
        selector.normalized_snippet_hash,
        selector.glob,
    )
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
/// Emits **one complete record per uncovered cell** rather than aggregating ids
/// and kinds into a single record: because coverage is the cartesian product of a
/// record's `allow_ids` × `change_kinds`, aggregating would silently pre-authorize
/// cells that never occurred in this diff. Repeatable-kind cells carry an active
/// `after_fingerprint` so the emitted record actually covers the transition. When
/// there is more than one cell, each record must be saved to its own file.
pub(crate) fn change_note_template(uncovered: &[UncoveredCell]) -> String {
    if uncovered.is_empty() {
        return "# No uncovered weakening edits in this diff; no change note is required.\n"
            .to_string();
    }

    let mut out = String::new();
    if uncovered.len() > 1 {
        out.push_str(&format!(
            "# {} uncovered weakening edits. Each block below is a separate record;\n\
             # save each to its own .allow/revisions/<id>.toml file with a unique id.\n\
             # Fill in id, created, owner, and reason. Records are append-only.\n\n",
            uncovered.len()
        ));
    } else {
        out.push_str(
            "# Fill in id, created, owner, and reason, then save this file to\n\
             # .allow/revisions/<id>.toml. Records are append-only.\n\n",
        );
    }

    for (index, cell) in uncovered.iter().enumerate() {
        if index > 0 {
            out.push_str("\n# --- separate record: save to its own file ---\n\n");
        }
        out.push_str("schema_version = \"1.0\"\n");
        out.push_str("id = \"CARGO-ALLOW-REV-XXXX\"\n");
        out.push_str("created = \"YYYY-MM-DD\"\n");
        out.push_str("owner = \"TODO-owner\"\n");
        out.push_str("reason = \"TODO: why this weakening edit is justified.\"\n");
        out.push_str(&format!("allow_ids = [\"{}\"]\n", cell.allow_id));
        out.push_str(&format!("change_kinds = [\"{}\"]\n", cell.change_kind));
        if let Some(fingerprint) = &cell.after_fingerprint {
            out.push_str(&format!("after_fingerprint = \"{fingerprint}\"\n"));
        }
    }
    out
}

#[cfg(test)]
#[path = "diff_change_note/tests.rs"]
mod tests;
