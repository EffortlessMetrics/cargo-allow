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
    PolicyChange {
        allow_id: entry.id.clone(),
        kind,
        severity,
        message: format!("{} {message}", entry.id),
        selector_precision: None,
        scope: Some(scope_change(base, entry)),
        occurrence_limit: None,
        lifecycle: None,
        evidence: None,
    }
}

fn scope_change(base: &AllowEntry, head: &AllowEntry) -> ScopeChange {
    let base_path = base.path.as_ref().map(normalize_path);
    let head_path = head.path.as_ref().map(normalize_path);
    if base_path.is_some() && head_path.is_some() && base_path != head_path {
        return ScopeChange {
            field: ScopeChangeField::Path,
            before: base_path,
            after: head_path,
        };
    }

    let base_glob = base.glob.as_deref().map(normalize_scope_text);
    let head_glob = head.glob.as_deref().map(normalize_scope_text);
    if base_glob.is_some() && head_glob.is_some() && base_glob != head_glob {
        return ScopeChange {
            field: ScopeChangeField::Glob,
            before: base_glob,
            after: head_glob,
        };
    }

    let base_selector_glob = base.selector.glob.as_deref().map(normalize_scope_text);
    let head_selector_glob = head.selector.glob.as_deref().map(normalize_scope_text);
    if base_selector_glob.is_some()
        && head_selector_glob.is_some()
        && base_selector_glob != head_selector_glob
    {
        return ScopeChange {
            field: ScopeChangeField::SelectorGlob,
            before: base_selector_glob,
            after: head_selector_glob,
        };
    }

    ScopeChange {
        field: ScopeChangeField::Effective,
        before: effective_scope(base),
        after: effective_scope(head),
    }
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
