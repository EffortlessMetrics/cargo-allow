use super::*;

#[test]
fn receipt_exposes_v1_schema_contract() {
    let json = render_receipt_with_context(
        "check",
        &[],
        true,
        ReportContext::source_syntax(
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(42),
            None,
        ),
    );
    assert!(json.contains("\"schema_version\": 1"));
    assert!(json.contains("\"schema_id\": \"cargo-allow.receipt.v1\""));
    assert!(json.contains("\"failed\": true"));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"files_scanned\": 42"));
    assert!(json.contains("\"cargo_metadata_not_invoked\""));
    assert!(json.contains("\"cargo_commands_not_invoked\""));
    assert!(json.contains("\"build_output_not_analyzed\""));
    assert!(json.contains("\"macro_token_tree_contents_not_analyzed\""));
    assert!(json.contains("\"missing_required_field\": 0"));
    assert!(json.contains("\"evidence_missing\": 0"));
}

#[test]
fn receipt_counts_policy_baseline_debt_context() {
    let json = render_receipt_with_context(
        "check",
        &[],
        false,
        ReportContext::source_syntax("git_tracked", None, None, Some(3)),
    );

    assert!(json.contains("\"baseline_debt\": 0"));
    assert!(json.contains("\"policy_baseline_debt\": 3"));
}

#[test]
fn receipt_counts_broken_evidence_links_context() {
    let mut context = ReportContext::source_syntax("git_tracked", None, None, None);
    context.broken_evidence_links = Some(2);

    let json = render_receipt_with_context("check", &[], false, context);

    assert!(json.contains("\"broken_evidence_links\": 2"));
}

#[test]
fn receipt_counts_policy_missing_evidence_context() {
    let mut context = ReportContext::source_syntax("git_tracked", None, None, None);
    context.policy_missing_evidence_entries = Some(4);

    let json = render_receipt_with_context("check", &[], false, context);

    assert!(json.contains("\"policy_missing_evidence\": 4"));
}
