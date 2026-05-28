use allow_core::{AllowConfig, AllowEntry, CargoAllowResult, SimpleDate};
use allow_policy::parse_policy;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::policy_change::{PolicyChange, PolicyChangeKind, PolicyChangeSeverity};

pub fn policy_changes_from_git(
    root: impl AsRef<Path>,
    base: &str,
    policy_path: impl AsRef<Path>,
    head_cfg: &AllowConfig,
) -> CargoAllowResult<Vec<PolicyChange>> {
    let Some(base_cfg) = policy_config_at_revision(root, base, policy_path)? else {
        return Ok(Vec::new());
    };
    Ok(policy_changes(&base_cfg, head_cfg))
}

pub fn policy_config_at_revision(
    root: impl AsRef<Path>,
    revision: &str,
    policy_path: impl AsRef<Path>,
) -> CargoAllowResult<Option<AllowConfig>> {
    let Some(text) = crate::read_file_at_revision(root, revision, policy_path)? else {
        return Ok(None);
    };
    parse_policy(&text).map(Some)
}

pub fn policy_changes(base: &AllowConfig, head: &AllowConfig) -> Vec<PolicyChange> {
    let base_by_id = base
        .allow
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let head_ids = head
        .allow
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut changes = Vec::new();
    for head_entry in &head.allow {
        let Some(base_entry) = base_by_id.get(head_entry.id.as_str()).copied() else {
            changes.push(added_allow_change(head_entry));
            continue;
        };
        changes.extend(entry_policy_changes(base_entry, head_entry));
    }
    for base_entry in &base.allow {
        if !head_ids.contains(base_entry.id.as_str()) {
            changes.push(removed_allow_change(base_entry));
        }
    }
    changes
}

fn added_allow_change(entry: &AllowEntry) -> PolicyChange {
    let baseline = entry.classification == "baseline_debt";
    PolicyChange {
        allow_id: entry.id.clone(),
        kind: if baseline {
            PolicyChangeKind::BaselineDebtAdded
        } else {
            PolicyChangeKind::AddedAllow
        },
        severity: if baseline {
            PolicyChangeSeverity::Fail
        } else {
            PolicyChangeSeverity::Review
        },
        message: if baseline {
            format!("{} added generated baseline debt", entry.id)
        } else {
            format!("{} added a new allow entry", entry.id)
        },
    }
}

fn removed_allow_change(entry: &AllowEntry) -> PolicyChange {
    PolicyChange {
        allow_id: entry.id.clone(),
        kind: PolicyChangeKind::RemovedAllow,
        severity: PolicyChangeSeverity::Improvement,
        message: format!("{} removed an allow entry", entry.id),
    }
}

