use allow_core::{Finding, FindingKind, Span, StructuralIdentity};
use std::path::PathBuf;

use crate::explain_common::finding_location_text;

fn minimal_finding(path: &str, span: Option<Span>) -> Finding {
    Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: PathBuf::from(path),
        span,
        identity: StructuralIdentity::new("rust", "method_call"),
        message: "fixture".to_string(),
        ledger: None,
    }
}

#[test]
fn finding_location_text_match_arm_discriminator() {
    let with_span = minimal_finding(
        "src\\module.rs",
        Some(Span {
            line: 12,
            column: 3,
        }),
    );
    assert_eq!(finding_location_text(&with_span), "src/module.rs:12:3");

    let without_span = minimal_finding("src\\module.rs", None);
    assert_eq!(finding_location_text(&without_span), "src/module.rs");
}

#[test]
fn finding_location_text_call_presence_observer() {
    let finding = minimal_finding(r"crates\allow-report\src\lib.rs", None);

    assert_eq!(
        finding_location_text(&finding),
        "crates/allow-report/src/lib.rs"
    );
}
