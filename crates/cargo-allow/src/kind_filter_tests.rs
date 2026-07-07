use super::parse_kind_filter_arg;

#[test]
fn parse_kind_filter_arg_call_presence_observer() {
    assert_eq!(parse_kind_filter_arg("panic"), Ok("panic".to_string()));
    assert_eq!(
        parse_kind_filter_arg("workflow"),
        Ok("workflow".to_string())
    );
    assert_eq!(
        parse_kind_filter_arg("unknown_kind"),
        Err(
            "unknown kind `unknown_kind`; supported kinds: panic, unsafe, lint-exception, non-rust, generated, policy-exception, no-panic-allowlist, executable, workflow, dependency-surface, process, network"
                .to_string()
        )
    );
}
