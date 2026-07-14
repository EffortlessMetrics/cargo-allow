use super::*;
use allow_core::{Lifecycle, Selector, Span, StructuralIdentity};

pub(super) fn row_status(rows: &[ListRow], id: &str) -> MatchStatus {
    rows.iter()
        .find(|row| row.id == id)
        .map(|row| row.status)
        .unwrap_or_else(|| std::panic::panic_any(format!("missing row {id}")))
}

pub(super) fn list_row(id: &str, kind: FindingKind, owner: &str, classification: &str) -> ListRow {
    ListRow {
        id: id.to_string(),
        status: if classification == "baseline_debt" {
            MatchStatus::BaselineDebt
        } else {
            MatchStatus::Matched
        },
        matches: 1,
        kind,
        family: None,
        owner: owner.to_string(),
        classification: classification.to_string(),
        scope: "src/lib.rs".to_string(),
        source_package: None,
        evidence_count: 0,
        broken_evidence_references: 0,
        weak_evidence_references: 0,
        selector_precision: 0,
        broad_scope: false,
        review_after: "-".to_string(),
        expires: "-".to_string(),
        reason: "reason".to_string(),
    }
}

pub(super) fn test_entry(id: &str, kind: FindingKind) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind,
        family: None,
        path: Some(PathBuf::from("tracked.file")),
        glob: None,
        owner: "owner".to_string(),
        classification: "classification".to_string(),
        reason: "reason".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle::empty(),
        selector: Selector {
            ast_kind: Some("tracked_file".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

pub(super) fn test_finding(
    kind: FindingKind,
    family: Option<&str>,
    path: &str,
    ast_kind: &str,
) -> Finding {
    Finding {
        kind,
        family: family.map(str::to_string),
        path: PathBuf::from(path),
        span: Some(Span { line: 1, column: 1 }),
        identity: StructuralIdentity::new("file", ast_kind),
        message: "test finding".to_string(),
        ledger: None,
    }
}

pub(super) fn test_outcome(
    status: MatchStatus,
    allow_id: Option<&str>,
    finding_index: Option<usize>,
    message: &str,
) -> MatchOutcome {
    MatchOutcome {
        status,
        allow_id: allow_id.map(str::to_string),
        candidate_ids: Vec::new(),
        finding_index,
        message: message.to_string(),
        score: 100,
    }
}
