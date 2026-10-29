//! #4645: raw identifiers and legal trivia (spaces, newlines, comments)
//! around supported exception syntax must not hide supported findings.
//!
//! Table-driven fixtures scan complete sources through
//! [`scan_rust_source_with_completeness`], require a complete parse, and
//! assert both family and position (line and character column of the written
//! name token) for every expected finding. Negative controls prove strings,
//! raw strings, comments, unrelated identifier suffixes, and unknown names
//! stay unscanned.

use allow_core::FindingKind;

use crate::scan_rust_source_with_completeness;

struct Expected {
    family: &'static str,
    kind: FindingKind,
    line: u32,
    needle: &'static str,
}

fn panic_finding(family: &'static str, line: u32, needle: &'static str) -> Expected {
    Expected {
        family,
        kind: FindingKind::Panic,
        line,
        needle,
    }
}

fn lint_finding(family: &'static str, line: u32, needle: &'static str) -> Expected {
    Expected {
        family,
        kind: FindingKind::LintException,
        line,
        needle,
    }
}

fn unsafe_finding(family: &'static str, line: u32, needle: &'static str) -> Expected {
    Expected {
        family,
        kind: FindingKind::Unsafe,
        line,
        needle,
    }
}

fn spelling_cases() -> Vec<(&'static str, String, Vec<Expected>)> {
    vec![
        // --- panic and assertion macros -----------------------------------
        (
            "direct panic macro control",
            r#"fn p() { panic!("x"); }"#.to_string(),
            vec![panic_finding("panic_macro", 1, "panic")],
        ),
        (
            "raw direct panic macro",
            r#"fn p() { r#panic!("x"); }"#.to_string(),
            vec![panic_finding("panic_macro", 1, "r#panic")],
        ),
        (
            "qualified panic macro control",
            r#"fn p() { std::panic!("x"); }"#.to_string(),
            vec![panic_finding("panic_macro", 1, "panic")],
        ),
        (
            "raw qualified panic macro",
            r#"fn p() { std::r#panic!("x"); }"#.to_string(),
            vec![panic_finding("panic_macro", 1, "r#panic")],
        ),
        (
            "spaced macro path",
            r#"fn p() { std :: panic!("x"); }"#.to_string(),
            vec![panic_finding("panic_macro", 1, "panic")],
        ),
        (
            "commented macro path",
            r#"fn p() { std::/* note */panic!("x"); }"#.to_string(),
            vec![panic_finding("panic_macro", 1, "panic")],
        ),
        (
            "newline macro path",
            "fn p() {\n    std::\n    panic!(\"x\");\n}\n".to_string(),
            vec![panic_finding("panic_macro", 3, "panic")],
        ),
        (
            "raw todo macro",
            r#"fn p() { r#todo!("x"); }"#.to_string(),
            vec![panic_finding("todo", 1, "r#todo")],
        ),
        (
            "raw unimplemented macro",
            r#"fn p() { r#unimplemented!("x"); }"#.to_string(),
            vec![panic_finding("unimplemented", 1, "r#unimplemented")],
        ),
        (
            "raw unreachable macro",
            r#"fn p() { r#unreachable!("x"); }"#.to_string(),
            vec![panic_finding("unreachable", 1, "r#unreachable")],
        ),
        (
            "raw assert macro",
            r#"fn p() { r#assert!(1 < 2); }"#.to_string(),
            vec![panic_finding("assert", 1, "r#assert")],
        ),
        (
            "raw qualified assert_eq macro",
            r#"fn p() { std::r#assert_eq!(1, 1); }"#.to_string(),
            vec![panic_finding("assert_eq", 1, "r#assert_eq")],
        ),
        (
            "raw assert_ne macro",
            r#"fn p() { r#assert_ne!(1, 2); }"#.to_string(),
            vec![panic_finding("assert_ne", 1, "r#assert_ne")],
        ),
        // --- panic methods -------------------------------------------------
        (
            "method unwrap control",
            "fn f(v: Option<u8>) -> u8 { v.unwrap() }".to_string(),
            vec![panic_finding("unwrap", 1, "unwrap")],
        ),
        (
            "raw unwrap method",
            "fn f(v: Option<u8>) -> u8 { v.r#unwrap() }".to_string(),
            vec![panic_finding("unwrap", 1, "r#unwrap")],
        ),
        (
            "raw expect method",
            r#"fn f(v: Option<u8>) -> u8 { v.r#expect("x") }"#.to_string(),
            vec![panic_finding("expect", 1, "r#expect")],
        ),
        (
            "comment between dot and method name",
            "fn f(v: Option<u8>) -> u8 { v./* c */unwrap() }".to_string(),
            vec![panic_finding("unwrap", 1, "unwrap")],
        ),
        (
            "ufcs unwrap control",
            "fn p() { Option::<u8>::unwrap(None); }".to_string(),
            vec![panic_finding("unwrap", 1, "unwrap")],
        ),
        (
            "ufcs raw unwrap",
            "fn p() { Option::<u8>::r#unwrap(None); }".to_string(),
            vec![panic_finding("unwrap", 1, "r#unwrap")],
        ),
        (
            "ufcs commented path",
            "fn p() { Option::<u8>::/* c */unwrap(None); }".to_string(),
            vec![panic_finding("unwrap", 1, "unwrap")],
        ),
        (
            "ufcs trailing turbofish",
            "fn p() { Vec::unwrap::<u8>(None); }".to_string(),
            vec![panic_finding("unwrap", 1, "unwrap")],
        ),
        (
            "ufcs raw trailing turbofish",
            r#"fn p() { Vec::r#expect::<u8>(None); }"#.to_string(),
            vec![panic_finding("expect", 1, "r#expect")],
        ),
        (
            "ufcs angle-bracketed receiver",
            "fn p() { <Option<u8>>::unwrap(None); }".to_string(),
            vec![panic_finding("unwrap", 1, "unwrap")],
        ),
        // --- lint attributes -----------------------------------------------
        (
            "allow attribute control",
            "#[allow(dead_code)]\nfn p() {}\n".to_string(),
            vec![lint_finding("allow_attribute", 1, "#[allow")],
        ),
        (
            "inner allow attribute control",
            "#![allow(dead_code)]\nfn p() {}\n".to_string(),
            vec![lint_finding("allow_attribute", 1, "#![allow")],
        ),
        (
            "raw allow attribute",
            "#[r#allow(dead_code)]\nfn p() {}\n".to_string(),
            vec![lint_finding("allow_attribute", 1, "#[r#allow")],
        ),
        (
            "raw inner allow attribute",
            "#![r#allow(dead_code)]\nfn p() {}\n".to_string(),
            vec![lint_finding("allow_attribute", 1, "#![r#allow")],
        ),
        (
            "comment between allow and parens",
            "#[allow/* note */(dead_code)]\nfn p() {}\n".to_string(),
            vec![lint_finding("allow_attribute", 1, "#[allow")],
        ),
        (
            "expect attribute control",
            "#[expect(dead_code)]\nfn p() {}\n".to_string(),
            vec![lint_finding("expect_attribute", 1, "#[expect")],
        ),
        (
            "raw expect attribute",
            "#[r#expect(dead_code)]\nfn p() {}\n".to_string(),
            vec![lint_finding("expect_attribute", 1, "#[r#expect")],
        ),
        (
            "raw deny attribute",
            "#[r#deny(dead_code)]\nfn p() {}\n".to_string(),
            vec![lint_finding("deny_attribute", 1, "r#deny")],
        ),
        (
            "raw forbid attribute",
            "#[r#forbid(dead_code)]\nfn p() {}\n".to_string(),
            vec![lint_finding("forbid_attribute", 1, "r#forbid")],
        ),
        (
            "raw warn attribute",
            "#[r#warn(dead_code)]\nfn p() {}\n".to_string(),
            vec![lint_finding("warn_attribute", 1, "r#warn")],
        ),
        (
            "cfg_attr allow control",
            "#[cfg_attr(feature = \"x\", allow(dead_code))]\nfn p() {}\n".to_string(),
            vec![lint_finding("allow_attribute", 1, "allow(")],
        ),
        (
            "cfg_attr raw allow",
            "#[cfg_attr(feature = \"x\", r#allow(dead_code))]\nfn p() {}\n".to_string(),
            vec![lint_finding("allow_attribute", 1, "allow(")],
        ),
        (
            "cfg_attr comment between allow and parens",
            "#[cfg_attr(feature = \"x\", allow/* c */(dead_code))]\nfn p() {}\n".to_string(),
            vec![lint_finding("allow_attribute", 1, "allow")],
        ),
        (
            "inner cfg_attr raw expect",
            "#![cfg_attr(feature = \"x\", r#expect(dead_code))]\nfn p() {}\n".to_string(),
            vec![lint_finding("expect_attribute", 1, "expect")],
        ),
        (
            // #4645 review: trivia may displace the written attribute name to
            // a later line; the finding must report the name's own position.
            "multiline commented allow attribute",
            "#[/* note */\nallow(dead_code)]\nfn p() {}\n".to_string(),
            vec![lint_finding("allow_attribute", 2, "allow")],
        ),
        (
            "multiline commented unsafe attribute",
            "#[/* note */\nunsafe(no_mangle)]\nfn p() {}\n".to_string(),
            vec![unsafe_finding("unsafe_attr", 2, "unsafe(")],
        ),
    ]
}

