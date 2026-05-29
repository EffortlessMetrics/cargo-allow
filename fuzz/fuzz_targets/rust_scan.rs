#![no_main]

use allow_report::{
    ReportContext, render_human_with_context, render_json_with_context,
    render_markdown_with_context,
};
use allow_rust::{parse_rust_syntax, scan_rust_source};
use libfuzzer_sys::fuzz_target;
use std::path::Path;

fuzz_target!(|data: &[u8]| {
    let source = String::from_utf8_lossy(data);
    let path = Path::new("fuzz/input.rs");

    // Cover both the tree-sitter syntax wrapper and the line-oriented finding
    // scanner for arbitrary Rust-like text.
    let _ = parse_rust_syntax(&source);
    let findings = scan_rust_source(path, &source);

    // Empty outcomes are enough to exercise report paths that summarize scanned
    // findings without requiring a policy match.
    let context = ReportContext::source_syntax("fuzz", Some("."), Some(1), None);
    let json = render_json_with_context("fuzz", &findings, &[], false, context);
    serde_json::from_str::<serde_json::Value>(&json).expect("report JSON must parse");
    let _ = render_human_with_context("fuzz", &findings, &[], false, context);
    let _ = render_markdown_with_context("fuzz", &findings, &[], false, context);
});
