use allow_core::{AllowEntry, CargoAllowError, CargoAllowResult};

use crate::text_validation::validate_required_text;

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
        if let Some(text) = value {
            validate_required_text(&format!("{} selector {field}", entry.id), text)?;
        }
    }
    let has_structural_identity = selector.has_structural_identity();
    if entry.kind.requires_source_selector_identity() && !has_structural_identity {
        return Err(CargoAllowError::new(format!(
            "{} source-code selector must include structural identity beyond path/glob scope and line hints",
            entry.id
        )));
    }
    if !has_structural_identity && selector.glob.is_none() {
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
