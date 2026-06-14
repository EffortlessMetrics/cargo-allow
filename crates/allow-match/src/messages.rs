use allow_core::{Finding, normalize_path};

pub(crate) fn family_suffix(finding: &Finding) -> String {
    finding
        .family
        .as_ref()
        .map(|f| format!(".{f}"))
        .unwrap_or_default()
}

pub fn finding_location(finding: &Finding) -> String {
    match &finding.span {
        Some(span) => format!(
            "{}:{}:{}",
            normalize_path(&finding.path),
            span.line,
            span.column
        ),
        None => normalize_path(&finding.path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{FindingKind, Span, StructuralIdentity};
    use std::path::PathBuf;

    #[test]
    fn finding_location_includes_normalized_path_line_and_column() {
        let finding = finding_with_span("src\\lib.rs", 42, 7);

        assert_eq!(finding_location(&finding), "src/lib.rs:42:7");
    }

    #[test]
    fn finding_location_without_span_returns_normalized_path_only() {
        let finding = Finding {
            span: None,
            ..finding_with_span("docs\\guide.md", 1, 1)
        };

        assert_eq!(finding_location(&finding), "docs/guide.md");
    }

    fn finding_with_span(path: &str, line: u32, column: u32) -> Finding {
        Finding {
            kind: FindingKind::Unsafe,
            family: Some("unsafe_block".to_string()),
            path: PathBuf::from(path),
            span: Some(Span { line, column }),
            identity: StructuralIdentity::new("rust", "unsafe_block"),
            message: format!("unsafe finding at {path}"),
        }
    }
}
