use allow_core::AllowEntry;

use crate::policy_change::{
    EvidenceChange, EvidenceChangeField, PolicyChange, PolicyChangeKind, PolicyChangeSeverity,
};
use crate::policy_compare::{added_values, removed_values};

pub(crate) fn evidence_policy_changes(base: &AllowEntry, head: &AllowEntry) -> Vec<PolicyChange> {
    let mut changes = Vec::new();
    if removed_values(&base.evidence, &head.evidence) {
        let removed = removed_items(&base.evidence, &head.evidence);
        let severity = removed_evidence_severity(&removed, &head.evidence);
        let message = removed_evidence_message(&removed, severity);
        changes.push(change(
            head,
            EvidenceChangeField::Evidence,
            removed,
            Vec::new(),
            PolicyChangeKind::EvidenceRemoved,
            severity,
            message,
        ));
    }
    if added_values(&base.evidence, &head.evidence) {
        let added = added_items(&base.evidence, &head.evidence);
        let severity = added_evidence_severity(&added);
        changes.push(change(
            head,
            EvidenceChangeField::Evidence,
            Vec::new(),
            added,
            PolicyChangeKind::EvidenceAdded,
            severity,
            added_evidence_message(severity),
        ));
    }
    if removed_values(&base.links, &head.links) {
        let removed = removed_items(&base.links, &head.links);
        let severity = removed_link_severity(&removed, &head.links);
        changes.push(change(
            head,
            EvidenceChangeField::Links,
            removed,
            Vec::new(),
            PolicyChangeKind::LinkRemoved,
            severity,
            removed_link_message(severity),
        ));
    }
    if added_values(&base.links, &head.links) {
        let added = added_items(&base.links, &head.links);
        let severity = added_link_severity(&added);
        changes.push(change(
            head,
            EvidenceChangeField::Links,
            Vec::new(),
            added,
            PolicyChangeKind::LinkAdded,
            severity,
            added_link_message(severity),
        ));
    }
    changes
}

fn change(
    entry: &AllowEntry,
    field: EvidenceChangeField,
    removed: Vec<String>,
    added: Vec<String>,
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
    .with_evidence(EvidenceChange {
        field,
        removed,
        added,
    })
}

fn added_evidence_severity(added: &[String]) -> PolicyChangeSeverity {
    if added
        .iter()
        .any(|item| evidence_reference_is_invalid_local(item))
    {
        return PolicyChangeSeverity::Fail;
    }
    if added.iter().any(|item| evidence_reference_is_weak(item)) {
        PolicyChangeSeverity::Review
    } else {
        PolicyChangeSeverity::Improvement
    }
}

fn added_link_severity(added: &[String]) -> PolicyChangeSeverity {
    if added
        .iter()
        .any(|item| evidence_reference_is_invalid_local(item))
    {
        return PolicyChangeSeverity::Fail;
    }
    if added.iter().any(|item| reference_is_weak(item)) {
        PolicyChangeSeverity::Review
    } else {
        PolicyChangeSeverity::Improvement
    }
}

fn removed_link_severity(removed: &[String], remaining: &[String]) -> PolicyChangeSeverity {
    if removed.iter().any(|item| reference_is_local_file(item)) {
        PolicyChangeSeverity::Fail
    } else if removed.iter().all(|item| reference_is_weak(item))
        && remaining.iter().any(|item| !reference_is_weak(item))
    {
        PolicyChangeSeverity::Improvement
    } else {
        PolicyChangeSeverity::Review
    }
}

fn removed_evidence_severity(removed: &[String], remaining: &[String]) -> PolicyChangeSeverity {
    if removed.iter().any(|item| !evidence_reference_is_weak(item)) {
        return PolicyChangeSeverity::Fail;
    }
    if remaining
        .iter()
        .any(|item| !evidence_reference_is_weak(item))
    {
        PolicyChangeSeverity::Improvement
    } else {
        PolicyChangeSeverity::Review
    }
}

