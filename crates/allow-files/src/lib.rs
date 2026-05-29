use allow_core::Finding;
use std::path::PathBuf;

mod classify;
mod family;
mod options;
mod path_rules;

pub use classify::{classify_path, classify_path_with_options};
pub use options::FileScanOptions;
pub use path_rules::is_rust_source;

pub fn scan_files(files: &[PathBuf]) -> Vec<Finding> {
    scan_files_with_options(files, &FileScanOptions::default())
}

pub fn scan_files_with_options(files: &[PathBuf], options: &FileScanOptions) -> Vec<Finding> {
    files
        .iter()
        .filter_map(|path| classify_path_with_options(path, options))
        .collect()
}

#[cfg(test)]
mod tests;
