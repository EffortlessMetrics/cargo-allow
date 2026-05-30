use allow_core::{AllowEntry, CargoAllowError, CargoAllowResult, FindingKind, Requirements};
use std::collections::BTreeSet;

pub(crate) fn validate_allow_entry_identity(
    entry: &AllowEntry,
    ids: &mut BTreeSet<String>,
) -> CargoAllowResult<()> {
    validate_allow_id(&entry.id)?;
    if entry
        .family
        .as_deref()
        .is_some_and(|family| family.trim().is_empty())
    {
        return Err(CargoAllowError::new(format!(
            "{} family must not be empty",
            entry.id
        )));
    }
    if let Some(family) = entry.family.as_deref() {
        validate_no_surrounding_whitespace(&entry.id, "family", family)?;
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
        validate_no_surrounding_whitespace(&entry.id, "owner", &entry.owner)?;
    }
    if !entry.classification.is_empty() {
        validate_no_surrounding_whitespace(&entry.id, "classification", &entry.classification)?;
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
    Ok(())
}

fn validate_no_surrounding_whitespace(id: &str, label: &str, value: &str) -> CargoAllowResult<()> {
    if value.trim() != value {
        return Err(CargoAllowError::new(format!(
            "{id} {label} must not have leading or trailing whitespace"
        )));
    }
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
        if value.trim().is_empty() {
            return Err(CargoAllowError::new(format!(
                "{id} {label} entry {} must not be empty",
                index + 1
            )));
        }
    }
    Ok(())
}
