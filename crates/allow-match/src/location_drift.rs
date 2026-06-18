use allow_core::{AllowEntry, Finding};

pub(crate) fn last_seen_drift_message(entry: &AllowEntry, finding: &Finding) -> Option<String> {
    let last_seen = entry.last_seen.as_ref()?;
    let span = finding.span.as_ref()?;
    if last_seen.line == span.line && last_seen.column == span.column {
        return None;
    }
    Some(format!(
        "{} last_seen changed from {}:{} to {}:{}",
        entry.id, last_seen.line, last_seen.column, span.line, span.column
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{
        AllowEntry, Finding, FindingKind, LastSeen, Lifecycle, Selector, Span, StructuralIdentity,
    };
    use std::path::PathBuf;

    fn entry_with_last_seen(line: u32, column: u32) -> AllowEntry {
        AllowEntry {
            id: "allow-drift".to_string(),
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: Some(PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "parser".to_string(),
            classification: "reviewed_exception".to_string(),
            reason: "fixture".to_string(),
            evidence: vec!["test:fixture".to_string()],
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle::empty(),
            selector: Selector::default(),
            last_seen: Some(LastSeen { line, column }),
        }
    }

    fn finding_at(line: u32, column: u32) -> Finding {
        Finding {
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: PathBuf::from("src/lib.rs"),
            span: Some(Span { line, column }),
            identity: StructuralIdentity::new("rust", "method_call"),
            message: String::new(),
        }
    }

    #[test]
    fn last_seen_drift_message_reports_line_and_column_movement() {
        let message = last_seen_drift_message(&entry_with_last_seen(7, 12), &finding_at(42, 5))
            .unwrap_or_else(|| std::panic::panic_any("expected drift message"));

        assert_eq!(
            message,
            "allow-drift last_seen changed from 7:12 to 42:5"
        );
    }

    #[test]
    fn last_seen_drift_message_absent_when_coordinates_match() {
        assert!(last_seen_drift_message(&entry_with_last_seen(7, 12), &finding_at(7, 12)).is_none());
    }
}
