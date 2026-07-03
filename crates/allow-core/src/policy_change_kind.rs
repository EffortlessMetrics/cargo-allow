//! Canonical policy-change-kind token vocabulary.
//!
//! `allow-diff` owns the `PolicyChangeKind` enum and its classification logic,
//! but `allow-diff` depends on `allow-policy` (not the reverse). To let
//! `allow-policy` validate revision-note `change_kinds` against the *same*
//! vocabulary the diff emits, the canonical token set is published here in
//! `allow-core`, the shared root both crates depend on.
//!
//! `allow-diff` binds its enum to this list with a parity test
//! (`PolicyChangeKind::ALL` mapped through `as_str` must equal
//! [`POLICY_CHANGE_KIND_TOKENS`], in order), so the two can never drift.

/// Canonical `snake_case` tokens for every `allow-diff` `PolicyChangeKind`.
///
/// Order matches `allow-diff`'s `PolicyChangeKind::ALL`; a parity test in
/// `allow-diff` enforces that.
pub const POLICY_CHANGE_KIND_TOKENS: &[&str] = &[
    "added_allow",
    "removed_allow",
    "baseline_debt_added",
    "baseline_debt_introduced",
    "baseline_debt_normalized",
    "kind_changed",
    "family_changed",
    "scope_broadened",
    "scope_changed",
    "scope_narrowed",
    "selector_changed",
    "selector_precision_decreased",
    "selector_precision_increased",
    "created_added",
    "created_changed",
    "created_removed",
    "expiry_extended",
    "expiry_shortened",
    "review_after_extended",
    "review_after_shortened",
    "evidence_added",
    "evidence_removed",
    "link_added",
    "link_removed",
    "owner_added",
    "owner_changed",
    "owner_removed",
    "owner_unassigned",
    "policy_owner_added",
    "policy_owner_changed",
    "policy_owner_removed",
    "policy_owner_unassigned",
    "policy_status_changed",
    "policy_status_weakened",
    "policy_status_tightened",
    "reason_added",
    "reason_changed",
    "reason_removed",
    "requirement_loosened",
    "requirement_tightened",
    "workspace_ignored_added",
    "workspace_ignored_removed",
    "workspace_generated_added",
    "workspace_generated_removed",
    "classification_added",
    "classification_changed",
    "classification_removed",
    "occurrence_limit_tightened",
    "occurrence_limit_loosened",
];

/// Whether `token` is a canonical `PolicyChangeKind` token.
pub fn is_policy_change_kind_token(token: &str) -> bool {
    POLICY_CHANGE_KIND_TOKENS.contains(&token)
}
