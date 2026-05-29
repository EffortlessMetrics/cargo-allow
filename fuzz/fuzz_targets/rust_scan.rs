#![no_main]

use allow_rust::{parse_rust_syntax, scan_rust_source};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };

    let findings = scan_rust_source("src/lib.rs", source);
    for finding in &findings {
        if let Some(span) = &finding.span {
            assert!(span.line > 0);
            assert!(span.column > 0);
        }
        assert_eq!(finding.path, std::path::Path::new("src/lib.rs"));
        let _ = finding.identity.stable_key();
    }

    if let Ok(tree) = parse_rust_syntax(source) {
        let _ = tree.root_kind();
        let _ = tree.has_error();
        let _ = tree.named_node_count();
        let _ = tree.containers(source);
    }
});
