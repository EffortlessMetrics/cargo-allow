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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_text_accepts_absent_and_valid_values() {
        assert!(validate_optional_text("policy owner", None).is_ok());
        assert!(validate_optional_text("policy owner", Some("repo-infra")).is_ok());
    }

    #[test]
    fn optional_text_rejects_blank_and_whitespace_padded_values() {
        let blank = validate_optional_text("policy owner", Some(" \t "))
            .err()
            .map(|err| err.to_string());
        let padded = validate_optional_text("policy status", Some(" advisory "))
            .err()
            .map(|err| err.to_string());

        assert_eq!(blank.as_deref(), Some("policy owner must not be empty"));
        assert_eq!(
            padded.as_deref(),
            Some("policy status must not have leading or trailing whitespace")
        );
    }

    #[test]
    fn required_text_rejects_empty_before_whitespace_diagnostics() {
        let empty = validate_required_text("allow reason", "")
            .err()
            .map(|err| err.to_string());
        let valid = validate_required_text("allow reason", "Reviewed exception.");

        assert_eq!(empty.as_deref(), Some("allow reason must not be empty"));
        assert!(valid.is_ok());
    }

    #[test]
    fn no_surrounding_whitespace_preserves_interior_whitespace() {
        assert!(validate_no_surrounding_whitespace("allow reason", "two words").is_ok());

        let leading = validate_no_surrounding_whitespace("allow reason", " two words")
            .err()
            .map(|err| err.to_string());
        let trailing = validate_no_surrounding_whitespace("allow reason", "two words ")
            .err()
            .map(|err| err.to_string());

        assert_eq!(
            leading.as_deref(),
            Some("allow reason must not have leading or trailing whitespace")
        );
        assert_eq!(
            trailing.as_deref(),
            Some("allow reason must not have leading or trailing whitespace")
        );
    }
}
