use crate::family::file_family;
use crate::options::FileScanOptions;
use crate::path_rules::{file_fingerprint, is_builtin_allowed, is_generated_path, is_rust_source};
use allow_core::{Finding, FindingKind, Span, StructuralIdentity, normalize_path};
use std::path::Path;

pub fn classify_path(path: &Path) -> Option<Finding> {
    classify_path_with_options(path, &FileScanOptions::default())
}

pub fn classify_path_with_options(path: &Path, options: &FileScanOptions) -> Option<Finding> {
    if is_rust_source(path) || is_builtin_allowed(path) {
        return None;
    }
    let generated = is_generated_path(path, &options.generated);
    let family = file_family(path, generated);
    let mut identity = StructuralIdentity::new("file", "tracked_file");
    identity.symbol = Some(normalize_path(path));
    identity.target_fingerprint = file_fingerprint(path);
    Some(Finding {
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
    })
}
