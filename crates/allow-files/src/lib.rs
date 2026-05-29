use allow_core::Finding;
#[cfg(test)]
use allow_core::FindingKind;
use std::path::{Path, PathBuf};

mod allowlist;
mod classifier;
mod family;
mod generated;
mod path_info;

#[derive(Debug, Clone, Default)]
pub struct FileScanOptions {
    pub generated: Vec<String>,
}

pub fn scan_files(files: &[PathBuf]) -> Vec<Finding> {
    scan_files_with_options(files, &FileScanOptions::default())
}

pub fn scan_files_with_options(files: &[PathBuf], options: &FileScanOptions) -> Vec<Finding> {
    classifier::scan_files_with_options(files, options)
}

pub fn classify_path(path: &Path) -> Option<Finding> {
    classify_path_with_options(path, &FileScanOptions::default())
}

pub fn classify_path_with_options(path: &Path, options: &FileScanOptions) -> Option<Finding> {
    classifier::classify_path_with_options(path, options)
}

pub fn is_rust_source(path: &Path) -> bool {
    allowlist::is_rust_source(path)
}

#[cfg(test)]
mod tests;
