use allow_core::{AllowEntry, CargoAllowError, CargoAllowResult};

pub(crate) fn validate_selector(entry: &AllowEntry) -> CargoAllowResult<()> {
    let selector = &entry.selector;
    for (field, value) in [
        ("ast_kind", selector.ast_kind.as_deref()),
        ("container", selector.container.as_deref()),
        ("callee", selector.callee.as_deref()),
        ("macro_name", selector.macro_name.as_deref()),
        ("lint", selector.lint.as_deref()),
        ("symbol", selector.symbol.as_deref()),
        (
            "receiver_fingerprint",
            selector.receiver_fingerprint.as_deref(),
        ),
        ("target_fingerprint", selector.target_fingerprint.as_deref()),
        (
            "normalized_snippet_hash",
            selector.normalized_snippet_hash.as_deref(),
        ),
    ] {
        if value.is_some_and(|text| text.trim().is_empty()) {
            return Err(CargoAllowError::new(format!(
                "{} selector {field} must not be empty",
                entry.id
            )));
        }
    }
    let has_identity = selector.ast_kind.is_some()
        || selector.container.is_some()
        || selector.callee.is_some()
        || selector.macro_name.is_some()
        || selector.lint.is_some()
        || selector.symbol.is_some()
        || selector.receiver_fingerprint.is_some()
        || selector.target_fingerprint.is_some()
        || selector.normalized_snippet_hash.is_some()
        || selector.glob.is_some();
    if !has_identity {
        return Err(CargoAllowError::new(format!(
            "{} selector must include structural identity beyond line hints",
            entry.id
        )));
    }
    Ok(())
}

pub(crate) fn validate_source_hints(entry: &AllowEntry) -> CargoAllowResult<()> {
    if entry.selector.line_hint == Some(0) {
        return Err(CargoAllowError::new(format!(
            "{} line_hint must be greater than zero",
            entry.id
        )));
    }
    if let Some(last_seen) = &entry.last_seen {
        if last_seen.line == 0 {
            return Err(CargoAllowError::new(format!(
                "{} last_seen line must be greater than zero",
                entry.id
            )));
        }
        if last_seen.column == 0 {
            return Err(CargoAllowError::new(format!(
                "{} last_seen column must be greater than zero",
                entry.id
            )));
        }
    }
    Ok(())
}
