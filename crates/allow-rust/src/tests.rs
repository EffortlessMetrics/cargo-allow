use allow_core::FindingKind;

use super::*;

mod indexing;
mod lint;
mod package;
mod panic;
mod syntax_tree;
mod unsafe_scan;

#[test]
fn scan_uses_syntax_container_scope() {
    let src = r#"
        fn actual(value: Result<(), ()>) {
            let text = "fn fake() {";
            value.unwrap();
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let Some(finding) = findings
        .iter()
        .find(|f| f.family.as_deref() == Some("unwrap"))
    else {
        std::panic::panic_any("unwrap finding should exist");
    };

    assert_eq!(finding.identity.container.as_deref(), Some("actual"));
}

#[test]
fn scan_uses_syntax_module_scope() {
    let src = r#"
        mod parser {
            fn parse(value: Result<(), ()>) {
                value.unwrap();
            }
        }

        fn load(value: Result<(), ()>) {
            value.expect("loaded");
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let Some(parser_finding) = findings
        .iter()
        .find(|f| f.family.as_deref() == Some("unwrap"))
    else {
        std::panic::panic_any("parser unwrap finding should exist");
    };
    let Some(root_finding) = findings
        .iter()
        .find(|f| f.family.as_deref() == Some("expect"))
    else {
        std::panic::panic_any("root expect finding should exist");
    };

    assert_eq!(parser_finding.identity.module.as_deref(), Some("parser"));
    assert_eq!(parser_finding.identity.container.as_deref(), Some("parse"));
    assert_eq!(root_finding.identity.module, None);
    assert_eq!(root_finding.identity.container.as_deref(), Some("load"));
}

#[test]
fn scan_uses_syntax_impl_method_scope() {
    let src = r#"
        mod parser {
            struct Parser;

            impl Parser {
                fn parse(&self, value: Result<(), ()>) {
                    value.unwrap();
                }
            }
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let Some(finding) = findings
        .iter()
        .find(|f| f.family.as_deref() == Some("unwrap"))
    else {
        std::panic::panic_any("impl unwrap finding should exist");
    };

    assert_eq!(finding.identity.module.as_deref(), Some("parser"));
    assert_eq!(finding.identity.container.as_deref(), Some("Parser::parse"));
}

#[test]
fn scan_uses_syntax_trait_impl_method_scope() {
    let src = r#"
        trait ParserApi {
            fn parse(&self, value: Result<(), ()>);
        }

        struct Parser;

        impl ParserApi for Parser {
            fn parse(&self, value: Result<(), ()>) {
                value.unwrap();
            }
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let Some(finding) = findings
        .iter()
        .find(|f| f.family.as_deref() == Some("unwrap"))
    else {
        std::panic::panic_any("trait impl unwrap finding should exist");
    };

    assert_eq!(
        finding.identity.container.as_deref(),
        Some("<Parser as ParserApi>::parse")
    );
}

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
