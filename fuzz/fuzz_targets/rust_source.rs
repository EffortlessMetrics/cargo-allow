#![no_main]

use libfuzzer_sys::fuzz_target;
use std::path::Path;

fuzz_target!(|data: &[u8]| {
    let source = String::from_utf8_lossy(data);
    let _ = allow_rust::parse_rust_syntax(&source);
    let _ = allow_rust::scan_rust_source(Path::new("fuzz.rs"), &source);
});
