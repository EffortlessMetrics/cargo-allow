use super::parse_worklist_kind_filter;

#[test]
fn parse_worklist_kind_filter_call_presence_observer() {
    assert_eq!(parse_worklist_kind_filter("panic"), Ok("panic".to_string()));
    assert_eq!(
        parse_worklist_kind_filter("workflow"),
        Ok("workflow".to_string())
    );
    assert!(
        parse_worklist_kind_filter("unknown_kind")
            .unwrap_err()
            .contains("supported kinds:"),
        "worklist kind error should list supported kinds"
    );
}
