use crate::scan_rust_source;

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
fn syntax_indexing_records_receiver_identity_per_expression() {
    let src = r#"
        fn load(left: &[u8], right: &[u8]) -> u8 {
            left[0] + right[1]
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let receivers = findings
        .iter()
        .filter(|f| f.family.as_deref() == Some("indexing"))
        .map(|f| f.identity.receiver_fingerprint.as_deref())
        .collect::<Vec<_>>();

    assert_eq!(receivers, vec![Some("left"), Some("right")]);
}

#[test]
fn syntax_indexing_records_symbol_identity_per_expression() {
    let src = r#"
        fn load(left: &[u8], right: &[u8]) -> u8 {
            left[0] + right[1]
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let symbols = findings
        .iter()
        .filter(|f| f.family.as_deref() == Some("indexing"))
        .map(|f| f.identity.symbol.as_deref())
        .collect::<Vec<_>>();

    assert_eq!(symbols, vec![Some("left[0]"), Some("right[1]")]);
}

#[test]
fn syntax_indexing_records_target_identity_per_expression() {
    let src = r#"
        fn load(left: &[u8], right: &[u8]) -> u8 {
            left[0] + right[1]
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let targets = findings
        .iter()
        .filter(|f| f.family.as_deref() == Some("indexing"))
        .map(|f| f.identity.target_fingerprint.as_deref())
        .collect::<Vec<_>>();

    assert_eq!(targets, vec![Some("left"), Some("right")]);
}

#[test]
fn syntax_indexing_keeps_borrowed_index_family_as_indexing() {
    let src = r#"
        fn load(items: &[u8]) -> &u8 {
            &items[0]
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let families = findings
        .iter()
        .filter(|f| f.identity.ast_kind == "index_expr")
        .map(|f| f.family.as_deref())
        .collect::<Vec<_>>();

    assert_eq!(families, vec![Some("indexing")]);
}

#[test]
fn syntax_indexing_classifies_range_indexes_as_slices() {
    let src = r#"
        fn load(text: &str) {
            let prefix = text[..1];
            let middle = text[1..3];
            let inclusive = text[1..=3];
            let borrowed = &text[0..1];
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let families = findings
        .iter()
        .filter(|f| f.identity.ast_kind == "index_expr")
        .map(|f| f.family.as_deref())
        .collect::<Vec<_>>();

    assert_eq!(
        families,
        vec![
            Some("string_slice"),
            Some("string_slice"),
            Some("string_slice"),
            Some("string_slice"),
        ]
    );
}

#[test]
fn syntax_indexing_records_nested_receiver_identity() {
    let src = r#"
        fn load(matrix: &[&[u8]]) -> u8 {
            matrix[0][1]
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let receivers = findings
        .iter()
        .filter(|f| f.family.as_deref() == Some("indexing"))
        .map(|f| f.identity.receiver_fingerprint.as_deref())
        .collect::<Vec<_>>();

    assert_eq!(receivers, vec![Some("matrix"), Some("matrix[0]")]);
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
