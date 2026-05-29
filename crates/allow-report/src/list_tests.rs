use super::*;

#[test]
fn list_json_renderer_records_filters_context_and_rows() {
    let rows = vec![ListRow {
        id: "allow-json",
        status: "baseline_debt",
        matches: 1,
        kind: "panic",
        family: Some("unwrap"),
        owner: "parser",
        classification: "baseline_debt",
        scope: "crates/parser/src/lib.rs",
        source_package: Some("parser"),
        evidence_count: 2,
        selector_precision: 42,
        broad_scope: true,
        review_after: Some("2026-07-01"),
        expires: None,
        reason: "generated baseline",
    }];

    let json = render_list_json(
        &rows,
        ListFilters {
            kind: Some("panic"),
            family: Some("unwrap"),
            owner: Some("parser"),
            baseline_debt: true,
            ..ListFilters::default()
        },
        InventoryContext::source_syntax("git_tracked", Some("H:/Code/Rust/cargo-allow"), Some(46)),
    );

    assert!(json.contains("\"schema_id\": \"cargo-allow.list.v1\""));
    assert!(json.contains("\"command\": \"list\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"files_scanned\": 46"));
    assert!(json.contains("\"kind\": \"panic\""));
    assert!(json.contains("\"family\": \"unwrap\""));
    assert!(json.contains("\"owner\": \"parser\""));
    assert!(json.contains("\"baseline_debt\": true"));
    assert!(json.contains("\"allow_entries\": 1"));
    assert!(json.contains("\"id\": \"allow-json\""));
    assert!(json.contains("\"source_package\": \"parser\""));
    assert!(json.contains("\"selector_precision\": 42"));
    assert!(json.contains("\"broad_scope\": true"));
    assert!(json.contains("\"review_after\": \"2026-07-01\""));
    assert!(json.contains("\"expires\": null"));

    let text = render_list_human(&rows);

    assert!(text.starts_with("id\tstatus\tmatches\tkind\tfamily"));
    assert!(text.contains(
            "allow-json\tbaseline_debt\t1\tpanic\tunwrap\tparser\tbaseline_debt\tcrates/parser/src/lib.rs\tparser\t2\t42\ttrue\t2026-07-01\t-\tgenerated baseline"
        ));
}
