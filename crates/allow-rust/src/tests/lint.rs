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
fn same_line_lint_attributes_record_target_function_container() {
    let src = r#"
#[allow(dead_code)] fn parse() {}
#[allow(dead_code)] fn render() {}
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
fn cfg_attr_lint_attributes_record_target_function_container() {
    let src = r#"
#[cfg_attr(feature = "lint", allow(dead_code))]
fn parse() {}

#[cfg_attr(feature = "lint", allow(dead_code))]
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
fn same_lint_and_policy_on_different_items_differs_by_container() {
    let shared = r#"#[expect(dead_code, reason = "policy:allow-shared: structural identity shared policy")]"#;
    let src = format!(
        r#"
{shared}
fn parse() {{}}

{shared}
fn render() {{}}
        "#
    );
    let findings = scan_rust_source("src/lib.rs", &src);
    let lint_findings = findings
        .iter()
        .filter(|f| f.kind == FindingKind::LintException)
        .map(|f| {
            (
                f.identity.container.as_deref(),
                f.identity.lint.as_deref(),
                f.identity.target_fingerprint.as_deref(),
                f.identity.stable_key(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(lint_findings.len(), 2);
    assert_eq!(lint_findings[0].1, Some("dead_code"));
    assert_eq!(lint_findings[1].1, Some("dead_code"));
    assert_eq!(
        lint_findings[0].2,
        Some("policy:allow-shared"),
        "shared policy id should still populate target_fingerprint"
    );
    assert_eq!(lint_findings[0].0, Some("parse"));
    assert_eq!(lint_findings[1].0, Some("render"));
    assert_ne!(lint_findings[0].3, lint_findings[1].3);
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
fn outer_lint_attributes_record_target_associated_item_container() {
    let src = r#"
struct Parser;

impl Parser {
    #[allow(dead_code)]
    const KIND: u8 = 0;
}

trait ParserApi {
    #[allow(dead_code)]
    type Output;
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
        vec![Some("Parser::KIND"), Some("ParserApi::Output")]
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
fn outer_lint_attributes_record_target_enum_variant_container() {
    let src = r#"
enum Token {
    #[allow(non_camel_case_types)]
    legacy,

    #[allow(non_camel_case_types)]
    rendered,
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
            (Some("non_camel_case_types"), Some("Token::legacy")),
            (Some("non_camel_case_types"), Some("Token::rendered"))
        ]
    );
}

#[test]
fn outer_lint_attributes_record_target_enum_variant_field_container() {
    let src = r#"
enum Token {
    Legacy {
        #[allow(dead_code)]
        raw: String,
    },
    Rendered {
        #[allow(dead_code)]
        raw: String,
    },
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
            (Some("dead_code"), Some("Token::Legacy::raw")),
            (Some("dead_code"), Some("Token::Rendered::raw"))
        ]
    );
}

#[test]
fn outer_lint_attributes_record_target_struct_field_container() {
    let src = r#"
struct Parser {
    #[allow(dead_code)]
    legacy: String,

    #[allow(dead_code)]
    rendered: String,
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
            (Some("dead_code"), Some("Parser::legacy")),
            (Some("dead_code"), Some("Parser::rendered"))
        ]
    );
}

#[test]
fn inner_impl_lint_attributes_record_target_container() {
    let src = r#"
struct Parser;

impl Parser {
    #![allow(dead_code)]
    fn parse() {}
}

struct Renderer;

impl Renderer {
    #![allow(dead_code)]
    fn render() {}
}
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let keyed = findings
        .iter()
        .filter(|f| f.kind == FindingKind::LintException)
        .map(|f| (f.identity.container.as_deref(), f.identity.stable_key()))
        .collect::<Vec<_>>();

    assert_eq!(keyed.len(), 2);
    assert_eq!(keyed[0].0, Some("Parser"));
    assert_eq!(keyed[1].0, Some("Renderer"));
    assert_ne!(keyed[0].1, keyed[1].1);
}

#[test]
fn inner_module_lint_attributes_record_target_module_scope() {
    let src = r#"
mod parser {
    #![allow(dead_code)]
}

mod render {
    #![allow(dead_code)]
}
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
                f.identity.stable_key(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(scopes.len(), 2);
    assert_eq!(scopes[0].0, Some("parser"));
    assert_eq!(scopes[1].0, Some("render"));
    assert_ne!(scopes[0].2, scopes[1].2);
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
    assert_eq!(expect.identity.container.as_deref(), Some("load"));
}

#[test]
fn multiline_lint_attributes_record_target_function_container() {
    let src = r#"
#[expect(
    dead_code,
    reason = "policy:allow-shared"
)]
fn parse() {}

#[expect(
    dead_code,
    reason = "policy:allow-shared"
)]
fn render() {}
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let keyed = findings
        .iter()
        .filter(|f| f.kind == FindingKind::LintException)
        .map(|f| {
            (
                f.identity.container.as_deref(),
                f.identity.target_fingerprint.as_deref(),
                f.identity.stable_key(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(keyed.len(), 2);
    assert_eq!(keyed[0].0, Some("parse"));
    assert_eq!(keyed[1].0, Some("render"));
    assert_eq!(keyed[0].1, Some("policy:allow-shared"));
    assert_ne!(keyed[0].2, keyed[1].2);
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
fn detects_cfg_attr_deny_forbid_warn_lint_attributes() {
    // #2578: cfg_attr nested deny/forbid/warn invocations were not detected.
    // Only allow/expect were recognized inside cfg_attr(...). This test
    // verifies the three newly-recognized kinds emit findings with the
    // correct family and lint identity.
    let src = r#"
#![cfg_attr(feature = "strict", deny(unsafe_code))]
#[cfg_attr(target_os = "windows", forbid(clippy::unwrap_used))]
#[cfg_attr(test, warn(dead_code))]
fn load() {}
        "#;
    let findings = scan_rust_source("src/lib.rs", src);

    let deny = findings
        .iter()
        .find(|f| f.family.as_deref() == Some("deny_attribute"))
        .unwrap_or_else(|| std::panic::panic_any("cfg_attr deny should be found"));
    assert_eq!(deny.identity.lint.as_deref(), Some("unsafe_code"));

    let forbid = findings
        .iter()
        .find(|f| f.family.as_deref() == Some("forbid_attribute"))
        .unwrap_or_else(|| std::panic::panic_any("cfg_attr forbid should be found"));
    assert_eq!(forbid.identity.lint.as_deref(), Some("clippy::unwrap_used"));

    let warn = findings
        .iter()
        .find(|f| f.family.as_deref() == Some("warn_attribute"))
        .unwrap_or_else(|| std::panic::panic_any("cfg_attr warn should be found"));
    assert_eq!(warn.identity.lint.as_deref(), Some("dead_code"));
}

#[test]
fn cfg_attr_deny_forbid_warn_detection_ignores_strings() {
    // #2578: the extended cfg_attr detection must not match deny/forbid/warn
    // text appearing inside string literals or doc comments.
    let src = r##"
#[cfg_attr(feature = "docs", doc = "deny(unsafe_code)")]
#[cfg_attr(feature = "docs", doc = r#"forbid(clippy::unwrap_used)"#)]
#[doc = "warn(dead_code) inside a string"]
fn load() {}
        "##;
    let findings = scan_rust_source("src/lib.rs", src);

    assert!(
        !findings
            .iter()
            .any(|f| f.family.as_deref() == Some("deny_attribute")
                || f.family.as_deref() == Some("forbid_attribute")
                || f.family.as_deref() == Some("warn_attribute")),
        "string-embedded deny/forbid/warn text must not produce findings: {findings:?}"
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