#[test]
fn raw_and_trivia_spellings_produce_supported_findings() {
    for (label, source, expected) in spelling_cases() {
        let scan = scan_rust_source_with_completeness("src/lib.rs", &source);
        assert!(
            !scan.has_parse_error,
            "{label}: fixture must produce a complete parse"
        );
        assert_eq!(
            scan.findings.len(),
            expected.len(),
            "{label}: unexpected finding set {:?}",
            scan.findings
                .iter()
                .map(|finding| {
                    (
                        finding.family.as_deref(),
                        finding.identity.line_hint,
                        finding.identity.column_hint,
                    )
                })
                .collect::<Vec<_>>()
        );
        for want in &expected {
            let finding = scan
                .findings
                .iter()
                .find(|finding| finding.family.as_deref() == Some(want.family))
                .unwrap_or_else(|| {
                    std::panic::panic_any(format!("{label}: missing {} finding", want.family))
                });
            assert_eq!(finding.kind, want.kind, "{label}: {} kind", want.family);
            let column = needle_column(&source, want.line, want.needle);
            assert_eq!(
                finding.span.as_ref().map(|span| (span.line, span.column)),
                Some((want.line, column)),
                "{label}: {} position (needle {:?})",
                want.family,
                want.needle
            );
            assert_eq!(
                finding.identity.column_hint,
                Some(column),
                "{label}: {} column hint (needle {:?})",
                want.family,
                want.needle
            );
        }
    }
}

