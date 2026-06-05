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
fn detects_each_lint_in_multi_lint_attribute() {
    let src = r#"
#[allow(dead_code, unused_variables)]
fn load() {}
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let lints = findings
        .iter()
        .filter(|f| {
            f.kind == FindingKind::LintException && f.family.as_deref() == Some("allow_attribute")
        })
        .map(|f| f.identity.lint.as_deref())
        .collect::<Vec<_>>();

    assert_eq!(lints, vec![Some("dead_code"), Some("unused_variables")]);
}

#[test]
fn detects_each_lint_in_multi_lint_expect_attribute_without_reason_metadata() {
    let src = r#"
#[expect(clippy::unwrap_used, clippy::expect_used, reason = "policy:allow-lint")]
fn load() {}
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let lints = findings
        .iter()
        .filter(|f| {
            f.kind == FindingKind::LintException && f.family.as_deref() == Some("expect_attribute")
        })
        .map(|f| {
            (
                f.identity.lint.as_deref(),
                f.identity.target_fingerprint.as_deref(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        lints,
        vec![
            (Some("clippy::unwrap_used"), Some("policy:allow-lint")),
            (Some("clippy::expect_used"), Some("policy:allow-lint"))
        ]
    );
}

#[test]
fn outer_lint_attributes_record_target_function_container() {
    let src = r#"
#[allow(dead_code)]
fn parse() {}

#[allow(dead_code)]
fn render() {}
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let containers = findings
        .iter()
        .filter(|f| {
            f.kind == FindingKind::LintException && f.family.as_deref() == Some("allow_attribute")
        })
        .map(|f| f.identity.container.as_deref())
        .collect::<Vec<_>>();

    assert_eq!(containers, vec![Some("parse"), Some("render")]);
}

#[test]
fn outer_lint_attributes_record_target_impl_method_container() {
    let src = r#"
struct Parser;

impl Parser {
    #[allow(dead_code)]
    fn parse(&self) {}

    #[allow(dead_code)]
    fn render(&self) {}
}
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let containers = findings
        .iter()
        .filter(|f| {
            f.kind == FindingKind::LintException && f.family.as_deref() == Some("allow_attribute")
        })
        .map(|f| f.identity.container.as_deref())
        .collect::<Vec<_>>();

    assert_eq!(
        containers,
        vec![Some("Parser::parse"), Some("Parser::render")]
    );
}

#[test]
fn outer_lint_attributes_record_target_item_container() {
    let src = r#"
#[allow(dead_code)]
struct Parser;

#[allow(dead_code)]
enum Token {}

#[allow(dead_code)]
trait Parse {}

#[allow(dead_code)]
impl Parser {}
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let containers = findings
        .iter()
        .filter(|f| {
            f.kind == FindingKind::LintException && f.family.as_deref() == Some("allow_attribute")
        })
        .map(|f| f.identity.container.as_deref())
        .collect::<Vec<_>>();

    assert_eq!(
        containers,
        vec![Some("Parser"), Some("Token"), Some("Parse"), Some("Parser")]
    );
}

#[test]
fn outer_lint_attributes_record_target_module_scope() {
    let src = r#"
#[allow(dead_code)]
mod parser {
    fn parse() {}
}

#[allow(dead_code)]
mod rendered;
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let scopes = findings
        .iter()
        .filter(|f| {
            f.kind == FindingKind::LintException && f.family.as_deref() == Some("allow_attribute")
        })
        .map(|f| {
            (
                f.identity.module.as_deref(),
                f.identity.container.as_deref(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        scopes,
        vec![(Some("parser"), None), (Some("rendered"), None)]
    );
}

#[test]
fn outer_lint_attributes_record_target_extern_block_container() {
    let src = r#"
#[allow(improper_ctypes)]
extern "C" {
    #[allow(dead_code)]
    fn ffi();
}
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let containers = findings
        .iter()
        .filter(|f| {
            f.kind == FindingKind::LintException && f.family.as_deref() == Some("allow_attribute")
        })
        .map(|f| (f.identity.lint.as_deref(), f.identity.container.as_deref()))
        .collect::<Vec<_>>();

    assert_eq!(
        containers,
        vec![
            (Some("improper_ctypes"), Some("extern \"C\"")),
            (Some("dead_code"), Some("extern \"C\"::ffi"))
        ]
    );
}

#[test]
fn outer_lint_attributes_record_target_use_declaration_container() {
    let src = r#"
#[allow(unused_imports)]
use crate::parser::Parser;

#[allow(unused_imports)]
use crate::render::Renderer;
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let containers = findings
        .iter()
        .filter(|f| {
            f.kind == FindingKind::LintException && f.family.as_deref() == Some("allow_attribute")
        })
        .map(|f| (f.identity.lint.as_deref(), f.identity.container.as_deref()))
        .collect::<Vec<_>>();

    assert_eq!(
        containers,
        vec![
            (Some("unused_imports"), Some("use crate::parser::Parser")),
            (Some("unused_imports"), Some("use crate::render::Renderer"))
        ]
    );
}

#[test]
fn outer_lint_attributes_record_target_macro_definition_container() {
    let src = r#"
#[allow(unused_macros)]
macro_rules! parse_token {
    () => {};
}

#[allow(unused_macros)]
macro_rules! render_token {
    () => {};
}
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let containers = findings
        .iter()
        .filter(|f| {
            f.kind == FindingKind::LintException && f.family.as_deref() == Some("allow_attribute")
        })
        .map(|f| (f.identity.lint.as_deref(), f.identity.container.as_deref()))
        .collect::<Vec<_>>();

    assert_eq!(
        containers,
        vec![
            (Some("unused_macros"), Some("macro_rules! parse_token")),
            (Some("unused_macros"), Some("macro_rules! render_token"))
        ]
    );
}

#[test]
fn detects_spaced_lint_attribute_tokens_from_source_syntax() {
    let outer = r#"  # [ allow(dead_code) ]"#;
    let inner = r#"# ! [ expect(clippy::unwrap_used, reason = "policy:allow-lint") ]"#;
    let src = format!(
        r#"
{inner}
{outer}
fn load() {{}}
        "#
    );
    let findings = scan_rust_source("src/lib.rs", &src);

    let allow = findings
        .iter()
        .find(|f| {
            f.kind == FindingKind::LintException && f.family.as_deref() == Some("allow_attribute")
        })
        .unwrap_or_else(|| std::panic::panic_any("spaced allow attribute should be found"));
    assert_eq!(allow.identity.lint.as_deref(), Some("dead_code"));
    assert_eq!(
        allow.span.as_ref().map(|span| span.column),
        Some(crate::text::column(outer, "allow"))
    );

    let expect = findings
        .iter()
        .find(|f| {
            f.kind == FindingKind::LintException && f.family.as_deref() == Some("expect_attribute")
        })
        .unwrap_or_else(|| std::panic::panic_any("spaced expect attribute should be found"));
    assert_eq!(expect.identity.lint.as_deref(), Some("clippy::unwrap_used"));
    assert_eq!(
        expect.identity.target_fingerprint.as_deref(),
        Some("policy:allow-lint")
    );
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
fn detects_multiple_cfg_attr_lint_attributes() {
    let line = r#"#[cfg_attr(feature = "lint", allow(dead_code), expect(clippy::unwrap_used, reason = "policy:allow-lint"))]"#;
    let src = format!(
        r#"
{line}
fn load() {{}}
        "#
    );
    let findings = scan_rust_source("src/lib.rs", &src);
    let lint_findings = findings
        .iter()
        .filter(|f| f.kind == FindingKind::LintException)
        .map(|f| {
            (
                f.family.as_deref(),
                f.identity.lint.as_deref(),
                f.span.as_ref().map(|span| span.column),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        lint_findings,
        vec![
            (
                Some("allow_attribute"),
                Some("dead_code"),
                Some(crate::text::column(line, "allow"))
            ),
            (
                Some("expect_attribute"),
                Some("clippy::unwrap_used"),
                Some(last_column(line, "expect"))
            )
        ]
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
    assert_eq!(
        detect_attr("allow (dead_code)", "allow"),
        Some("dead_code)")
    );
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
