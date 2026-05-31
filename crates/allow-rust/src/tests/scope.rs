use crate::scan_rust_source;

#[test]
fn scan_uses_syntax_container_scope() {
    let src = r#"
        fn actual(value: Result<(), ()>) {
            let text = "fn fake() {";
            value.unwrap();
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let Some(finding) = findings
        .iter()
        .find(|f| f.family.as_deref() == Some("unwrap"))
    else {
        std::panic::panic_any("unwrap finding should exist");
    };

    assert_eq!(finding.identity.container.as_deref(), Some("actual"));
}

#[test]
fn scan_uses_syntax_module_scope() {
    let src = r#"
        mod parser {
            fn parse(value: Result<(), ()>) {
                value.unwrap();
            }
        }

        fn load(value: Result<(), ()>) {
            value.expect("loaded");
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let Some(parser_finding) = findings
        .iter()
        .find(|f| f.family.as_deref() == Some("unwrap"))
    else {
        std::panic::panic_any("parser unwrap finding should exist");
    };
    let Some(root_finding) = findings
        .iter()
        .find(|f| f.family.as_deref() == Some("expect"))
    else {
        std::panic::panic_any("root expect finding should exist");
    };

    assert_eq!(parser_finding.identity.module.as_deref(), Some("parser"));
    assert_eq!(parser_finding.identity.container.as_deref(), Some("parse"));
    assert_eq!(root_finding.identity.module, None);
    assert_eq!(root_finding.identity.container.as_deref(), Some("load"));
}

#[test]
fn scan_uses_syntax_impl_method_scope() {
    let src = r#"
        mod parser {
            struct Parser;

            impl Parser {
                fn parse(&self, value: Result<(), ()>) {
                    value.unwrap();
                }
            }
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let Some(finding) = findings
        .iter()
        .find(|f| f.family.as_deref() == Some("unwrap"))
    else {
        std::panic::panic_any("impl unwrap finding should exist");
    };

    assert_eq!(finding.identity.module.as_deref(), Some("parser"));
    assert_eq!(finding.identity.container.as_deref(), Some("Parser::parse"));
}

#[test]
fn scan_uses_syntax_trait_impl_method_scope() {
    let src = r#"
        trait ParserApi {
            fn parse(&self, value: Result<(), ()>);
        }

        struct Parser;

        impl ParserApi for Parser {
            fn parse(&self, value: Result<(), ()>) {
                value.unwrap();
            }
        }
        "#;
    let findings = scan_rust_source("src/lib.rs", src);
    let Some(finding) = findings
        .iter()
        .find(|f| f.family.as_deref() == Some("unwrap"))
    else {
        std::panic::panic_any("trait impl unwrap finding should exist");
    };

    assert_eq!(
        finding.identity.container.as_deref(),
        Some("<Parser as ParserApi>::parse")
    );
}