fn entry_policy_changes(base: &AllowEntry, head: &AllowEntry) -> Vec<PolicyChange> {
    let mut changes = Vec::new();
    if scope_broadened(base, head) {
        changes.push(change(
            head,
            PolicyChangeKind::ScopeBroadened,
            PolicyChangeSeverity::Fail,
            "scope broadened",
        ));
    }
    if scope_narrowed(base, head) {
        changes.push(change(
            head,
            PolicyChangeKind::ScopeNarrowed,
            PolicyChangeSeverity::Improvement,
            "scope narrowed",
        ));
    }
    let base_precision = selector_precision_score(base);
    let head_precision = selector_precision_score(head);
    if head_precision < base_precision {
        changes.push(PolicyChange {
            allow_id: head.id.clone(),
            kind: PolicyChangeKind::SelectorPrecisionDecreased,
            severity: PolicyChangeSeverity::Fail,
            message: format!(
                "{} selector precision decreased: {} -> {}",
                head.id, base_precision, head_precision
            ),
        });
    } else if head_precision > base_precision {
        changes.push(PolicyChange {
            allow_id: head.id.clone(),
            kind: PolicyChangeKind::SelectorPrecisionIncreased,
            severity: PolicyChangeSeverity::Improvement,
            message: format!(
                "{} selector precision increased: {} -> {}",
                head.id, base_precision, head_precision
            ),
        });
    }
    if date_extended(
        base.lifecycle.expires.as_deref(),
        head.lifecycle.expires.as_deref(),
    ) {
        changes.push(change(
            head,
            PolicyChangeKind::ExpiryExtended,
            PolicyChangeSeverity::Review,
            "expiry extended or removed",
        ));
    }
    if date_shortened(
        base.lifecycle.expires.as_deref(),
        head.lifecycle.expires.as_deref(),
    ) {
        changes.push(change(
            head,
            PolicyChangeKind::ExpiryShortened,
            PolicyChangeSeverity::Improvement,
            "expiry shortened or added",
        ));
    }
    if date_extended(
        base.lifecycle.review_after.as_deref(),
        head.lifecycle.review_after.as_deref(),
    ) {
        changes.push(change(
            head,
            PolicyChangeKind::ReviewAfterExtended,
            PolicyChangeSeverity::Review,
            "review_after extended or removed",
        ));
    }
    if date_shortened(
        base.lifecycle.review_after.as_deref(),
        head.lifecycle.review_after.as_deref(),
    ) {
        changes.push(change(
            head,
            PolicyChangeKind::ReviewAfterShortened,
            PolicyChangeSeverity::Improvement,
            "review_after shortened or added",
        ));
    }
    if removed_values(&base.evidence, &head.evidence) {
        changes.push(change(
            head,
            PolicyChangeKind::EvidenceRemoved,
            PolicyChangeSeverity::Fail,
            "evidence removed",
        ));
    }
    if added_values(&base.evidence, &head.evidence) {
        changes.push(change(
            head,
            PolicyChangeKind::EvidenceAdded,
            PolicyChangeSeverity::Improvement,
            "evidence added",
        ));
    }
    if removed_required_text(&base.owner, &head.owner) {
        changes.push(change(
            head,
            PolicyChangeKind::OwnerRemoved,
            PolicyChangeSeverity::Fail,
            "owner removed",
        ));
    }
    if added_required_text(&base.owner, &head.owner) {
        changes.push(change(
            head,
            PolicyChangeKind::OwnerAdded,
            PolicyChangeSeverity::Improvement,
            "owner added",
        ));
    }
    if removed_required_text(&base.reason, &head.reason) {
        changes.push(change(
            head,
            PolicyChangeKind::ReasonRemoved,
            PolicyChangeSeverity::Fail,
            "reason removed",
        ));
    }
    if added_required_text(&base.reason, &head.reason) {
        changes.push(change(
            head,
            PolicyChangeKind::ReasonAdded,
            PolicyChangeSeverity::Improvement,
            "reason added",
        ));
    }
    if removed_required_text(&base.classification, &head.classification) {
        changes.push(change(
            head,
            PolicyChangeKind::ClassificationRemoved,
            PolicyChangeSeverity::Fail,
            "classification removed",
        ));
    }
    if added_required_text(&base.classification, &head.classification) {
        changes.push(change(
            head,
            PolicyChangeKind::ClassificationAdded,
            PolicyChangeSeverity::Improvement,
            "classification added",
        ));
    }
    if occurrence_limit_loosened(base.occurrence_limit, head.occurrence_limit) {
        changes.push(change(
            head,
            PolicyChangeKind::OccurrenceLimitLoosened,
            PolicyChangeSeverity::Fail,
            "occurrence_limit increased or removed",
        ));
    }
    if occurrence_limit_tightened(base.occurrence_limit, head.occurrence_limit) {
        changes.push(change(
            head,
            PolicyChangeKind::OccurrenceLimitTightened,
            PolicyChangeSeverity::Improvement,
            "occurrence_limit tightened",
        ));
    }
    changes
}

fn change(
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
    }
}

pub fn selector_precision_score(entry: &AllowEntry) -> u32 {
    let selector = &entry.selector;
    let mut score = 0;
    if entry.path.is_some() {
        score += 20;
    }
    if entry.glob.is_some() || selector.glob.is_some() {
        score += 5;
    }
    if entry.family.is_some() {
        score += 10;
    }
    if selector.ast_kind.is_some() {
        score += 15;
    }
    if selector.container.is_some() {
        score += 15;
    }
    if selector.callee.is_some() {
        score += 10;
    }
    if selector.macro_name.is_some() {
        score += 10;
    }
    if selector.lint.is_some() {
        score += 10;
    }
    if selector.symbol.is_some() {
        score += 8;
    }
    if selector.receiver_fingerprint.is_some() {
        score += 6;
    }
    if selector.target_fingerprint.is_some() {
        score += 6;
    }
    if selector.normalized_snippet_hash.is_some() {
        score += 20;
    }
    if entry.occurrence_limit.is_some() {
        score += 5;
    }
    score
}

