use allow_core::FindingKind;

use crate::package::{source_package_for_path, source_package_name};
use crate::text::{detect_attr, index_symbol, index_target_fingerprint};

use super::*;

#[test]
fn detects_panic_family() {
    let src = r#"
        fn load() {
            let x = std::fs::read_to_string("x").unwrap();
            let y = items[0];
            panic!("bad");
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    assert!(
        findings
            .iter()
            .any(|f| f.family.as_deref() == Some("unwrap"))
    );
    assert!(
        findings
            .iter()
            .any(|f| f.family.as_deref() == Some("indexing"))
    );
    assert!(
        findings
            .iter()
            .any(|f| f.family.as_deref() == Some("panic_macro"))
    );
}

#[test]
fn detects_panic_methods_from_syntax() {
    let src = r#"
        fn load() {
            let x = std::fs::read_to_string("x").unwrap();
            let y = std::fs::read_to_string("y").expect("read y");
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);

    for family in ["unwrap", "expect"] {
        assert!(
            findings.iter().any(|f| f.kind == FindingKind::Panic
                && f.family.as_deref() == Some(family)
                && f.identity.ast_kind == "method_call"),
            "missing {family}"
        );
    }
}

#[test]
fn syntax_panic_methods_record_non_empty_receiver_fingerprint() {
    let src = r#"
        fn load(value: Result<(), ()>) {
            value.expect("loaded");
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let expect = findings
        .iter()
        .find(|f| f.kind == FindingKind::Panic && f.family.as_deref() == Some("expect"))
        .unwrap_or_else(|| std::panic::panic_any("expected expect finding"));

    assert_eq!(
        expect.identity.receiver_fingerprint.as_deref(),
        Some("value")
    );
}

#[test]
fn syntax_panic_methods_record_multiline_receiver_fingerprint() {
    let src = r#"
        fn load() {
            parse_policy(
                "policy = \"cargo-allow\""
            )
            .expect("policy parses");
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let expect = findings
        .iter()
        .find(|f| f.kind == FindingKind::Panic && f.family.as_deref() == Some("expect"))
        .unwrap_or_else(|| std::panic::panic_any("expected expect finding"));

    assert!(
        expect
            .identity
            .receiver_fingerprint
            .as_deref()
            .is_some_and(|receiver| receiver.contains("parse_policy"))
    );
}

#[test]
fn syntax_panic_methods_record_unicode_receiver_fingerprint() {
    let src = r#"
        fn load(é_value: Result<(), ()>) {
            é_value.expect("loaded");
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let expect = findings
        .iter()
        .find(|f| f.kind == FindingKind::Panic && f.family.as_deref() == Some("expect"))
        .unwrap_or_else(|| std::panic::panic_any("expected expect finding"));

    assert_eq!(
        expect.identity.receiver_fingerprint.as_deref(),
        Some("é_value")
    );
}

#[test]
fn syntax_panic_methods_ignore_text_in_strings_and_comments() {
    let src = r#"
        fn load() {
            // value.unwrap();
            let text = "value.expect(\"string\")";
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);

    assert!(
        !findings
            .iter()
            .any(|f| f.kind == FindingKind::Panic && f.identity.ast_kind == "method_call")
    );
}

#[test]
fn scan_rust_files_adds_source_package_context_from_manifest() {
    let root = temp_root("source-package");
    let crate_dir = root.join("crates").join("parser");
    fs::create_dir_all(crate_dir.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("crate dir: {err}")));
    fs::write(
        crate_dir.join("Cargo.toml"),
        "[package]\nname = \"parser\"\nversion = \"0.1.0\"\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("manifest write: {err}")));
    fs::write(
        crate_dir.join("src").join("lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("rust write: {err}")));
    let files = vec![
        PathBuf::from("crates/parser/Cargo.toml"),
        PathBuf::from("crates/parser/src/lib.rs"),
    ];
    assert_eq!(
        source_package_name("[package]\nname = \"parser\"\n"),
        Some("parser".to_string())
    );
    let packages = source_package_contexts(&root, &files)
        .unwrap_or_else(|err| std::panic::panic_any(format!("package contexts: {err}")));
    assert_eq!(
        packages,
        vec![SourcePackageContext {
            root: "crates/parser".to_string(),
            name: "parser".to_string()
        }]
    );
    assert!(source_package_for_path(&files[1], &packages).is_some());

    let findings = scan_rust_files(&root, &files)
        .unwrap_or_else(|err| std::panic::panic_any(format!("scan rust files: {err}")));

    let unwrap = findings
        .iter()
        .find(|finding| finding.family.as_deref() == Some("unwrap"))
        .unwrap_or_else(|| std::panic::panic_any("expected unwrap finding"));
    assert_eq!(unwrap.identity.crate_name.as_deref(), Some("parser"));
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn source_package_context_prefers_nested_manifest() {
    let packages = source_package_contexts_from_sources([
        (
            PathBuf::from("Cargo.toml"),
            "[package]\nname = \"root\"\n".to_string(),
        ),
        (
            PathBuf::from("crates/parser/Cargo.toml"),
            "[package]\nname = \"parser\"\n".to_string(),
        ),
    ]);

    let package = source_package_for_path(Path::new("crates/parser/src/lib.rs"), &packages)
        .unwrap_or_else(|| std::panic::panic_any("expected nested package context"));

    assert_eq!(package.name, "parser");
}

#[test]
fn source_package_context_does_not_match_sibling_prefixes() {
    let packages = source_package_contexts_from_sources([(
        PathBuf::from("crates/parser/Cargo.toml"),
        "[package]\nname = \"parser\"\n".to_string(),
    )]);

    assert!(
        source_package_for_path(Path::new("crates/parser-extra/src/lib.rs"), &packages).is_none()
    );
}

#[test]
fn scan_rust_files_ignores_workspace_manifest_without_package_name() {
    let root = temp_root("workspace-manifest");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("manifest write: {err}")));
    fs::write(
        root.join("src").join("lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("rust write: {err}")));
    let files = vec![PathBuf::from("Cargo.toml"), PathBuf::from("src/lib.rs")];

    let findings = scan_rust_files(&root, &files)
        .unwrap_or_else(|err| std::panic::panic_any(format!("scan rust files: {err}")));

    let unwrap = findings
        .iter()
        .find(|finding| finding.family.as_deref() == Some("unwrap"))
        .unwrap_or_else(|| std::panic::panic_any("expected unwrap finding"));
    assert_eq!(unwrap.identity.crate_name, None);
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn scan_rust_files_ignores_invalid_manifest_source_text() {
    let root = temp_root("invalid-manifest");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    fs::write(root.join("Cargo.toml"), "[package\nname = \"broken\"\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("manifest write: {err}")));
    fs::write(
        root.join("src").join("lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("rust write: {err}")));
    let files = vec![PathBuf::from("Cargo.toml"), PathBuf::from("src/lib.rs")];

    let findings = scan_rust_files(&root, &files)
        .unwrap_or_else(|err| std::panic::panic_any(format!("scan rust files: {err}")));

    let unwrap = findings
        .iter()
        .find(|finding| finding.family.as_deref() == Some("unwrap"))
        .unwrap_or_else(|| std::panic::panic_any("expected unwrap finding"));
    assert_eq!(unwrap.identity.crate_name, None);
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn scan_rust_files_ignores_non_utf8_manifest_source_text() {
    let root = temp_root("non-utf8-manifest");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    fs::write(
        root.join("Cargo.toml"),
        b"[package]\nname = \"broken\"\n\xFF",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("manifest write: {err}")));
    fs::write(
        root.join("src").join("lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("rust write: {err}")));
    let files = vec![PathBuf::from("Cargo.toml"), PathBuf::from("src/lib.rs")];

    let findings = scan_rust_files(&root, &files)
        .unwrap_or_else(|err| std::panic::panic_any(format!("scan rust files: {err}")));

    let unwrap = findings
        .iter()
        .find(|finding| finding.family.as_deref() == Some("unwrap"))
        .unwrap_or_else(|| std::panic::panic_any("expected unwrap finding"));
    assert_eq!(unwrap.identity.crate_name, None);
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn scan_rust_files_ignores_unreadable_manifest_context() {
    let root = temp_root("unreadable-manifest");
    fs::create_dir_all(root.join("Cargo.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("manifest dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("src dir: {err}")));
    fs::write(
        root.join("src").join("lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("rust write: {err}")));
    let files = vec![PathBuf::from("Cargo.toml"), PathBuf::from("src/lib.rs")];

    let findings = scan_rust_files(&root, &files)
        .unwrap_or_else(|err| std::panic::panic_any(format!("scan rust files: {err}")));

    let unwrap = findings
        .iter()
        .find(|finding| finding.family.as_deref() == Some("unwrap"))
        .unwrap_or_else(|| std::panic::panic_any("expected unwrap finding"));
    assert_eq!(unwrap.identity.crate_name, None);
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn syntax_panic_methods_do_not_parse_macro_token_trees() {
    let src = r#"
        fn load(value: Result<(), ()>) {
            assert!(value.unwrap() == ());
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);

    assert!(
        !findings
            .iter()
            .any(|f| f.kind == FindingKind::Panic && f.identity.ast_kind == "method_call")
    );
}

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
fn detects_panic_macros_from_syntax() {
    let src = r#"
        fn load() {
            panic!("bad");
            todo!("later");
            unimplemented!("later");
            unreachable!("bad state");
            std::panic!("scoped");
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);

    for family in ["panic_macro", "todo", "unimplemented", "unreachable"] {
        assert!(
            findings
                .iter()
                .any(|f| f.kind == FindingKind::Panic && f.family.as_deref() == Some(family)),
            "missing {family}"
        );
    }
    assert_eq!(
        findings
            .iter()
            .filter(|f| f.kind == FindingKind::Panic && f.family.as_deref() == Some("panic_macro"))
            .count(),
        2
    );
}

#[test]
fn syntax_panic_macros_ignore_text_in_strings_and_comments() {
    let src = r##"
        fn load() {
            // panic!("comment");
            let text = "todo!(\"string\") unimplemented!(\"string\") unreachable!(\"string\")";
        }
        "##;
    let findings = scan_rust_source("src/lib.rs", src);

    assert!(
        !findings
            .iter()
            .any(|f| f.kind == FindingKind::Panic && f.identity.ast_kind == "macro_call")
    );
}

#[test]
fn detects_unsafe_and_attrs() {
    let src = r#"
        #[allow(clippy::unwrap_used)]
        unsafe fn read() {
            unsafe { core::ptr::read(0 as *const u8); }
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    assert!(
        findings
            .iter()
            .any(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_fn"))
    );
    assert!(
        findings
            .iter()
            .any(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_block"))
    );
    assert!(
        findings
            .iter()
            .any(|f| f.kind == FindingKind::LintException)
    );
}

#[test]
fn detects_unsafe_item_kinds_from_syntax() {
    let src = r#"
        struct Handle;
        unsafe impl Send for Handle {}
        unsafe trait Marker {}
        unsafe extern "C" {
            fn read_handle();
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);

    for family in ["unsafe_impl", "unsafe_trait", "unsafe_extern_block"] {
        assert!(
            findings
                .iter()
                .any(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some(family)),
            "missing {family}"
        );
    }
}

#[test]
fn detects_unsafe_function_signatures_from_syntax() {
    let src = r#"
        trait Reader {
            unsafe fn read();
        }
        extern "C" {
            pub unsafe fn read_handle();
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);

    let unsafe_fn_count = findings
        .iter()
        .filter(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_fn"))
        .count();
    assert_eq!(unsafe_fn_count, 2);
}

#[test]
fn detects_multiple_unsafe_constructs_on_one_line() {
    let src = r#"
        unsafe fn read(ptr: *const u8) -> u8 { unsafe { core::ptr::read(ptr) } }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);

    assert!(
        findings
            .iter()
            .any(|f| { f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_fn") })
    );
    assert!(
        findings.iter().any(|f| {
            f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_block")
        })
    );
}

#[test]
fn detects_repeated_unsafe_blocks_on_one_line() {
    let src = r#"
        fn read(left: *const u8, right: *const u8) { unsafe { core::ptr::read(left); } unsafe { core::ptr::read(right); } }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let unsafe_blocks = findings
        .iter()
        .filter(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_block"))
        .count();

    assert_eq!(unsafe_blocks, 2);
}

#[test]
fn unsafe_findings_record_nearby_safety_comment_metadata() {
    let src = r#"
        fn read(ptr: *const u8) -> u8 {
            // SAFETY: caller provides a valid pointer.
            unsafe { core::ptr::read(ptr) }
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);

    let unsafe_block = findings
        .iter()
        .find(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_block"))
        .unwrap_or_else(|| std::panic::panic_any("unsafe block should be found"));
    assert_eq!(
        unsafe_block.identity.target_fingerprint.as_deref(),
        Some("safety-comment:present")
    );
}

#[test]
fn unsafe_findings_without_safety_comment_have_no_safety_metadata() {
    let src = r#"
        fn read(ptr: *const u8) -> u8 {
            unsafe { core::ptr::read(ptr) }
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);

    let unsafe_block = findings
        .iter()
        .find(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_block"))
        .unwrap_or_else(|| std::panic::panic_any("unsafe block should be found"));
    assert_ne!(
        unsafe_block.identity.target_fingerprint.as_deref(),
        Some("safety-comment:present")
    );
}

#[test]
fn syntax_unsafe_constructs_ignore_text_in_strings() {
    let src = r##"
        /// unsafe fn documented_only();
        fn load() {
            // unsafe { core::ptr::read(ptr) }
            let unsafe_fn = "unsafe fn read() {}";
            let unsafe_block = "unsafe { core::ptr::read(ptr) }";
            let unsafe_impl = "unsafe impl Send for Handle {}";
        }
        "##;
    let findings = scan_rust_source("src/lib.rs", src);

    assert!(!findings.iter().any(|f| f.kind == FindingKind::Unsafe));
}

#[test]
fn detects_unsafe_attribute_from_syntax() {
    let src = r#"
        #[unsafe(no_mangle)]
        fn exported() {}
        "#;
    let findings = scan_rust_source("src/lib.rs", src);

    assert!(
        findings.iter().any(|f| {
            f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_attr")
        })
    );
    assert!(
        !findings
            .iter()
            .any(|f| { f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_fn") })
    );
}

#[test]
fn syntax_unsafe_attributes_ignore_attribute_text_in_strings() {
    let src = r##"
        fn load() {
            let text = "#[unsafe(no_mangle)]";
        }
        "##;
    let findings = scan_rust_source("src/lib.rs", src);

    assert!(
        !findings.iter().any(|f| {
            f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_attr")
        })
    );
}

#[test]
fn syntax_lint_attributes_ignore_attribute_text_in_strings() {
    let src = r##"
        fn load() {
            let text = "#[allow(dead_code)]";
        }
        "##;
    let findings = scan_rust_source("src/lib.rs", src);

    assert!(
        !findings
            .iter()
            .any(|f| f.kind == FindingKind::LintException)
    );
}

#[test]
fn detects_outer_and_inner_lint_attributes_from_syntax() {
    let src = r#"
#![allow(dead_code)]

  #[expect(clippy::unwrap_used, reason = "policy:allow-lint")]
fn load() {}
        "#;
    let findings = scan_rust_source("src/lib.rs", src);

    let allow = findings
        .iter()
        .find(|f| {
            f.kind == FindingKind::LintException && f.family.as_deref() == Some("allow_attribute")
        })
        .unwrap_or_else(|| std::panic::panic_any("inner allow attribute should be found"));
    assert_eq!(allow.identity.lint.as_deref(), Some("dead_code"));

    let expect = findings
        .iter()
        .find(|f| {
            f.kind == FindingKind::LintException && f.family.as_deref() == Some("expect_attribute")
        })
        .unwrap_or_else(|| std::panic::panic_any("outer expect attribute should be found"));
    assert_eq!(expect.identity.lint.as_deref(), Some("clippy::unwrap_used"));
    assert!(
        expect
            .identity
            .symbol
            .as_deref()
            .is_some_and(|symbol| symbol.contains("policy:allow-lint"))
    );
    assert_eq!(
        expect.identity.target_fingerprint.as_deref(),
        Some("policy:allow-lint")
    );
    assert_eq!(expect.span.as_ref().map(|span| span.column), Some(3));
}

#[test]
fn detects_multiline_lint_attribute_policy_reference_from_syntax() {
    let src = r#"
#[expect(
    clippy::unwrap_used,
    reason = "policy:allow-lint"
)]
fn load() {}
        "#;
    let findings = scan_rust_source("src/lib.rs", src);

    let expect = findings
        .iter()
        .find(|f| {
            f.kind == FindingKind::LintException && f.family.as_deref() == Some("expect_attribute")
        })
        .unwrap_or_else(|| std::panic::panic_any("multiline expect attribute should be found"));

    assert_eq!(expect.identity.lint.as_deref(), Some("clippy::unwrap_used"));
    assert!(
        expect
            .identity
            .symbol
            .as_deref()
            .is_some_and(|symbol| symbol.contains("policy:allow-lint"))
    );
    assert_eq!(
        expect.identity.target_fingerprint.as_deref(),
        Some("policy:allow-lint")
    );
}

#[test]
fn detect_attr_returns_text_after_outer_and_inner_prefixes() {
    assert_eq!(
        detect_attr("#[allow(dead_code)]", "allow"),
        Some("dead_code)]")
    );
    assert_eq!(
        detect_attr("#![expect(clippy::unwrap_used)]", "expect"),
        Some("clippy::unwrap_used)]")
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
