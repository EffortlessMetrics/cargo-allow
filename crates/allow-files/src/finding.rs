use allow_core::{Finding, FindingKind, Span, StructuralIdentity, normalize_path};
use std::path::Path;

use crate::path_rules::lower_extension;

pub(crate) fn build_file_finding(path: &Path, family: String, generated: bool) -> Finding {
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
        message: format!("tracked non-Rust file classified as {family}"),
    }
}

fn file_fingerprint(path: &Path) -> Option<String> {
    lower_extension(path).or_else(|| {
        let file_name = crate::path_rules::lower_file_name(path);
        (!file_name.is_empty()).then_some(file_name)
    })
}