fn scope_broadened(base: &AllowEntry, head: &AllowEntry) -> bool {
    let base_exact_path =
        base.path.is_some() && base.glob.is_none() && base.selector.glob.is_none();
    let head_uses_glob = head.glob.is_some() || head.selector.glob.is_some();
    if base_exact_path && head_uses_glob {
        return true;
    }
    if glob_scope_broadened(base.glob.as_deref(), head.glob.as_deref())
        || glob_scope_broadened(base.selector.glob.as_deref(), head.selector.glob.as_deref())
    {
        return true;
    }
    match (entry_scope_text(base), entry_scope_text(head)) {
        (Some(base_scope), Some(head_scope)) => {
            head_scope.contains('*')
                && !base_scope.contains('*')
                && wildcard_covers_path(head_scope, base_scope)
        }
        _ => false,
    }
}

fn scope_narrowed(base: &AllowEntry, head: &AllowEntry) -> bool {
    !scope_broadened(base, head) && scope_broadened(head, base)
}

fn glob_scope_broadened(base: Option<&str>, head: Option<&str>) -> bool {
    match (base, head) {
        (Some(base), Some(head)) => head != base && wildcard_covers_path(head, base),
        (None, Some(head)) => head.contains('*'),
        _ => false,
    }
}

fn entry_scope_text(entry: &AllowEntry) -> Option<&str> {
    entry
        .path
        .as_ref()
        .and_then(|path| path.to_str())
        .or(entry.glob.as_deref())
        .or(entry.selector.glob.as_deref())
}

fn wildcard_covers_path(pattern: &str, path: &str) -> bool {
    if pattern == "*" || pattern == "**" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path.starts_with(prefix);
    }
    if let Some(prefix) = pattern.split('*').next() {
        return !prefix.is_empty() && path.starts_with(prefix);
    }
    false
}

fn date_extended(base: Option<&str>, head: Option<&str>) -> bool {
    match (base, head) {
        (Some(base), Some(head)) if base == head => false,
        (Some(base), Some("never")) if base != "never" => true,
        (Some(base), Some(head)) => match (SimpleDate::parse(base), SimpleDate::parse(head)) {
            (Some(base_date), Some(head_date)) => head_date > base_date,
            _ => false,
        },
        (Some(base), None) => base != "never",
        _ => false,
    }
}

fn date_shortened(base: Option<&str>, head: Option<&str>) -> bool {
    match (base, head) {
        (_, Some("never")) => false,
        (Some(base), Some(head)) if base == head => false,
        (Some("never"), Some(head)) => SimpleDate::parse(head).is_some(),
        (Some(base), Some(head)) => match (SimpleDate::parse(base), SimpleDate::parse(head)) {
            (Some(base_date), Some(head_date)) => head_date < base_date,
            _ => false,
        },
        (None, Some(head)) => SimpleDate::parse(head).is_some(),
        _ => false,
    }
}

fn removed_values(base: &[String], head: &[String]) -> bool {
    base.iter()
        .any(|item| !head.iter().any(|head| head == item))
}

fn added_values(base: &[String], head: &[String]) -> bool {
    head.iter()
        .any(|item| !base.iter().any(|base| base == item))
}

fn removed_required_text(base: &str, head: &str) -> bool {
    !base.trim().is_empty() && head.trim().is_empty()
}

fn added_required_text(base: &str, head: &str) -> bool {
    base.trim().is_empty() && !head.trim().is_empty()
}

fn occurrence_limit_loosened(base: Option<u32>, head: Option<u32>) -> bool {
    match (base, head) {
        (Some(_), None) => true,
        (Some(base), Some(head)) => head > base,
        _ => false,
    }
}

fn occurrence_limit_tightened(base: Option<u32>, head: Option<u32>) -> bool {
    match (base, head) {
        (None, Some(_)) => true,
        (Some(base), Some(head)) => head < base,
        _ => false,
    }
}
