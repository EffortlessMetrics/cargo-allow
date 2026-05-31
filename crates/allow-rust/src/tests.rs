use allow_core::FindingKind;

use crate::text::{index_symbol, index_target_fingerprint};

use super::*;

mod lint;
mod package;
mod panic;
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
fn syntax_indexing_ignores_common_bracket_false_positives() {
    let src = [
        "#[allow(dead_code)]",
        "fn load(xs: &[u8]) {",
        "    let literal = [1, 2, 3];",
        "    let nested_type: Vec<[u8; 4]> = Vec::new();",
        "    let macro_vec = vec![1, 2, 3];",
        "    let macro_custom = custom![1, 2, 3];",
        "    let string_literal = \"items[0]\";",
        "    use crate::{alpha, beta};",
        "    let actual = xs[0];",
        "    let call_index = xs.as_ref()[0];",
        "}",
    ]
    .join("\n");
    let findings = scan_rust_source("src/lib.rs", &src);
    let indexing = findings
        .iter()
        .filter(|f| f.family.as_deref() == Some("indexing"))
        .count();

    assert_eq!(indexing, 2);
}

#[test]
fn syntax_indexing_detects_true_positive_shapes() {
    let lb = char::from(91);
    let rb = char::from(93);
    let src = format!(
        r#"
        fn load(xs: &Vec<u8>, matrix: &Vec<Vec<u8>>) {{
            let direct = xs{lb}0{rb};
            let nested = matrix{lb}0{rb}{lb}1{rb};
            let call = xs.as_ref(){lb}0{rb};
        }}
        "#
    );
    let findings = scan_rust_source("src/lib.rs", &src);
    let indexing = findings
        .iter()
        .filter(|f| f.family.as_deref() == Some("indexing"))
        .count();

    assert_eq!(indexing, 3);
}

#[test]
fn syntax_indexing_records_multiline_bracket_span() {
    let src = ["fn load(xs: &[u8]) -> u8 {", "    xs", "        [0]", "}"].join("\n");
    let findings = scan_rust_source("src/lib.rs", &src);
    let indexing = findings
        .iter()
        .find(|f| f.family.as_deref() == Some("indexing"))
        .unwrap_or_else(|| std::panic::panic_any("expected indexing finding"));

    assert_eq!(
        indexing.span.as_ref().map(|span| (span.line, span.column)),
        Some((3, 9))
    );
}

#[test]
fn syntax_indexing_uses_direct_bracket_not_receiver_bracket() {
    let src = [
        "fn load(idx: usize) -> u8 {",
        "    make([1, 2])[idx]",
        "}",
        "fn make(values: [u8; 2]) -> [u8; 2] { values }",
    ]
    .join("\n");
    let findings = scan_rust_source("src/lib.rs", &src);
    let indexing = findings
        .iter()
        .find(|f| f.family.as_deref() == Some("indexing"))
        .unwrap_or_else(|| std::panic::panic_any("expected indexing finding"));

    assert_eq!(
        indexing.span.as_ref().map(|span| (span.line, span.column)),
        Some((2, 17))
    );
}

#[test]
fn index_symbol_truncates_on_character_boundaries() {
    let line = format!("let actual = values[{}];", "\u{00e9}".repeat(120));

    assert_eq!(index_symbol(&line).chars().count(), 100);
}

#[test]
fn index_target_fingerprint_truncates_on_character_boundaries() {
    let line = format!("let actual = {}[0];", "\u{00e9}".repeat(60));
    let fingerprint = index_target_fingerprint(&line);

    assert_eq!(fingerprint.as_ref().map(|s| s.chars().count()), Some(40));
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

#[test]
fn parser_foundation_parses_valid_rust() {
    let tree = parse_rust_syntax("fn load() { let value = 1; }")
        .unwrap_or_else(|err| std::panic::panic_any(format!("parser should load: {err}")));

    assert_eq!(tree.root_kind(), "source_file");
    assert!(!tree.has_error());
    assert!(tree.named_node_count() > 1);
}

#[test]
fn parser_foundation_reports_invalid_rust_without_compilation() {
    let tree = parse_rust_syntax("fn broken( { let value = ;")
        .unwrap_or_else(|err| std::panic::panic_any(format!("parser should load: {err}")));

    assert_eq!(tree.root_kind(), "source_file");
    assert!(tree.has_error());
    assert!(tree.named_node_count() > 0);
}

#[test]
fn syntax_containers_include_nested_module_functions() {
    let source = r#"
        mod parser {
            pub fn parse_span() {}
            mod inner {
                fn normalize_span() {}
            }
        }
        "#;
    let tree = parse_rust_syntax(source)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parser should load: {err}")));
    let containers = tree.containers(source);

    let parse_span = containers
        .iter()
        .find(|container| container.name == "parse_span")
        .unwrap_or_else(|| std::panic::panic_any("parse_span container should exist"));
    assert_eq!(parse_span.kind, "function");
    assert_eq!(parse_span.module().as_deref(), Some("parser"));
    assert!(parse_span.start_line > 0);
    assert!(parse_span.end_line >= parse_span.start_line);

    let normalize_span = containers
        .iter()
        .find(|container| container.name == "normalize_span")
        .unwrap_or_else(|| std::panic::panic_any("normalize_span container should exist"));
    assert_eq!(normalize_span.module().as_deref(), Some("parser::inner"));
}

#[test]
fn syntax_containers_include_inherent_impl_methods() {
    let source = r#"
        mod parser {
            struct Parser;

            impl Parser {
                fn parse_span(&self) {}
            }
        }
        "#;
    let tree = parse_rust_syntax(source)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parser should load: {err}")));
    let containers = tree.containers(source);

    let method = containers
        .iter()
        .find(|container| container.name == "Parser::parse_span")
        .unwrap_or_else(|| std::panic::panic_any("Parser::parse_span should exist"));
    assert_eq!(method.kind, "method");
    assert_eq!(method.module().as_deref(), Some("parser"));
    assert!(method.start_line > 0);
    assert!(method.end_line >= method.start_line);
}

#[test]
fn syntax_containers_include_trait_impl_methods() {
    let source = r#"
        trait ParserApi {
            fn parse_span(&self);
        }

        struct Parser;

        impl ParserApi for Parser {
            fn parse_span(&self) {}
        }
        "#;
    let tree = parse_rust_syntax(source)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parser should load: {err}")));
    let containers = tree.containers(source);

    let method = containers
        .iter()
        .find(|container| container.name == "<Parser as ParserApi>::parse_span")
        .unwrap_or_else(|| std::panic::panic_any("<Parser as ParserApi>::parse_span should exist"));
    assert_eq!(method.kind, "method");
    assert_eq!(method.module(), None);
}

#[test]
fn syntax_containers_recover_from_invalid_source() {
    let source = r#"
        fn parsed_before_error() {}
        fn broken( {
        "#;
    let tree = parse_rust_syntax(source)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parser should load: {err}")));
    let containers = tree.containers(source);

    assert!(tree.has_error());
    assert!(
        containers
            .iter()
            .any(|container| container.name == "parsed_before_error")
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
