use allow_core::{Finding, FindingKind, Span, StructuralIdentity, normalize_path};
use std::path::Path;

use crate::path_rules::lower_extension;

pub(crate) fn build_file_finding(
    path: &Path,
    family: String,
    generated: bool,
    note: Option<&str>,
) -> Finding {
    let mut identity = StructuralIdentity::new("file", "tracked_file");
    identity.symbol = Some(normalize_path(path));
    identity.target_fingerprint = file_fingerprint(path);
    Finding {
        kind: if generated {
            FindingKind::GeneratedCode
        } else {
            FindingKind::NonRustFile
        },
        family: Some(family.clone()),
        path: path.to_path_buf(),
        span: Some(Span { line: 1, column: 1 }),
        identity,
        message: match note {
            Some(note) => format!("tracked non-Rust file classified as {family} ({note})"),
            None => format!("tracked non-Rust file classified as {family}"),
        },
        ledger: None,
    }
}

fn file_fingerprint(path: &Path) -> Option<String> {
    lower_extension(path).or_else(|| {
        let file_name = crate::path_rules::lower_file_name(path);
        (!file_name.is_empty()).then_some(file_name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_file_finding_call_presence_observer() {
        let finding = build_file_finding(
            Path::new(r"docs\Guide.MD"),
            "documentation".to_string(),
            false,
            None,
        );

        assert_eq!(finding.kind, FindingKind::NonRustFile);
        assert_eq!(finding.family.as_deref(), Some("documentation"));
        assert_eq!(finding.path, Path::new(r"docs\Guide.MD"));
        assert_eq!(finding.span, Some(Span { line: 1, column: 1 }));
        assert_eq!(finding.identity.language, "file");
        assert_eq!(finding.identity.ast_kind, "tracked_file");
        assert_eq!(finding.identity.symbol.as_deref(), Some("docs/Guide.MD"));
        assert_eq!(finding.identity.target_fingerprint.as_deref(), Some("md"));
        assert_eq!(
            finding.message,
            "tracked non-Rust file classified as documentation"
        );

        let generated = build_file_finding(
            Path::new("target/generated/bindings.rs"),
            "generated_code".to_string(),
            true,
            None,
        );

        assert_eq!(generated.kind, FindingKind::GeneratedCode);
        assert_eq!(generated.family.as_deref(), Some("generated_code"));
    }

    #[test]
    fn file_fingerprint_call_presence_observer() {
        assert_eq!(
            file_fingerprint(Path::new("tools/Build.PS1")),
            Some("ps1".to_string())
        );
        assert_eq!(
            file_fingerprint(Path::new("config/.ENV")),
            Some(".env".to_string())
        );
        assert_eq!(
            file_fingerprint(Path::new("bin/TOOL")),
            Some("tool".to_string())
        );
        assert_eq!(file_fingerprint(Path::new("")), None);
    }
}
