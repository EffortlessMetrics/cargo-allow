use super::*;
use allow_core::{Span, StructuralIdentity};

pub(super) fn test_finding_at_line(
    kind: FindingKind,
    family: Option<&str>,
    path: &str,
    ast_kind: &str,
    line: u32,
) -> Finding {
    Finding {
        kind,
        family: family.map(str::to_string),
        path: PathBuf::from(path),
        span: Some(Span { line, column: 1 }),
        identity: StructuralIdentity::new("file", ast_kind),
        message: "test finding".to_string(),
        ledger: None,
    }
}
