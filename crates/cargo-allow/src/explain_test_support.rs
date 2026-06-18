use super::*;
use allow_core::{AllowEntry, FindingKind, Lifecycle, Selector, Span, StructuralIdentity};
use std::path::PathBuf;

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
