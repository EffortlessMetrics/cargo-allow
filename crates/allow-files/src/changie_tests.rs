//! Changie sensor parse tests (#3588).

use crate::changie::*;

fn doc(path: &str, text: &str) -> ChangieSourceDocument {
    ChangieSourceDocument::from_bytes(
        ChangieRepoPath::from_repo_relative(path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("repo path: {err}"))),
        text.as_bytes().to_vec(),
        Some("test-subject".to_string()),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("source doc: {err}")))
}

#[test]
fn generation_is_pinned() {
    assert_eq!(ChangieCompatibilityGeneration::current().as_str(), "1.25");
    assert_eq!(CHANGIE_COMPATIBILITY_GENERATION, "1.25");
}

#[test]
fn repository_config_parses_with_no_unknown_top_level_fields() {
    let text = include_str!("../../../.changie.yaml");
    let config = parse_config(doc(".changie.yaml", text));
    assert!(
        config.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        config.diagnostics
    );
    assert!(
        config.unknown_fields.is_empty(),
        "real config reported unknown fields: {:?}",
        config
            .unknown_fields
            .iter()
            .map(|f| f.path.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        config_field(&config, "changesDir"),
        ChangieFieldPresence::Present(&ChangieNode {
            value: ChangieValue::String(".changes".into()),
            range: config
                .root
                .as_ref()
                .and_then(|node| match &node.value {
                    ChangieValue::Mapping(mapping) => mapping.first("changesDir"),
                    _ => None,
                })
                .map(|node| node.range)
                .unwrap_or(ChangieSourceRange {
                    start: ChangieSourcePos {
                        line: 1,
                        column: 1,
                        index: 0
                    },
                    end: ChangieSourcePos {
                        line: 1,
                        column: 1,
                        index: 0
                    },
                }),
        }),
        "changesDir must parse as a string without coercion"
    );
    match config_field(&config, "kinds") {
        ChangieFieldPresence::Present(node) => {
            assert!(matches!(node.value, ChangieValue::Sequence(_)));
        }
        ChangieFieldPresence::Missing => std::panic::panic_any("kinds missing from real config"),
    }
    match config_field(&config, "unreleasedDir") {
        ChangieFieldPresence::Present(node) => {
            assert_eq!(node.value, ChangieValue::String(".".into()));
        }
        ChangieFieldPresence::Missing => {
            std::panic::panic_any("unreleasedDir missing from real config")
        }
    }
}

#[test]
fn presence_law_distinguishes_missing_null_and_empty() {
    let config = parse_config(doc(
        ".changie.yaml",
        "nullValue:\nquotedEmpty: \"\"\nplainString: text\n",
    ));
    assert_eq!(
        config_field(&config, "absent").as_ref(),
        ChangieFieldPresence::Missing
    );
    match config_field(&config, "nullValue") {
        ChangieFieldPresence::Present(node) => assert_eq!(node.value, ChangieValue::Null),
        ChangieFieldPresence::Missing => std::panic::panic_any("nullValue should be present"),
    }
    match config_field(&config, "quotedEmpty") {
        ChangieFieldPresence::Present(node) => {
            assert_eq!(node.value, ChangieValue::EmptyString)
        }
        ChangieFieldPresence::Missing => std::panic::panic_any("quotedEmpty should be present"),
    }
    match config_field(&config, "plainString") {
        ChangieFieldPresence::Present(node) => {
            assert_eq!(node.value, ChangieValue::String("text".into()))
        }
        ChangieFieldPresence::Missing => std::panic::panic_any("plainString should be present"),
    }
}

#[test]
fn scalar_typing_is_preserved_without_coercion() {
    let config = parse_config(doc(
        ".changes/Fixture.yaml",
        "integer: 42\nboolean: true\ntext: \"42\"\nnegatives: -7\n",
    ));
    let shape = |key: &str| match config_field(&config, key) {
        ChangieFieldPresence::Present(node) => node.value.clone(),
        ChangieFieldPresence::Missing => std::panic::panic_any(format!("{key} missing")),
    };
    assert_eq!(shape("integer"), ChangieValue::Integer(42));
    assert_eq!(shape("boolean"), ChangieValue::Boolean(true));
    assert_eq!(shape("text"), ChangieValue::String("42".into()));
    assert_eq!(shape("negatives"), ChangieValue::Integer(-7));
}

#[test]
fn duplicate_keys_are_retained_in_authored_order() {
    let config = parse_config(doc(
        ".changes/Fixture.yaml",
        "kind: Fixed\nkind: Added\nkind: Fixed\n",
    ));
    let root = config
        .root
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("duplicate-key document must still parse"));
    let ChangieValue::Mapping(mapping) = &root.value else {
        std::panic::panic_any("root is not a mapping");
    };
    assert_eq!(mapping.count("kind"), 3, "duplicates must not collapse");
    assert_eq!(
        mapping.first("kind").map(|node| node.value.clone()),
        Some(ChangieValue::String("Fixed".into())),
        "first() honors authored order"
    );
}

