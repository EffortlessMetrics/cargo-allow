use crate::scan_rust_source;
use crate::text::{index_symbol, index_target_fingerprint};

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

    assert_eq!(indexing, 4);
}

#[test]
fn syntax_indexing_detects_multiple_expressions_on_one_line() {
    let src = r#"
        fn load(left: &[u8], right: &[u8]) -> u8 {
            left[0] + right[1]
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let indexing = findings
        .iter()
        .filter(|f| f.family.as_deref() == Some("indexing"))
        .count();

    assert_eq!(indexing, 2);
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
