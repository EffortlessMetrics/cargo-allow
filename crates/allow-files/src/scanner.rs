use allow_core::Finding;
use std::path::{Path, PathBuf};

use crate::{
    families::{FileFamilyClassification, file_family},
    finding::build_file_finding,
    options::FileScanOptions,
    path_rules::{is_generated_path, is_scannable_non_rust},
};

pub fn scan_files(files: &[PathBuf]) -> Vec<Finding> {
    scan_files_with_options(files, &FileScanOptions::default())
}

pub fn scan_files_with_options(files: &[PathBuf], options: &FileScanOptions) -> Vec<Finding> {
    files
        .iter()
        .filter_map(|path| classify_path_with_options(path, options))
        .collect()
}

pub fn classify_path(path: &Path) -> Option<Finding> {
    classify_path_with_options(path, &FileScanOptions::default())
}

pub fn classify_path_with_options(path: &Path, options: &FileScanOptions) -> Option<Finding> {
    let classification = classify_file_family_with_options(path, options)?;
    let generated = matches!(&classification, FileFamilyClassification::Generated);
    let family = classification.family().to_string();
    let note = match classification {
        FileFamilyClassification::Custom { rule_id, .. } => Some(format!("rule {rule_id}")),
        FileFamilyClassification::Ambiguous { rule_ids, families } => Some(format!(
            "conflicting rules {} assign families {}",
            rule_ids.join(", "),
            families.join(", ")
        )),
        FileFamilyClassification::Generated | FileFamilyClassification::BuiltIn(_) => None,
    };
    Some(build_file_finding(path, family, generated, note.as_deref()))
}

/// Classify one scannable path using built-in family rules.
pub fn classify_file_family(path: &Path) -> Option<FileFamilyClassification> {
    classify_file_family_with_options(path, &FileScanOptions::default())
}

/// Classify one scannable path using built-in and validated custom rules.
pub fn classify_file_family_with_options(
    path: &Path,
    options: &FileScanOptions,
) -> Option<FileFamilyClassification> {
    let generated = is_generated_path(path, &options.generated);
    if !generated && !is_scannable_non_rust(path) {
        return None;
    }
    Some(file_family(path, generated, &options.file_families))
}