fn added_evidence_message(severity: PolicyChangeSeverity) -> &'static str {
    match severity {
        PolicyChangeSeverity::Review => "weak evidence added",
        PolicyChangeSeverity::Improvement => "evidence added",
        PolicyChangeSeverity::Fail => "invalid local evidence added",
    }
}

fn added_link_message(severity: PolicyChangeSeverity) -> &'static str {
    match severity {
        PolicyChangeSeverity::Review => "weak traceability link added",
        PolicyChangeSeverity::Improvement => "traceability link added",
        PolicyChangeSeverity::Fail => "invalid traceability link added",
    }
}

fn removed_evidence_message(removed: &[String], severity: PolicyChangeSeverity) -> &'static str {
    if severity != PolicyChangeSeverity::Fail {
        return "weak evidence removed";
    }
    if removed.iter().any(|item| reference_is_local_file(item)) {
        "local evidence removed"
    } else {
        "evidence removed"
    }
}

fn removed_link_message(severity: PolicyChangeSeverity) -> &'static str {
    match severity {
        PolicyChangeSeverity::Fail => "local traceability link removed",
        PolicyChangeSeverity::Improvement => "weak traceability link removed",
        PolicyChangeSeverity::Review => "traceability link removed",
    }
}

fn evidence_reference_is_invalid_local(reference: &str) -> bool {
    let Some((prefix, target)) = reference.split_once(':') else {
        return false;
    };
    if !reference_prefix_is_local_file(prefix) {
        return false;
    }
    let target = target.trim().replace('\\', "/");
    target.is_empty()
        || target.starts_with('/')
        || target.contains(':')
        || target.split('/').any(|part| part == "." || part == "..")
        || target.chars().any(|ch| matches!(ch, '*' | '?'))
}

fn evidence_reference_is_weak(reference: &str) -> bool {
    reference_is_weak(reference)
}

fn reference_is_local_file(reference: &str) -> bool {
    reference
        .split_once(':')
        .map(|(prefix, _)| reference_prefix_is_local_file(prefix))
        .unwrap_or(false)
}

fn reference_prefix_is_local_file(prefix: &str) -> bool {
    allow_policy::local_file_evidence_prefixes().any(|known| known == prefix.trim())
}

fn reference_is_weak(reference: &str) -> bool {
    let Some((prefix, target)) = reference.split_once(':') else {
        return true;
    };
    let prefix = prefix.trim();
    let target = target.trim();
    target.is_empty() || !allow_policy::recognized_evidence_prefixes().any(|known| known == prefix)
}

fn removed_items(base: &[String], head: &[String]) -> Vec<String> {
    base.iter()
        .filter(|item| !head.iter().any(|head| head == *item))
        .cloned()
        .collect()
}

