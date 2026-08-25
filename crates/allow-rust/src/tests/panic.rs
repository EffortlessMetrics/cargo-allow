use allow_core::FindingKind;

use crate::scan_rust_source;

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
        Some("param:0")
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
fn syntax_panic_methods_record_nested_receiver_fingerprint() {
    let src = r#"
        fn load(builder: Builder, fallback: Loader) {
            builder.step().unwrap();
            fallback.source().expect("loaded");
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let receivers = findings
        .iter()
        .filter(|f| f.kind == FindingKind::Panic && f.identity.ast_kind == "method_call")
        .map(|f| {
            (
                f.family.as_deref(),
                f.identity.receiver_fingerprint.as_deref(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        receivers,
        vec![
            (Some("unwrap"), Some("builder.step()")),
            (Some("expect"), Some("fallback.source()")),
        ]
    );
}

#[test]
fn syntax_panic_methods_record_unicode_receiver_fingerprint() {
    let name = "\u{00e9}_value";
    let src = format!(
        r#"
        fn load({name}: Result<(), ()>) {{
            {name}.expect("loaded");
        }}
        "#
    );
    let findings = scan_rust_source("src/lib.rs", &src);
    let expect = findings
        .iter()
        .find(|f| f.kind == FindingKind::Panic && f.family.as_deref() == Some("expect"))
        .unwrap_or_else(|| std::panic::panic_any("expected expect finding"));

    assert_eq!(
        expect.identity.receiver_fingerprint.as_deref(),
        Some("param:0")
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
fn detects_assertion_macros_with_distinct_families_and_paths() {
    let src = r#"
        fn validate(left: i32, right: i32) {
            assert!(left < right);
            assert_eq!(left, right);
            std::assert_ne!(left, right);
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let assertion_findings = findings
        .iter()
        .filter(|finding| {
            finding.kind == FindingKind::Panic
                && ["assert", "assert_eq", "assert_ne"]
                    .contains(&finding.family.as_deref().unwrap_or_default())
        })
        .map(|finding| {
            (
                finding.family.as_deref(),
                finding.identity.ast_kind.as_str(),
                finding.identity.macro_name.as_deref(),
                finding.identity.target_fingerprint.as_deref(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        assertion_findings,
        vec![
            (Some("assert"), "macro_call", Some("assert"), Some("assert")),
            (
                Some("assert_eq"),
                "macro_call",
                Some("assert_eq"),
                Some("assert_eq"),
            ),
            (
                Some("assert_ne"),
                "macro_call",
                Some("assert_ne"),
                Some("std::assert_ne"),
            ),
        ]
    );
}

#[test]
fn assertion_macro_names_in_strings_and_comments_are_not_findings() {
    let src = r##"
        fn text_only() {
            // assert!(comment)
            let text = r#"assert_eq!(string) assert_ne!(string)"#;
            let other = "assert!(string)";
            let _ = (text, other);
        }
        "##;
    let findings = scan_rust_source("src/lib.rs", src);

    assert!(!findings.iter().any(|finding| {
        finding.kind == FindingKind::Panic
            && ["assert", "assert_eq", "assert_ne"]
                .contains(&finding.family.as_deref().unwrap_or_default())
    }));
}

#[test]
fn syntax_panic_macros_record_visible_macro_path() {
    let src = r#"
        fn load() {
            panic!("bad");
            std::panic!("scoped");
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let macro_paths = findings
        .iter()
        .filter(|f| f.kind == FindingKind::Panic && f.family.as_deref() == Some("panic_macro"))
        .map(|f| f.identity.target_fingerprint.as_deref())
        .collect::<Vec<_>>();

    assert_eq!(macro_paths, vec![Some("panic"), Some("std::panic")]);
}

#[test]
fn syntax_panic_macros_record_visible_macro_path_for_full_family() {
    let src = r#"
        fn load() {
            std::panic!("bad");
            crate::todo!("later");
            core::unimplemented!("later");
            alloc::unreachable!("bad state");
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let macro_paths = findings
        .iter()
        .filter(|f| f.kind == FindingKind::Panic && f.identity.ast_kind == "macro_call")
        .map(|f| {
            (
                f.family.as_deref(),
                f.identity.macro_name.as_deref(),
                f.identity.target_fingerprint.as_deref(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        macro_paths,
        vec![
            (Some("panic_macro"), Some("panic"), Some("std::panic")),
            (Some("todo"), Some("todo"), Some("crate::todo")),
            (
                Some("unimplemented"),
                Some("unimplemented"),
                Some("core::unimplemented")
            ),
            (
                Some("unreachable"),
                Some("unreachable"),
                Some("alloc::unreachable")
            ),
        ]
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
