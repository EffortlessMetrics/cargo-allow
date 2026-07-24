use allow_core::{AllowEntry, Finding};

/// Maximum line-distance that is NOT considered drift. A finding that shifted
/// by this many lines or fewer (e.g. from an edit further up the file) does
/// not fire `LocationDrift`. Set to 0 to flag any movement (#1808).
pub(crate) const DRIFT_LINE_TOLERANCE: u32 = 3;

pub(crate) fn last_seen_drift_message(entry: &AllowEntry, finding: &Finding) -> Option<String> {
    let last_seen = entry.last_seen.as_ref()?;
    let span = finding.span.as_ref()?;
    let line_delta = last_seen.line.abs_diff(span.line);
    // Column-only changes always drift (they suggest the finding itself
    // changed, not that surrounding code shifted). Line-only shifts within
    // the tolerance are suppressed (#1808).
    let drifted = if last_seen.line == span.line {
        last_seen.column != span.column
    } else {
        line_delta > DRIFT_LINE_TOLERANCE
    };
    if !drifted {
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
            ledger: None,
        }
    }

    #[test]
    fn last_seen_drift_message_reports_line_and_column_movement() {
        let message = last_seen_drift_message(&entry_with_last_seen(7, 12), &finding_at(42, 5))
            .unwrap_or_else(|| std::panic::panic_any("expected drift message"));

        assert_eq!(message, "allow-drift last_seen changed from 7:12 to 42:5");
    }

    #[test]
    fn last_seen_drift_message_absent_when_coordinates_match() {
        assert!(
            last_seen_drift_message(&entry_with_last_seen(7, 12), &finding_at(7, 12)).is_none()
        );
    }

    #[test]
    fn last_seen_drift_message_absent_within_line_tolerance() {
        // #1808: a shift of ≤DRIFT_LINE_TOLERANCE lines does not fire drift.
        for delta in 1..=DRIFT_LINE_TOLERANCE {
            assert!(
                last_seen_drift_message(&entry_with_last_seen(10, 5), &finding_at(10 + delta, 5))
                    .is_none(),
                "shift of {delta} lines should be within tolerance"
            );
            // Also downward shifts.
            assert!(
                last_seen_drift_message(&entry_with_last_seen(10 + delta, 5), &finding_at(10, 5))
                    .is_none(),
                "downward shift of {delta} lines should be within tolerance"
            );
        }
    }

    #[test]
    fn last_seen_drift_message_fires_beyond_line_tolerance() {
        // A shift beyond the tolerance fires drift.
        let delta = DRIFT_LINE_TOLERANCE + 1;
        assert!(
            last_seen_drift_message(&entry_with_last_seen(10, 5), &finding_at(10 + delta, 5))
                .is_some(),
            "shift of {delta} lines should fire drift"
        );
    }

    #[test]
    fn last_seen_drift_message_fires_on_column_only_change() {
        // A column change on the same line always fires drift — it suggests
        // the finding itself moved, not that surrounding code shifted.
        assert!(
            last_seen_drift_message(&entry_with_last_seen(10, 5), &finding_at(10, 12)).is_some(),
            "column-only change should fire drift"
        );
    }
}
