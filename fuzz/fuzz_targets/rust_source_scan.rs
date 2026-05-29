#![no_main]

use allow_rust::{parse_rust_syntax, scan_rust_source};
use libfuzzer_sys::fuzz_target;
use std::path::PathBuf;

const MAX_SOURCE: usize = 128 * 1024;

fuzz_target!(|data: &[u8]| {
    let split_at = data.iter().position(|byte| *byte == 0).unwrap_or(0);
    let (path_bytes, source_bytes) = data.split_at(split_at);
    let source_bytes = source_bytes.strip_prefix(&[0]).unwrap_or(source_bytes);

    let path = if path_bytes.is_empty() {
        PathBuf::from("src/lib.rs")
    } else {
        PathBuf::from(
            String::from_utf8_lossy(&path_bytes[..path_bytes.len().min(256)]).into_owned(),
        )
    };
    let source = String::from_utf8_lossy(&source_bytes[..source_bytes.len().min(MAX_SOURCE)]);

    let findings = scan_rust_source(&path, &source);
    for finding in &findings {
        let _ = finding.identity.stable_key();
    }

    if let Ok(tree) = parse_rust_syntax(&source) {
        let _ = tree.root_kind();
        let _ = tree.has_error();
        let _ = tree.named_node_count();
        for container in tree.containers(&source) {
            let _ = container.module();
        }
    }
});
