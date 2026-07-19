use super::CheckArgs;
use crate::OutputFormat;

#[test]
fn check_args_leave_mode_unset_for_policy_default() {
    let args = CheckArgs {
        root: crate::RootArgs::default(),
        config: None,
        profile: None,
        compat: false,
        kind: None,
        include_untracked: false,
        format: OutputFormat::Human,
        output: None,
        receipt: None,
        mode: None,
        deny: Vec::new(),
        phase: None,
        staged: false,
        staged_identity_only: false,
        expect_staged_identity: None,
        tool_mode: None,
        tool_digest: None,
        preview_authorized: false,
    };
    assert!(args.mode.is_none());
}
