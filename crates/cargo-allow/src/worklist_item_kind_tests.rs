use super::parse_work_item_kind_filter;

#[test]
fn parse_work_item_kind_filter_call_presence_observer() {
    assert_eq!(
        parse_work_item_kind_filter("new-unreceipted-finding"),
        Ok("new_unreceipted_finding".to_string())
    );
    assert_eq!(
        parse_work_item_kind_filter("stale_allow"),
        Ok("stale_allow".to_string())
    );
    assert!(
        parse_work_item_kind_filter("unknown_kind")
            .unwrap_err()
            .starts_with("unknown work item kind `unknown_kind`; supported kinds:")
    );
}
