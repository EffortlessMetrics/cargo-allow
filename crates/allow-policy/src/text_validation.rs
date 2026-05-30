use allow_core::{CargoAllowError, CargoAllowResult};

pub(crate) fn validate_required_text(label: &str, value: &str) -> CargoAllowResult<()> {
    validate_not_empty(label, value)?;
    validate_no_surrounding_whitespace(label, value)
}

pub(crate) fn validate_optional_text(label: &str, value: Option<&str>) -> CargoAllowResult<()> {
    if let Some(value) = value {
        validate_required_text(label, value)?;
    }
    Ok(())
}

pub(crate) fn validate_no_surrounding_whitespace(label: &str, value: &str) -> CargoAllowResult<()> {
    if value.trim() != value {
        return Err(CargoAllowError::new(format!(
            "{label} must not have leading or trailing whitespace"
        )));
    }
    Ok(())
}

fn validate_not_empty(label: &str, value: &str) -> CargoAllowResult<()> {
    if value.trim().is_empty() {
        return Err(CargoAllowError::new(format!("{label} must not be empty")));
    }
    Ok(())
}