fn added_items(base: &[String], head: &[String]) -> Vec<String> {
    head.iter()
        .filter(|item| !base.iter().any(|base| base == *item))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{FindingKind, Lifecycle, Selector};
    use std::path::PathBuf;

    fn entry(id: &str, evidence: &[&str], links: &[&str]) -> AllowEntry {
        AllowEntry {
            id: id.to_string(),
            kind: FindingKind::Unsafe,
            family: Some("unsafe_block".to_string()),
            path: Some(PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "repo-infra".to_string(),
            classification: "approved".to_string(),
            reason: "covered by retained evidence".to_string(),
            evidence: evidence.iter().map(|item| item.to_string()).collect(),
            links: links.iter().map(|item| item.to_string()).collect(),
            occurrence_limit: Some(1),
            lifecycle: Lifecycle::empty(),
            selector: Selector::default(),
            last_seen: None,
        }
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn evidence_policy_changes_emits_removed_and_added_evidence_and_links() {
        let base = entry(
            "allow-unsafe",
            &["test:range-is-covered", "binary:cargo"],
            &["adr:docs/adr/0001.md", "issue:123"],
        );
        let head = entry(
            "allow-unsafe",
            &["test:range-is-covered", "test:new-coverage"],
            &["issue:123", "pr:456"],
        );

        let changes = evidence_policy_changes(&base, &head);

        assert_eq!(changes.len(), 4);
        let [removed_evidence, added_evidence, removed_link, added_link] = changes.as_slice()
        else {
            return;
        };
        assert_eq!(removed_evidence.kind, PolicyChangeKind::EvidenceRemoved);
        assert_eq!(removed_evidence.severity, PolicyChangeSeverity::Improvement);
        assert_eq!(
            removed_evidence.message,
            "allow-unsafe weak evidence removed"
        );
        assert_eq!(
            removed_evidence.evidence.as_ref().map(|evidence| (
                evidence.field,
                evidence.removed.clone(),
                evidence.added.clone()
            )),
            Some((
                EvidenceChangeField::Evidence,
                strings(&["binary:cargo"]),
                Vec::new()
            ))
        );
        assert_eq!(added_evidence.kind, PolicyChangeKind::EvidenceAdded);
        assert_eq!(added_evidence.severity, PolicyChangeSeverity::Improvement);
        assert_eq!(added_evidence.message, "allow-unsafe evidence added");
        assert_eq!(
            added_evidence.evidence.as_ref().map(|evidence| (
                evidence.field,
                evidence.removed.clone(),
                evidence.added.clone()
            )),
            Some((
                EvidenceChangeField::Evidence,
                Vec::new(),
                strings(&["test:new-coverage"])
            ))
        );
        assert_eq!(removed_link.kind, PolicyChangeKind::LinkRemoved);
        assert_eq!(removed_link.severity, PolicyChangeSeverity::Fail);
        assert_eq!(
            removed_link.message,
            "allow-unsafe local traceability link removed"
        );
        assert_eq!(
            removed_link.evidence.as_ref().map(|evidence| (
                evidence.field,
                evidence.removed.clone(),
                evidence.added.clone()
            )),
            Some((
                EvidenceChangeField::Links,
                strings(&["adr:docs/adr/0001.md"]),
                Vec::new()
            ))
        );
        assert_eq!(added_link.kind, PolicyChangeKind::LinkAdded);
        assert_eq!(added_link.severity, PolicyChangeSeverity::Improvement);
        assert_eq!(added_link.message, "allow-unsafe traceability link added");
        assert_eq!(
            added_link.evidence.as_ref().map(|evidence| (
                evidence.field,
                evidence.removed.clone(),
                evidence.added.clone()
            )),
            Some((EvidenceChangeField::Links, Vec::new(), strings(&["pr:456"])))
        );
    }

    #[test]
    fn severity_helpers_classify_added_reference_boundaries() {
        assert_eq!(
            added_evidence_severity(&strings(&["doc:../outside.md"])),
            PolicyChangeSeverity::Fail
        );
        assert_eq!(
            added_evidence_severity(&strings(&["spreadsheet:manual-review"])),
            PolicyChangeSeverity::Review
        );
        assert_eq!(
            added_evidence_severity(&strings(&["test:range-is-covered"])),
            PolicyChangeSeverity::Improvement
        );
        assert_eq!(
            added_link_severity(&strings(&["doc:../outside.md"])),
            PolicyChangeSeverity::Fail
        );
        assert_eq!(
            added_link_severity(&strings(&["manual review note"])),
            PolicyChangeSeverity::Review
        );
        assert_eq!(
            added_link_severity(&strings(&["issue:123"])),
            PolicyChangeSeverity::Improvement
        );
    }

    #[test]
    fn severity_helpers_classify_removed_reference_boundaries() {
        assert_eq!(
            removed_evidence_severity(&strings(&["doc:docs/safety.md"]), &[]),
            PolicyChangeSeverity::Fail
        );
        assert_eq!(
            removed_evidence_severity(
                &strings(&["binary:cargo"]),
                &strings(&["test:range-is-covered"])
            ),
            PolicyChangeSeverity::Improvement
        );
        assert_eq!(
            removed_evidence_severity(&strings(&["binary:cargo"]), &[]),
            PolicyChangeSeverity::Review
        );
        assert_eq!(
            removed_link_severity(
                &strings(&["adr:docs/adr/0001.md"]),
                &strings(&["issue:123"])
            ),
            PolicyChangeSeverity::Fail
        );
        assert_eq!(
            removed_link_severity(
                &strings(&["spreadsheet:manual-review"]),
                &strings(&["pr:123"])
            ),
            PolicyChangeSeverity::Improvement
        );
        assert_eq!(
            removed_link_severity(&strings(&["issue:123"]), &strings(&["pr:456"])),
            PolicyChangeSeverity::Review
        );
    }

    #[test]
    fn message_helpers_follow_severity_and_local_removed_state() {
        assert_eq!(
            added_evidence_message(PolicyChangeSeverity::Review),
            "weak evidence added"
        );
        assert_eq!(
            added_evidence_message(PolicyChangeSeverity::Improvement),
            "evidence added"
        );
        assert_eq!(
            added_evidence_message(PolicyChangeSeverity::Fail),
            "invalid local evidence added"
        );
        assert_eq!(
            added_link_message(PolicyChangeSeverity::Review),
            "weak traceability link added"
        );
        assert_eq!(
            added_link_message(PolicyChangeSeverity::Improvement),
            "traceability link added"
        );
        assert_eq!(
            added_link_message(PolicyChangeSeverity::Fail),
            "invalid traceability link added"
        );
        assert_eq!(
            removed_evidence_message(&strings(&["binary:cargo"]), PolicyChangeSeverity::Review),
            "weak evidence removed"
        );
        assert_eq!(
            removed_evidence_message(
                &strings(&["binary:cargo"]),
                PolicyChangeSeverity::Improvement
            ),
            "weak evidence removed"
        );
        assert_eq!(
            removed_evidence_message(
                &strings(&["doc:docs/safety.md"]),
                PolicyChangeSeverity::Fail
            ),
            "local evidence removed"
        );
        assert_eq!(
            removed_evidence_message(&strings(&["test:old-proof"]), PolicyChangeSeverity::Fail),
            "evidence removed"
        );
        assert_eq!(
            removed_link_message(PolicyChangeSeverity::Fail),
            "local traceability link removed"
        );
        assert_eq!(
            removed_link_message(PolicyChangeSeverity::Improvement),
            "weak traceability link removed"
        );
        assert_eq!(
            removed_link_message(PolicyChangeSeverity::Review),
            "traceability link removed"
        );
    }

    #[test]
    fn removed_evidence_message_treats_improvement_as_non_fail_boundary() {
        let removed = Vec::new();

        assert_eq!(
            removed_evidence_message(&removed, PolicyChangeSeverity::Improvement),
            "weak evidence removed"
        );
    }

    #[test]
    fn reference_classification_handles_local_invalid_and_weak_boundaries() {
        assert!(evidence_reference_is_invalid_local("doc:"));
        assert!(evidence_reference_is_invalid_local("doc:/absolute.md"));
        assert!(evidence_reference_is_invalid_local("doc:docs/../safety.md"));
        assert!(evidence_reference_is_invalid_local("doc:docs/./safety.md"));
        assert!(evidence_reference_is_invalid_local("doc:docs/*.md"));
        assert!(!evidence_reference_is_invalid_local(
            "doc:docs/safety/parser-spans.md"
        ));
        assert!(!evidence_reference_is_invalid_local(
            "test:../not-a-local-file-path.md"
        ));

        assert!(reference_is_local_file(" doc : docs/safety.md"));
        assert!(!reference_is_local_file("issue:123"));
        assert!(reference_prefix_is_local_file(" doc "));
        assert!(!reference_prefix_is_local_file("issue"));

        assert!(evidence_reference_is_weak("manual review note"));
        assert!(reference_is_weak("test:"));
        assert!(reference_is_weak("spreadsheet:manual-review"));
        assert!(!reference_is_weak("test:range-is-covered"));
    }

    #[test]
    fn added_and_removed_item_helpers_preserve_head_order() {
        let base = strings(&["a", "b", "c"]);
        let head = strings(&["c", "d", "a", "e"]);

        assert_eq!(removed_items(&base, &head), strings(&["b"]));
        assert_eq!(added_items(&base, &head), strings(&["d", "e"]));
    }
}
