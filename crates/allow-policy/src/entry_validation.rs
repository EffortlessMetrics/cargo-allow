use allow_core::{AllowEntry, CargoAllowError, CargoAllowResult, FindingKind, Requirements};
use std::collections::BTreeSet;

use crate::evidence_reference::{EvidenceReference, recognized_evidence_prefixes};
use crate::source_tree_scope::validate_path_scope;
use crate::text_validation::{validate_no_surrounding_whitespace, validate_required_text};

pub(crate) fn validate_allow_entry_identity(
    entry: &AllowEntry,
    ids: &mut BTreeSet<String>,
) -> CargoAllowResult<()> {
    validate_allow_id(&entry.id)?;
    if let Some(family) = entry.family.as_deref() {
        validate_required_text(&format!("{} family", entry.id), family)?;
    }
    if !ids.insert(entry.id.clone()) {
        return Err(CargoAllowError::new(format!(
            "duplicate allow id `{}`",
            entry.id
        )));
    }
    Ok(())
}

pub(crate) fn validate_allow_entry_requirements(
    entry: &AllowEntry,
    requirements: &Requirements,
) -> CargoAllowResult<()> {
    if !entry.owner.is_empty() {
        validate_no_surrounding_whitespace(&format!("{} owner", entry.id), &entry.owner)?;
    }
    if !entry.classification.is_empty() {
        validate_no_surrounding_whitespace(
            &format!("{} classification", entry.id),
            &entry.classification,
        )?;
    }
    if !entry.reason.is_empty() {
        validate_no_surrounding_whitespace(&format!("{} reason", entry.id), &entry.reason)?;
    }
    if requirements.owner_required && entry.owner.trim().is_empty() {
        return Err(CargoAllowError::new(format!("{} missing owner", entry.id)));
    }
    if requirements.owner_required
        && entry.owner.trim() == "unowned"
        && entry.classification != "baseline_debt"
    {
        return Err(CargoAllowError::new(format!(
            "{} missing concrete owner",
            entry.id
        )));
    }
    if requirements.reason_required && entry.reason.trim().is_empty() {
        return Err(CargoAllowError::new(format!("{} missing reason", entry.id)));
    }
    if requirements.classification_required && entry.classification.trim().is_empty() {
        return Err(CargoAllowError::new(format!(
            "{} missing classification",
            entry.id
        )));
    }
    validate_non_empty_values(&entry.id, "evidence", &entry.evidence)?;
    validate_non_empty_values(&entry.id, "link", &entry.links)?;
    validate_unique_values(&entry.id, "evidence", &entry.evidence)?;
    validate_unique_values(&entry.id, "link", &entry.links)?;
    validate_local_link_scopes(entry)?;
    Ok(())
}

pub(crate) fn validate_allow_entry_evidence_and_limit(
    entry: &AllowEntry,
    requirements: &Requirements,
) -> CargoAllowResult<()> {
    if requirements.unsafe_evidence_required
        && entry.kind == FindingKind::Unsafe
        && entry.evidence.is_empty()
    {
        return Err(CargoAllowError::new(format!(
            "{} unsafe entry missing evidence",
            entry.id
        )));
    }
    if let Some(label) = typed_evidence_required_label(entry)
        && !entry
            .evidence
            .iter()
            .any(|evidence| evidence_is_typed(evidence))
    {
        return Err(CargoAllowError::new(format!(
            "{} {label} entry requires at least one typed evidence reference",
            entry.id,
        )));
    }
    if requirements.evidence_required && entry.evidence.is_empty() {
        return Err(CargoAllowError::new(format!(
            "{} missing evidence",
            entry.id
        )));
    }
    if entry.occurrence_limit == Some(0) {
        return Err(CargoAllowError::new(format!(
            "{} occurrence_limit must be greater than zero",
            entry.id
        )));
    }
    Ok(())
}

fn typed_evidence_required_label(entry: &AllowEntry) -> Option<String> {
    if entry.classification == "baseline_debt" {
        return None;
    }
    match (entry.kind, entry.family.as_deref()) {
        (FindingKind::Unsafe, _) => Some("unsafe".to_string()),
        (FindingKind::PolicyException, Some("process_spawn" | "network_destination")) => entry
            .family
            .as_deref()
            .map(|family| format!("policy_exception.{family}")),
        _ => None,
    }
}

fn evidence_is_typed(evidence: &str) -> bool {
    let Some((prefix, target)) = evidence.split_once(':') else {
        return false;
    };
    let prefix = prefix.trim();
    let target = target.trim();
    !target.is_empty() && recognized_evidence_prefixes().any(|known| known == prefix)
}

fn validate_allow_id(id: &str) -> CargoAllowResult<()> {
    if id.trim().is_empty() {
        return Err(CargoAllowError::new("allow entry has empty id"));
    }
    if id.trim() != id {
        return Err(CargoAllowError::new(format!(
            "allow id `{id}` must not have leading or trailing whitespace"
        )));
    }
    if !id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        return Err(CargoAllowError::new(format!(
            "allow id `{id}` may contain only ASCII letters, digits, hyphen, or underscore"
        )));
    }
    Ok(())
}

fn validate_non_empty_values(id: &str, label: &str, values: &[String]) -> CargoAllowResult<()> {
    for (index, value) in values.iter().enumerate() {
        validate_required_text(&format!("{id} {label} entry {}", index + 1), value)?;
    }
    Ok(())
}

fn validate_unique_values(id: &str, label: &str, values: &[String]) -> CargoAllowResult<()> {
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if !seen.insert(value.as_str()) {
            return Err(CargoAllowError::new(format!(
                "{} duplicate {} entry `{}` at position {}",
                id,
                label,
                value,
                index + 1
            )));
        }
    }
    Ok(())
}

fn validate_local_link_scopes(entry: &AllowEntry) -> CargoAllowResult<()> {
    for (index, link) in entry.links.iter().enumerate() {
        let Some(reference) = EvidenceReference::parse(link) else {
            continue;
        };
        if !reference.kind.is_local_file() {
            continue;
        }
        validate_path_scope(
            &format!("{} link entry {}", entry.id, index + 1),
            &reference.value,
        )?;
    }
    Ok(())
}
