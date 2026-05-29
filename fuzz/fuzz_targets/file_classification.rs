#![no_main]

use allow_files::{FileScanOptions, classify_path_with_options, scan_files_with_options};
use libfuzzer_sys::fuzz_target;
use std::path::PathBuf;

const MAX_TEXT: usize = 512;
const MAX_PATTERNS: usize = 8;

fuzz_target!(|data: &[u8]| {
    let mut fields = data.split(|byte| *byte == 0 || *byte == b'\n');
    let path_text = fields
        .next()
        .map(bounded_lossy)
        .unwrap_or_else(|| "src/lib.rs".to_string());
    let generated = fields
        .take(MAX_PATTERNS)
        .map(bounded_lossy)
        .collect::<Vec<_>>();
    let options = FileScanOptions { generated };
    let path = PathBuf::from(path_text);

    if let Some(finding) = classify_path_with_options(&path, &options) {
        let _ = finding.source_package_name();
        let _ = finding.identity.stable_key();
    }

    let files = vec![
        path.clone(),
        PathBuf::from("Cargo.toml"),
        PathBuf::from("src/lib.rs"),
    ];
    for finding in scan_files_with_options(&files, &options) {
        let _ = finding.identity.stable_key();
    }
});

fn bounded_lossy(input: &[u8]) -> String {
    String::from_utf8_lossy(&input[..input.len().min(MAX_TEXT)]).into_owned()
}
