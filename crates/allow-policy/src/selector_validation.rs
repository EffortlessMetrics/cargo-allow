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

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{FindingKind, LastSeen, Lifecycle, Selector};
    use std::path::PathBuf;

    #[test]
    fn validate_selector_call_presence_observer() {
        let mut source = entry("source", FindingKind::Panic);
        source.selector.ast_kind = Some("method_call".to_string());
        source.selector.callee = Some("unwrap".to_string());
        assert!(validate_selector(&source).is_ok());

        source.selector.normalized_snippet_hash = Some("   ".to_string());
        let blank = validate_selector(&source)
            .expect_err("blank selector identity field should be rejected");
        assert!(
            blank
                .to_string()
                .contains("source selector normalized_snippet_hash must not be empty")
        );

        let mut scope_only_source = entry("scope-only-source", FindingKind::Panic);
        scope_only_source.selector.glob = Some("src/lib.rs".to_string());
        let err = validate_selector(&scope_only_source)
            .expect_err("source-code selector needs structural identity");
        assert!(
            err.to_string().contains(
                "scope-only-source source-code selector must include structural identity"
            )
        );
    }

    #[test]
    fn validate_selector_return_value_discriminator() {
        let mut non_source = entry("non-source", FindingKind::NonRustFile);
        non_source.selector.glob = Some("docs/**".to_string());

        assert!(validate_selector(&non_source).is_ok());
    }

    #[test]
    fn validate_source_hints_call_presence_observer() {
        let mut line_zero = entry("line-zero", FindingKind::Panic);
        line_zero.selector.ast_kind = Some("unsafe_block".to_string());
        line_zero.selector.line_hint = Some(0);
        let err = validate_source_hints(&line_zero).expect_err("zero line_hint should be rejected");
        assert!(
            err.to_string()
                .contains("line-zero line_hint must be greater than zero")
        );

        let mut last_seen_line_zero = entry("last-seen-line-zero", FindingKind::Panic);
        last_seen_line_zero.last_seen = Some(LastSeen { line: 0, column: 1 });
        let err = validate_source_hints(&last_seen_line_zero)
            .expect_err("zero last_seen line should be rejected");
        assert!(
            err.to_string()
                .contains("last-seen-line-zero last_seen line must be greater than zero")
        );

        let mut last_seen_column_zero = entry("last-seen-column-zero", FindingKind::Panic);
        last_seen_column_zero.last_seen = Some(LastSeen { line: 1, column: 0 });
        let err = validate_source_hints(&last_seen_column_zero)
            .expect_err("zero last_seen column should be rejected");
        assert!(
            err.to_string()
                .contains("last-seen-column-zero last_seen column must be greater than zero")
        );
    }

    #[test]
    fn validate_source_hints_return_value_discriminator() {
        let mut hinted = entry("hinted", FindingKind::Unsafe);
        hinted.selector.ast_kind = Some("unsafe_block".to_string());
        hinted.selector.line_hint = Some(12);
        hinted.last_seen = Some(LastSeen {
            line: 34,
            column: 5,
        });

        assert!(validate_source_hints(&hinted).is_ok());
        assert!(validate_source_hints(&entry("empty", FindingKind::NonRustFile)).is_ok());
    }

    fn entry(id: &str, kind: FindingKind) -> AllowEntry {
        AllowEntry {
            id: id.to_string(),
            kind,
            family: None,
            path: Some(PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "policy".to_string(),
            classification: "reviewed".to_string(),
            reason: "fixture".to_string(),
            evidence: Vec::new(),
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle::empty(),
            selector: Selector::default(),
            last_seen: None,
        }
    }
}
