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
