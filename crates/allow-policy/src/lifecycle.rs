use allow_core::{AllowEntry, CargoAllowError, CargoAllowResult, Requirements, SimpleDate};

/// Maximum lifetime, in days, for generated baseline debt policy entries.
pub const BASELINE_DEBT_MAX_DAYS: i64 = 120;

fn has_real_lifecycle_review(entry: &AllowEntry) -> bool {
    let has_review_after = entry
        .lifecycle
        .review_after
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_expiry = entry
        .lifecycle
        .expires
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty() && value != "never");
    has_review_after || has_expiry
}

pub(crate) fn validate_lifecycle_requirements(
    entry: &AllowEntry,
    requirements: &Requirements,
) -> CargoAllowResult<()> {
    if requirements.expires_or_review_after_required && !has_real_lifecycle_review(entry) {
        return Err(CargoAllowError::new(format!(
            "{} missing expires or review_after",
            entry.id
        )));
    }
    Ok(())
}

pub(crate) fn validate_lifecycle(entry: &AllowEntry) -> CargoAllowResult<()> {
    let created = parse_lifecycle_date(&entry.id, "created", entry.lifecycle.created.as_deref())?;
    let review_after = parse_lifecycle_date(
        &entry.id,
        "review_after",
        entry.lifecycle.review_after.as_deref(),
    )?;
    let expires = parse_expires(&entry.id, entry.lifecycle.expires.as_deref())?;

    if let (Some(created), Some(review_after)) = (created, review_after)
        && created > review_after
    {
        return Err(CargoAllowError::new(format!(
            "{} review_after must not be before created",
            entry.id
        )));
    }
    if let (Some(created), Some(expires)) = (created, expires)
        && created > expires
    {
        return Err(CargoAllowError::new(format!(
            "{} expires must not be before created",
            entry.id
        )));
    }
    if let (Some(review_after), Some(expires)) = (review_after, expires)
        && review_after > expires
    {
        return Err(CargoAllowError::new(format!(
            "{} review_after must not be after expires",
            entry.id
        )));
    }
    if entry.classification == "baseline_debt" {
        let expires = expires.ok_or_else(|| {
            CargoAllowError::new(format!("{} baseline_debt requires expires", entry.id))
        })?;
        // Require 'created' for baseline_debt so the day-range check is
        // deterministic. Using today_utc_approx when created is absent made
        // the gate pass/fail depending on the day cargo-allow was invoked
        // — CI vs local could disagree (#1829).
        let start = created.ok_or_else(|| {
            CargoAllowError::new(format!(
                "{} baseline_debt requires created date for deterministic expiry range check",
                entry.id
            ))
        })?;
        let latest_allowed_expiry = start.add_days(BASELINE_DEBT_MAX_DAYS);
        if expires > latest_allowed_expiry {
            return Err(CargoAllowError::new(format!(
                "{} baseline_debt expires must be within {BASELINE_DEBT_MAX_DAYS} days",
                entry.id
            )));
        }
    }
    Ok(())
}

fn parse_lifecycle_date(
    id: &str,
    field: &str,
    value: Option<&str>,
) -> CargoAllowResult<Option<SimpleDate>> {
    match value {
        Some(value) => SimpleDate::parse(value).map(Some).ok_or_else(|| {
            CargoAllowError::new(format!("{id} has invalid {field} date `{value}`"))
        }),
        None => Ok(None),
    }
}

fn parse_expires(id: &str, value: Option<&str>) -> CargoAllowResult<Option<SimpleDate>> {
    match value {
        Some("never") => Ok(None),
        Some(value) => SimpleDate::parse(value).map(Some).ok_or_else(|| {
            CargoAllowError::new(format!("{id} has invalid expires date `{value}`"))
        }),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{AllowEntry, FindingKind, Lifecycle, Selector};

    fn baseline_debt_entry(id: &str, created: Option<&str>, expires: Option<&str>) -> AllowEntry {
        AllowEntry {
            id: id.to_string(),
            kind: FindingKind::Panic,
            family: None,
            path: Some(std::path::PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "core".to_string(),
            classification: "baseline_debt".to_string(),
            reason: "test".to_string(),
            evidence: vec![],
            links: vec![],
            occurrence_limit: None,
            lifecycle: Lifecycle {
                created: created.map(|s| s.to_string()),
                review_after: None,
                expires: expires.map(|s| s.to_string()),
            },
            selector: Selector::default(),
            last_seen: None,
        }
    }

    #[test]
    fn baseline_debt_without_created_is_rejected() {
        let entry = baseline_debt_entry("allow-1", None, Some("2026-09-01"));
        let err = validate_lifecycle(&entry).unwrap_err();
        assert!(
            err.to_string().contains("requires created date"),
            "should require created for deterministic range check: {err}"
        );
    }

    #[test]
    fn baseline_debt_with_created_passes_range_check() {
        let entry = baseline_debt_entry("allow-1", Some("2026-06-01"), Some("2026-09-01"));
        assert!(validate_lifecycle(&entry).is_ok());
    }

    #[test]
    fn baseline_debt_range_exceeding_max_days_is_rejected() {
        let entry = baseline_debt_entry("allow-1", Some("2026-01-01"), Some("2027-06-01"));
        let err = validate_lifecycle(&entry).unwrap_err();
        assert!(
            err.to_string().contains("must be within 120 days"),
            "should reject range exceeding max: {err}"
        );
    }
}
