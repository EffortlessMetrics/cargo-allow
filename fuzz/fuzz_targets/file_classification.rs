#![no_main]

use allow_files::{
    FileScanOptions, classify_path, classify_path_with_options, scan_files_with_options,
};
use libfuzzer_sys::fuzz_target;
use std::path::PathBuf;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    let paths = input
        .split('\0')
        .take(8)
        .map(|part| {
            if part.is_empty() {
                PathBuf::from("file.txt")
            } else {
                PathBuf::from(part)
            }
        })
        .collect::<Vec<_>>();
    let patterns = input
        .lines()
        .take(4)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let options = FileScanOptions {
        generated: patterns,
    };

    for path in &paths {
        let baseline = classify_path(path);
        let configured = classify_path_with_options(path, &options);
        for finding in baseline.iter().chain(configured.iter()) {
            assert_eq!(finding.path, *path);
            assert!(finding.span.is_some());
            let _ = finding.identity.stable_key();
        }
    }

    let scanned = scan_files_with_options(&paths, &options);
    assert!(scanned.len() <= paths.len());
});
