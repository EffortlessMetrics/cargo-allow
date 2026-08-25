use allow_core::FindingKind;
use std::fs;
use std::path::PathBuf;

use super::*;

#[cfg(feature = "syntax")]
mod cache_root_alias;
mod capped_read;
mod finding_builder;
mod indexing;
mod lint;
mod package;
mod panic;
mod parse_error;
mod persistent_scan_cache;
mod proptest_parser;
mod safety_comment_nodes;
mod scope;
mod structural_identity;
mod syntax_coupling;
mod syntax_tree;
mod text;
mod unsafe_scan;

#[test]
fn syntax_panic_columns_are_character_based_after_unicode_prefixes() {
    let line = "    let café = 1; value.unwrap(); panic!(\"boom\");";
    let src = format!("fn load(value: Result<(), ()>) {{\n{line}\n}}\n");
    let findings = scan_rust_source("src/lib.rs", &src);
    let unwrap = findings
        .iter()
        .find(|f| f.kind == FindingKind::Panic && f.family.as_deref() == Some("unwrap"))
        .unwrap_or_else(|| std::panic::panic_any("expected unwrap finding"));
    let panic_macro = findings
        .iter()
        .find(|f| f.kind == FindingKind::Panic && f.family.as_deref() == Some("panic_macro"))
        .unwrap_or_else(|| std::panic::panic_any("expected panic macro finding"));

    assert_eq!(
        unwrap.span.as_ref().map(|span| (span.line, span.column)),
        Some((2, char_column(line, "unwrap")))
    );
    assert_eq!(
        unwrap.identity.column_hint,
        Some(char_column(line, "unwrap"))
    );
    assert_eq!(
        panic_macro
            .span
            .as_ref()
            .map(|span| (span.line, span.column)),
        Some((2, char_column(line, "panic")))
    );
    assert_eq!(
        panic_macro.identity.column_hint,
        Some(char_column(line, "panic"))
    );
}

#[test]
fn syntax_index_columns_are_character_based_after_unicode_prefixes() {
    let line = "    let café = xs[0];";
    let src = format!("fn load(xs: &[u8]) -> u8 {{\n{line}\n}}\n");
    let findings = scan_rust_source("src/lib.rs", &src);
    let indexing = findings
        .iter()
        .find(|f| f.kind == FindingKind::Panic && f.family.as_deref() == Some("indexing"))
        .unwrap_or_else(|| std::panic::panic_any("expected indexing finding"));

    assert_eq!(
        indexing.span.as_ref().map(|span| (span.line, span.column)),
        Some((2, char_column(line, "[")))
    );
    assert_eq!(indexing.identity.column_hint, Some(char_column(line, "[")));
}

#[test]
fn syntax_unsafe_columns_are_character_based_after_unicode_prefixes() {
    let line = "    let café = unsafe { core::ptr::read(ptr) };";
    let src = format!("fn load(ptr: *const u8) -> u8 {{\n{line}\n}}\n");
    let findings = scan_rust_source("src/lib.rs", &src);
    let unsafe_block = findings
        .iter()
        .find(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_block"))
        .unwrap_or_else(|| std::panic::panic_any("expected unsafe block finding"));

    assert_eq!(
        unsafe_block
            .span
            .as_ref()
            .map(|span| (span.line, span.column)),
        Some((2, char_column(line, "unsafe")))
    );
    assert_eq!(
        unsafe_block.identity.column_hint,
        Some(char_column(line, "unsafe"))
    );
}

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|err| std::panic::panic_any(format!("system clock: {err}")))
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-rust-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("temp root: {err}")));
    root
}

fn char_column(line: &str, needle: &str) -> u32 {
    let byte_column = line
        .find(needle)
        .unwrap_or_else(|| std::panic::panic_any(format!("{needle} should exist in {line}")));
    line.char_indices()
        .take_while(|(idx, _)| *idx < byte_column)
        .count() as u32
        + 1
}
