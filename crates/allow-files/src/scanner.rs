use allow_core::Finding;
use std::path::{Path, PathBuf};

use crate::{
    families::file_family,
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
    let generated = is_generated_path(path, &options.generated);
    if !generated && !is_scannable_non_rust(path) {
        return None;
    }
    let family = file_family(path, generated);
    Some(build_file_finding(path, family, generated))
}
