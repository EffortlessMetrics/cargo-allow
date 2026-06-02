use crate::parse_rust_syntax;

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
fn syntax_containers_include_trait_definition_methods() {
    let source = r#"
        mod parser {
            trait ParserApi {
                fn parse_span(&self) {}
            }
        }
        "#;
    let tree = parse_rust_syntax(source)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parser should load: {err}")));
    let containers = tree.containers(source);

    let method = containers
        .iter()
        .find(|container| container.name == "ParserApi::parse_span")
        .unwrap_or_else(|| std::panic::panic_any("ParserApi::parse_span should exist"));
    assert_eq!(method.kind, "method");
    assert_eq!(method.module().as_deref(), Some("parser"));
}

#[test]
fn syntax_containers_include_trait_method_signatures() {
    let source = r#"
        mod parser {
            trait ParserApi {
                fn parse_span(&self);
            }
        }
        "#;
    let tree = parse_rust_syntax(source)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parser should load: {err}")));
    let containers = tree.containers(source);

    let method = containers
        .iter()
        .find(|container| container.name == "ParserApi::parse_span")
        .unwrap_or_else(|| std::panic::panic_any("ParserApi::parse_span signature should exist"));
    assert_eq!(method.kind, "method");
    assert_eq!(method.module().as_deref(), Some("parser"));
}

#[test]
fn syntax_containers_include_extern_function_signatures_with_abi() {
    let source = r#"
        extern "C" {
            fn access();
        }

        extern "system" {
            fn access();
        }
        "#;
    let tree = parse_rust_syntax(source)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parser should load: {err}")));
    let names = tree
        .containers(source)
        .into_iter()
        .map(|container| container.name)
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec![
            "extern \"C\"::access".to_string(),
            "extern \"system\"::access".to_string(),
        ]
    );
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