#[test]
fn unknown_top_level_fields_are_recorded_with_ranges() {
    let config = parse_config(doc(".changie.yaml", "changesDir: .changes\nnotAField: 1\n"));
    assert_eq!(config.unknown_fields.len(), 1);
    assert_eq!(config.unknown_fields[0].path.to_string(), "notAField");
    assert!(config.unknown_fields[0].range.start.line >= 1);
}

#[test]
fn aliases_are_unsupported_not_resolved() {
    let config = parse_config(doc(".changes/Fixture.yaml", "anchor: &a value\nkind: *a\n"));
    assert_eq!(
        config.unsupported_fields.len(),
        1,
        "alias value recorded: {:?}",
        config.unsupported_fields
    );
    assert!(
        config.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == ChangieParseDiagnosticKind::UnsupportedConstruct
        })
    );
    match config_field(&config, "kind") {
        ChangieFieldPresence::Present(node) => {
            assert_eq!(node.value, ChangieValue::UnsupportedAlias)
        }
        ChangieFieldPresence::Missing => std::panic::panic_any("kind should be present"),
    }
}

#[test]
fn malformed_yaml_fails_closed_with_a_diagnostic() {
    let config = parse_config(doc(".changie.yaml", "kinds:\n\t- tabbed\n"));
    assert!(config.root.is_none(), "malformed document has no tree");
    assert!(
        config
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == ChangieParseDiagnosticKind::Malformed),
        "malformed diagnostic expected: {:?}",
        config.diagnostics
    );
}

#[test]
fn non_utf8_documents_report_without_a_tree() {
    let source = ChangieSourceDocument::from_bytes(
        ChangieRepoPath::from_repo_relative(".changes/Fixture.yaml")
            .unwrap_or_else(|err| std::panic::panic_any(err)),
        vec![0xff, 0xfe, b'k'],
        None,
    )
    .unwrap_or_else(|err| std::panic::panic_any(err));
    let config = parse_config(source);
    assert!(config.root.is_none());
    assert!(
        config
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == ChangieParseDiagnosticKind::NonUtf8)
    );
}

#[test]
fn fragment_document_parses_kind_body_and_custom_shapes() {
    let fragment = parse_fragment(doc(
        ".changes/Fixed-20260816-example.yaml",
        "kind: Fixed\nbody: >-\n  A wrapped body.\ncustom:\n  PR: 12\n",
    ));
    assert!(fragment.diagnostics.is_empty());
    assert!(fragment.unknown_fields.is_empty());
    match fragment_field(&fragment, "custom") {
        ChangieFieldPresence::Present(node) => {
            let ChangieValue::Mapping(mapping) = &node.value else {
                std::panic::panic_any("custom is not a mapping");
            };
            assert_eq!(
                mapping.first("PR").map(|node| node.value.clone()),
                Some(ChangieValue::Integer(12)),
                "custom integer values stay integers; interpretation is validation's job"
            );
        }
        ChangieFieldPresence::Missing => std::panic::panic_any("custom missing"),
    }
    match fragment_field(&fragment, "body") {
        ChangieFieldPresence::Present(node) => match &node.value {
            ChangieValue::String(text) => assert!(text.contains("wrapped body")),
            other => std::panic::panic_any(format!("body shape: {other:?}")),
        },
        ChangieFieldPresence::Missing => std::panic::panic_any("body missing"),
    }
}