/// The issue's minimal fixture: all four raw spellings in one source must
/// yield exactly the same four finding families as the ordinary spelling.
#[test]
fn issue_fixture_raw_spellings_match_ordinary_families() {
    let raw = "#[r#allow(dead_code)]\nfn u(value: Option<u8>) -> u8 { value.r#unwrap() }\nfn e(value: Option<u8>) -> u8 { value.r#expect(\"retained\") }\nfn p() { std::r#panic!(\"retained\"); }\n";
    let ordinary = "#[allow(dead_code)]\nfn u(value: Option<u8>) -> u8 { value.unwrap() }\nfn e(value: Option<u8>) -> u8 { value.expect(\"retained\") }\nfn p() { std::panic!(\"retained\"); }\n";

    for (label, source) in [("raw", raw), ("ordinary", ordinary)] {
        let scan = scan_rust_source_with_completeness("src/lib.rs", source);
        assert!(
            !scan.has_parse_error,
            "{label}: fixture must produce a complete parse"
        );
        let mut families = scan
            .findings
            .iter()
            .map(|finding| finding.family.as_deref())
            .collect::<Vec<_>>();
        families.sort_unstable();
        assert_eq!(
            families,
            vec![
                Some("allow_attribute"),
                Some("expect"),
                Some("panic_macro"),
                Some("unwrap"),
            ],
            "{label}: family set"
        );
    }
}

#[test]
fn string_comment_and_unknown_name_controls_stay_unscanned() {
    let sources: Vec<(&'static str, &str)> = vec![
        (
            "string and comment only",
            r#"fn p() { let _ = "r#unwrap std::r#panic!"; } // panic!()"#,
        ),
        (
            "raw string only",
            r##"fn p() { let t = r#"assert_eq!(x) std::r#panic!"#; let _ = t; }"##,
        ),
        (
            "unknown method names",
            r#"fn f(v: Option<u8>) { let _ = v.r#unwrap_or(); let _ = v.r#expect_err("x"); }"#,
        ),
        (
            "unknown macro names",
            "fn p() { debug_assert!(true); std::r#debug_assert!(true); }",
        ),
        (
            "raw unknown attribute name",
            "#[r#allowing(dead_code)]\nfn p() {}\n",
        ),
        (
            "ordinary attribute name suffix",
            "#[allowing(dead_code)]\nfn p() {}\n",
        ),
        (
            "allow-shaped comment only",
            "// #[allow(dead_code)]\n// r#panic!(\"x\")\nfn p() {}\n",
        ),
        (
            // #4645 review: an unqualified generic free call wraps a plain
            // `identifier` callee; a user free function with a panic-method
            // name is not a panic method.
            "unqualified generic free call",
            "fn unwrap<T>(v: T) -> T { v }\nfn p() { unwrap::<u8>(1); }\n",
        ),
        (
            "unqualified free call",
            "fn unwrap(v: u8) -> u8 { v }\nfn p() { unwrap(1); }\n",
        ),
        (
            // #4645 review: comment contents are not source syntax — a
            // commented-out invocation inside a cfg_attr must stay unscanned.
            "cfg_attr commented-out invocation",
            "#[cfg_attr(test, /* allow/* note */(dead_code) */ doc = \"x\")]\nfn p() {}\n",
        ),
        (
            "cfg_attr line-commented invocation",
            "#[cfg_attr(test, // allow(dead_code)\n doc = \"x\")]\nfn p() {}\n",
        ),
    ];
    for (label, source) in sources {
        let scan = scan_rust_source_with_completeness("src/lib.rs", source);
        assert!(
            !scan.has_parse_error,
            "{label}: control must produce a complete parse"
        );
        let relevant = scan
            .findings
            .iter()
            .filter(|finding| {
                matches!(
                    finding.kind,
                    FindingKind::Panic | FindingKind::LintException
                )
            })
            .count();
        assert_eq!(relevant, 0, "{label}: must produce no exception findings");
    }
}

fn needle_column(source: &str, line: u32, needle: &str) -> u32 {
    let line_text = source
        .lines()
        .nth(line as usize - 1)
        .unwrap_or_else(|| std::panic::panic_any(format!("line {line} missing in source")));
    let byte_column = line_text
        .find(needle)
        .unwrap_or_else(|| std::panic::panic_any(format!("{needle:?} missing in {line_text:?}")));
    line_text
        .char_indices()
        .take_while(|(idx, _)| *idx < byte_column)
        .count() as u32
        + 1
}
