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

#[test]
fn parse_kind_filter_arg_accepts_case_insensitive_aliases() {
    assert_eq!(
        parse_kind_filter_arg(" PANIC-FAMILY "),
        Ok(" PANIC-FAMILY ".to_string())
    );
    assert_eq!(
        parse_kind_filter_arg("WORKFLOW"),
        Ok("WORKFLOW".to_string())
    );
}
