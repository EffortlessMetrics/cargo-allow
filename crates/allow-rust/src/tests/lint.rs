use allow_core::FindingKind;

use crate::scan_rust_source;
use crate::text::detect_attr;

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
fn detects_cfg_attr_lint_attributes_from_source_syntax() {
    let line = r#"  #[cfg_attr(feature = "lint", allow(dead_code))]"#;
    let src = format!(
        r#"
{line}
#[cfg_attr(feature = "lint", expect(clippy::unwrap_used, reason = "policy:allow-lint"))]
fn load() {{}}
        "#
    );
    let findings = scan_rust_source("src/lib.rs", &src);

    let allow = findings
        .iter()
        .find(|f| {
            f.kind == FindingKind::LintException && f.family.as_deref() == Some("allow_attribute")
        })
        .unwrap_or_else(|| std::panic::panic_any("cfg_attr allow attribute should be found"));
    assert_eq!(allow.identity.lint.as_deref(), Some("dead_code"));
    assert_eq!(
        allow.span.as_ref().map(|span| span.column),
        Some(crate::text::column(line, "allow"))
    );

    let expect = findings
        .iter()
        .find(|f| {
            f.kind == FindingKind::LintException && f.family.as_deref() == Some("expect_attribute")
        })
        .unwrap_or_else(|| std::panic::panic_any("cfg_attr expect attribute should be found"));
    assert_eq!(expect.identity.lint.as_deref(), Some("clippy::unwrap_used"));
    assert_eq!(
        expect.identity.target_fingerprint.as_deref(),
        Some("policy:allow-lint")
    );
}

#[test]
fn cfg_attr_lint_detection_ignores_attribute_strings() {
    let src = r##"
#[doc = "example #[allow(dead_code)] text"]
#[cfg_attr(feature = "docs", doc = "allow(dead_code)")]
#[cfg_attr(feature = "docs", doc = r#"expect(clippy::unwrap_used)"#)]
fn load() {}
        "##;
    let findings = scan_rust_source("src/lib.rs", src);

    assert!(
        !findings
            .iter()
            .any(|f| f.kind == FindingKind::LintException)
    );
}

#[test]
fn cfg_attr_lint_detection_ignores_custom_attribute_suffixes() {
    let src = r#"
#[cfg_attr(feature = "custom", my_allow(dead_code))]
#[cfg_attr(feature = "custom", custom::expect(clippy::unwrap_used))]
fn load() {}
        "#;
    let findings = scan_rust_source("src/lib.rs", src);

    assert!(
        !findings
            .iter()
            .any(|f| f.kind == FindingKind::LintException)
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
    assert_eq!(detect_attr("allow(dead_code)", "allow"), Some("dead_code)"));
}
