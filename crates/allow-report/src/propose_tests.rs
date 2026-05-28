use super::*;

#[test]
fn propose_json_renderer_records_options_summary_and_defaults() {
    let report = ProposeReport {
        inventory: InventoryContext::source_syntax(
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(76),
        ),
        kind: Some("panic"),
        expires: "2026-08-02",
        policy_output: Some("target/cargo-allow/proposed.toml"),
        force: true,
        findings_scanned: 54,
        baseline_debt_entries_proposed: 2,
    };

    let json = render_propose_json(report);

    assert!(json.contains("\"schema_id\": \"cargo-allow.propose.v1\""));
    assert!(json.contains("\"command\": \"propose\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"files_scanned\": 76"));
    assert!(json.contains("\"kind\": \"panic\""));
    assert!(json.contains("\"expires\": \"2026-08-02\""));
    assert!(json.contains("\"policy_output\": \"target/cargo-allow/proposed.toml\""));
    assert!(json.contains("\"force\": true"));
    assert!(json.contains("\"findings_scanned\": 54"));
    assert!(json.contains("\"baseline_debt_entries_proposed\": 2"));
    assert!(json.contains("\"owner\": \"unowned\""));
    assert!(json.contains("\"classification\": \"baseline_debt\""));

    let text = render_propose_human(report);

    assert!(text.contains("cargo-allow propose summary"));
    assert!(text.contains("findings scanned: 54"));
    assert!(text.contains("baseline_debt entries proposed: 2"));
    assert!(text.contains("owner: unowned"));
    assert!(text.contains("classification: baseline_debt"));
    assert!(text.contains("expires: 2026-08-02"));
    assert!(text.contains("output: target/cargo-allow/proposed.toml"));
    assert!(text.contains("generated debt still requires human review"));
}
