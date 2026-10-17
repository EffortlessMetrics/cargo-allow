use super::{
    is_compat_only_kind_filter, parse_kind_filter, parse_kind_filter_arg,
    reject_compat_only_kind_filter,
};
use allow_core::{CargoAllowErrorKind, FindingKind};

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

#[test]
fn compat_only_kind_filter_detects_the_five_compat_surfaces() {
    // #4632: exactly the five kinds whose sensors live behind --compat, in
    // every alias spelling, map to a compat-only filter.
    for kind in [
        "workflow",
        "github_workflow",
        "executable",
        "exec",
        "process",
        "proc",
        "network",
        "net",
        "dependency-surface",
        "dep",
    ] {
        let parsed = parse_kind_filter(kind)
            .unwrap_or_else(|err| std::panic::panic_any(format!("parse {kind}: {err}")));
        assert!(
            is_compat_only_kind_filter(parsed),
            "`{kind}` should be a compat-only filter: {parsed:?}"
        );
        assert_eq!(parsed.kind, FindingKind::PolicyException);
    }
}

#[test]
fn non_compat_kind_filters_are_not_compat_only() {
    for kind in [
        "panic",
        "unsafe",
        "lint-exception",
        "non-rust",
        "generated",
        "policy-exception",
    ] {
        let parsed = parse_kind_filter(kind)
            .unwrap_or_else(|err| std::panic::panic_any(format!("parse {kind}: {err}")));
        assert!(
            !is_compat_only_kind_filter(parsed),
            "`{kind}` must stay accepted outside --compat"
        );
    }
}

#[test]
fn reject_compat_only_kind_filter_names_kind_and_compat_lane() {
    let err = reject_compat_only_kind_filter("check", false, Some("workflow"))
        .expect_err("non-compat check must reject a compat-only kind");
    assert_eq!(err.kind(), CargoAllowErrorKind::Usage);
    let message = err.to_string();
    assert!(
        message.contains("--kind workflow") && message.contains("--compat lane for check"),
        "error should name the kind and the lane: {message}"
    );
    assert!(
        message.contains("permanently green"),
        "error should state the green-gate hazard: {message}"
    );
}

#[test]
fn reject_compat_only_kind_filter_passes_compat_lane_and_other_kinds() {
    // The compat lane keeps its policy-demand behavior (#4632).
    reject_compat_only_kind_filter("check", true, Some("workflow"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("compat lane must pass: {err}")));
    // Non-compat kinds are not the guard's business.
    reject_compat_only_kind_filter("check", false, Some("non-rust"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("non-rust must pass: {err}")));
    // No filter at all passes, and an unparseable kind stays the caller's
    // parse error.
    reject_compat_only_kind_filter("why", false, None)
        .unwrap_or_else(|err| std::panic::panic_any(format!("absent kind must pass: {err}")));
    reject_compat_only_kind_filter("why", false, Some("not_a_kind"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("unknown kind must pass: {err}")));
}
