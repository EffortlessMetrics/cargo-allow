use allow_core::{AllowEntry, normalize_path};

use crate::policy_change::{
    PolicyChange, PolicyChangeKind, PolicyChangeSeverity, ScopeChange, ScopeChangeField,
};
use crate::policy_scope::{scope_broadened, scope_changed, scope_narrowed};

pub(crate) fn scope_policy_changes(base: &AllowEntry, head: &AllowEntry) -> Vec<PolicyChange> {
    let mut changes = Vec::new();
    if scope_broadened(base, head) {
        changes.push(change(
            base,
            head,
            PolicyChangeKind::ScopeBroadened,
            PolicyChangeSeverity::Fail,
            "scope broadened",
        ));
    }
    if scope_narrowed(base, head) {
        changes.push(change(
            base,
            head,
            PolicyChangeKind::ScopeNarrowed,
            PolicyChangeSeverity::Improvement,
            "scope narrowed",
        ));
    }
    if scope_changed(base, head) {
        changes.push(change(
            base,
            head,
            PolicyChangeKind::ScopeChanged,
            PolicyChangeSeverity::Review,
            "scope changed",
        ));
    }
    changes
}

fn change(
    base: &AllowEntry,
    entry: &AllowEntry,
    kind: PolicyChangeKind,
    severity: PolicyChangeSeverity,
    message: &str,
) -> PolicyChange {
    PolicyChange::new(
        entry.id.clone(),
        kind,
        severity,
        format!("{} {message}", entry.id),
    )
    .with_scope(first_scope_change(base, entry))
}

/// Return the first (most significant) scope change for backward-compat
/// with the single-scope PolicyChange model. The full set of changed
/// scope fields is available via all_scope_changes (#1932).
fn first_scope_change(base: &AllowEntry, head: &AllowEntry) -> ScopeChange {
    all_scope_changes(base, head)
        .into_iter()
        .next()
        .unwrap_or(ScopeChange {
            field: ScopeChangeField::Effective,
            before: effective_scope(base),
            after: effective_scope(head),
        })
}

/// Return ALL changed scope fields, not just the first (#1932).
/// A PR that changes both path AND glob will now produce entries for both.
fn all_scope_changes(base: &AllowEntry, head: &AllowEntry) -> Vec<ScopeChange> {
    let mut changes = Vec::new();

    let base_path = base.path.as_ref().map(normalize_path);
    let head_path = head.path.as_ref().map(normalize_path);
    let path_changed = base_path.is_some() && head_path.is_some() && base_path != head_path;
    let path_equal = base_path == head_path;
    if path_changed {
        changes.push(ScopeChange {
            field: ScopeChangeField::Path,
            before: base_path,
            after: head_path,
        });
    }

    let base_glob = base.glob.as_deref().map(normalize_scope_text);
    let head_glob = head.glob.as_deref().map(normalize_scope_text);
    if path_equal && base_glob != head_glob {
        changes.push(ScopeChange {
            field: ScopeChangeField::Glob,
            before: base_glob,
            after: head_glob,
        });
    }

    let base_sel = base.selector.glob.as_deref().map(normalize_scope_text);
    let head_sel = head.selector.glob.as_deref().map(normalize_scope_text);
    if path_equal && base_sel != head_sel {
        changes.push(ScopeChange {
            field: ScopeChangeField::SelectorGlob,
            before: base_sel,
            after: head_sel,
        });
    }

    changes
}

fn effective_scope(entry: &AllowEntry) -> Option<String> {
    entry
        .path
        .as_ref()
        .map(normalize_path)
        .or_else(|| entry.glob.as_deref().map(normalize_scope_text))
        .or_else(|| entry.selector.glob.as_deref().map(normalize_scope_text))
}

fn normalize_scope_text(scope: &str) -> String {
    scope.replace('\\', "/")
}

#[cfg(test)]
#[path = "policy_entry_scope_tests.rs"]
mod tests;
