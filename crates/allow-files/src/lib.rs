mod families;
mod finding;
mod options;
mod path_rules;
mod scanner;

pub use options::FileScanOptions;
pub use path_rules::is_rust_source;
pub use scanner::{classify_path, classify_path_with_options, scan_files, scan_files_with_options};

#[cfg(test)]
mod tests;
