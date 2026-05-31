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

    if let (Some(created), Some(review_after)) = (created, review_after) {
        if created.days_until(review_after) < 0 {
            return Err(CargoAllowError::new(format!(
                "{} review_after must not be before created",
                entry.id
            )));
        }
    }
    if let (Some(created), Some(expires)) = (created, expires) {
        if created.days_until(expires) < 0 {
            return Err(CargoAllowError::new(format!(
                "{} expires must not be before created",
                entry.id
            )));
        }
    }
    if let (Some(review_after), Some(expires)) = (review_after, expires) {
        if review_after.days_until(expires) < 0 {
            return Err(CargoAllowError::new(format!(
                "{} review_after must not be after expires",
                entry.id
            )));
        }
    }
    if entry.classification == "baseline_debt" {
        let expires = expires.ok_or_else(|| {
            CargoAllowError::new(format!("{} baseline_debt requires expires", entry.id))
        })?;
        let start = created.unwrap_or_else(SimpleDate::today_utc_approx);
        let days = start.days_until(expires);
        if !(0..=BASELINE_DEBT_MAX_DAYS).contains(&days) {
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
