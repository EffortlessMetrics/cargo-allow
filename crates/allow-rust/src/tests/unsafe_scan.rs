use allow_core::FindingKind;

use crate::scan_rust_source;

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
fn unsafe_trait_findings_record_trait_symbol() {
    let src = r#"
        unsafe trait Marker {}
        "#;
    let findings = scan_rust_source("src/lib.rs", src);

    let unsafe_trait = findings
        .iter()
        .find(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_trait"))
        .unwrap_or_else(|| std::panic::panic_any("unsafe trait should be found"));

    assert_eq!(unsafe_trait.identity.symbol.as_deref(), Some("Marker"));
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
fn unsafe_function_findings_record_function_symbol() {
    let src = r#"
        unsafe fn read(ptr: *const u8) -> u8 {
            unsafe { core::ptr::read(ptr) }
        }

        trait Reader {
            unsafe fn read_trait();
        }

        extern "C" {
            pub unsafe fn read_handle();
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);

    let symbols = findings
        .iter()
        .filter(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_fn"))
        .map(|f| f.identity.symbol.as_deref())
        .collect::<Vec<_>>();

    assert_eq!(
        symbols,
        vec![Some("read"), Some("read_trait"), Some("read_handle")]
    );
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
fn detects_spaced_unsafe_attribute_tokens_from_source_syntax() {
    let line = r#"        # [ unsafe(no_mangle) ]"#;
    let src = format!(
        r#"
{line}
        fn exported() {{}}
        "#
    );
    let findings = scan_rust_source("src/lib.rs", &src);
    let unsafe_attr = findings
        .iter()
        .find(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_attr"))
        .unwrap_or_else(|| std::panic::panic_any("spaced unsafe attribute should be found"));

    assert_eq!(
        unsafe_attr.span.as_ref().map(|span| span.column),
        Some(crate::text::column(line, "unsafe"))
    );
}

#[test]
fn detects_cfg_attr_unsafe_attribute_from_source_syntax() {
    let line = r#"        #[cfg_attr(feature = "ffi", unsafe(no_mangle))]"#;
    let src = format!(
        r#"
{line}
        fn exported() {{}}
        "#
    );
    let findings = scan_rust_source("src/lib.rs", &src);
    let unsafe_attr = findings
        .iter()
        .find(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_attr"))
        .unwrap_or_else(|| std::panic::panic_any("expected cfg_attr unsafe attribute finding"));

    assert_eq!(
        unsafe_attr.span.as_ref().map(|span| span.column),
        Some(crate::text::column(line, "unsafe"))
    );
}

#[test]
fn cfg_attr_unsafe_attribute_column_ignores_quoted_unsafe_text() {
    let line =
        r#"        #[cfg_attr(feature = "ffi", doc = "unsafe(no_mangle)", unsafe(no_mangle))]"#;
    let src = format!(
        r#"
{line}
        fn exported() {{}}
        "#
    );
    let findings = scan_rust_source("src/lib.rs", &src);
    let unsafe_attr = findings
        .iter()
        .find(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_attr"))
        .unwrap_or_else(|| std::panic::panic_any("expected cfg_attr unsafe attribute finding"));

    assert_eq!(
        unsafe_attr.span.as_ref().map(|span| span.column),
        Some(last_column(line, "unsafe"))
    );
}

#[test]
fn detects_multiple_cfg_attr_unsafe_attributes() {
    let line = r#"        #[cfg_attr(feature = "ffi", unsafe(no_mangle), unsafe(export_name = "fixture"))]"#;
    let src = format!(
        r#"
{line}
        fn exported() {{}}
        "#
    );
    let findings = scan_rust_source("src/lib.rs", &src);
    let columns = findings
        .iter()
        .filter(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_attr"))
        .map(|f| f.span.as_ref().map(|span| span.column))
        .collect::<Vec<_>>();

    assert_eq!(
        columns,
        vec![
            Some(crate::text::column(line, "unsafe")),
            Some(last_column(line, "unsafe"))
        ]
    );
}

#[test]
fn detects_multiple_unsafe_attributes_on_one_line() {
    let src = r#"
        #[unsafe(no_mangle)] #[unsafe(export_name = "fixture")]
        fn exported() {}
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let unsafe_attrs = findings
        .iter()
        .filter(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_attr"))
        .count();

    assert_eq!(unsafe_attrs, 2);
}

fn last_column(line: &str, needle: &str) -> u32 {
    let index = line
        .rfind(needle)
        .unwrap_or_else(|| std::panic::panic_any(format!("missing `{needle}` in `{line}`")));
    line.char_indices()
        .take_while(|(byte, _)| *byte < index)
        .count() as u32
        + 1
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
fn syntax_unsafe_attributes_ignore_unsafe_text_inside_attribute_strings() {
    let src = r##"
        #[doc = "example #[unsafe(no_mangle)] text"]
        #[cfg_attr(feature = "docs", doc = "unsafe(no_mangle)")]
        #[cfg_attr(feature = "docs", doc = r#"unsafe(export_name = "fixture")"#)]
        fn exported() {}
        "##;
    let findings = scan_rust_source("src/lib.rs", src);

    assert!(
        !findings.iter().any(|f| {
            f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_attr")
        })
    );
}

#[test]
fn cfg_attr_unsafe_detection_ignores_custom_attribute_suffixes() {
    let src = r#"
        #[cfg_attr(feature = "custom", my_unsafe(no_mangle))]
        #[cfg_attr(feature = "custom", custom::unsafe(no_mangle))]
        fn exported() {}
        "#;
    let findings = scan_rust_source("src/lib.rs", src);

    assert!(
        !findings.iter().any(|f| {
            f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_attr")
        })
    );
}