#[test]
fn nested_sequences_of_mappings_preserve_order_and_locations() {
    let config = parse_config(doc(
        ".changie.yaml",
        "kinds:\n  - label: Added\n  - label: Fixed\n  - label: Security\n",
    ));
    match config_field(&config, "kinds") {
        ChangieFieldPresence::Present(node) => {
            let ChangieValue::Sequence(items) = &node.value else {
                std::panic::panic_any("kinds is not a sequence");
            };
            assert_eq!(items.len(), 3);
            let labels: Vec<String> = items
                .iter()
                .filter_map(|item| match &item.value {
                    ChangieValue::Mapping(mapping) => match mapping.first("label") {
                        Some(label) => match &label.value {
                            ChangieValue::String(text) => Some(text.clone()),
                            _ => None,
                        },
                        None => None,
                    },
                    _ => None,
                })
                .collect();
            assert_eq!(labels, vec!["Added", "Fixed", "Security"]);
            assert!(items[2].range.start.line > items[0].range.start.line);
        }
        ChangieFieldPresence::Missing => std::panic::panic_any("kinds missing"),
    }
}

#[test]
fn repo_paths_normalize_and_reject_escapes() {
    assert_eq!(
        ChangieRepoPath::from_repo_relative(".changes//Fixture.yaml")
            .map(|path| path.as_str().to_string()),
        Ok(".changes/Fixture.yaml".to_string())
    );
    assert_eq!(
        ChangieRepoPath::from_repo_relative("crates/x/../y/Config.yaml")
            .map(|path| path.as_str().to_string()),
        Ok("crates/y/Config.yaml".to_string())
    );
    assert!(ChangieRepoPath::from_repo_relative("/absolute/path.yaml").is_err());
    assert!(ChangieRepoPath::from_repo_relative("../outside.yaml").is_err());
    assert!(ChangieRepoPath::from_repo_relative("").is_err());
}

#[test]
fn content_identity_and_line_endings_are_retained() {
    let lf = doc(".changie.yaml", "kind: Fixed\n");
    let crlf = ChangieSourceDocument::from_bytes(
        ChangieRepoPath::from_repo_relative(".changie.yaml")
            .unwrap_or_else(|err| std::panic::panic_any(err)),
        b"kind: Fixed\r\n".to_vec(),
        None,
    )
    .unwrap_or_else(|err| std::panic::panic_any(err));
    let mixed = ChangieSourceDocument::from_bytes(
        ChangieRepoPath::from_repo_relative(".changie.yaml")
            .unwrap_or_else(|err| std::panic::panic_any(err)),
        b"kind: Fixed\r\nbody: x\n".to_vec(),
        None,
    )
    .unwrap_or_else(|err| std::panic::panic_any(err));
    assert_eq!(lf.line_ending_class(), ChangieLineEndingClass::Lf);
    assert_eq!(crlf.line_ending_class(), ChangieLineEndingClass::Crlf);
    assert_eq!(mixed.line_ending_class(), ChangieLineEndingClass::Mixed);
    assert_ne!(lf.content_identity(), crlf.content_identity());
    assert_eq!(
        lf.content_identity(),
        doc(".changie.yaml", "kind: Fixed\n").content_identity()
    );
    assert_eq!(lf.subject(), Some("test-subject"));
}
