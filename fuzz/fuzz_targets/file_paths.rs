#![no_main]

use allow_files::{FileScanOptions, classify_path_with_options, scan_files_with_options};
use libfuzzer_sys::fuzz_target;
use std::path::PathBuf;

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);
    let mut chunks = input.split('\0');
    let generated = chunks
        .next()
        .unwrap_or_default()
        .lines()
        .take(16)
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let paths = chunks
        .flat_map(str::lines)
        .take(64)
        .map(PathBuf::from)
        .collect::<Vec<_>>();

    let options = FileScanOptions { generated };
    for path in &paths {
        let _ = classify_path_with_options(path, &options);
    }
    let _ = scan_files_with_options(&paths, &options);
});
