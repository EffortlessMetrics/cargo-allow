use super::{CheckArgs, cmd_check};
use crate::OutputFormat;
use std::fs;

#[test]
fn check_args_leave_mode_unset_for_policy_default() {
    let args = CheckArgs {
        artifact_dir: None,
        emit: None,
        persistent_cache: super::check_args::PersistentCacheMode::On,
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

#[test]
fn source_tree_check_emits_scan_status_before_missing_policy_error() -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-check-status-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;

    let result = cmd_check(&CheckArgs {
        artifact_dir: None,
        emit: None,
        persistent_cache: super::check_args::PersistentCacheMode::On,
        root: crate::RootArgs {
            root: Some(root.clone()),
        },
        config: None,
        profile: None,
        compat: false,
        kind: None,
        include_untracked: false,
        format: OutputFormat::Human,
        output: None,
        receipt: None,
        mode: Some("no-new".to_string()),
        deny: Vec::new(),
        phase: None,
        staged: false,
        staged_identity_only: false,
        expect_staged_identity: None,
        tool_mode: None,
        tool_digest: None,
        preview_authorized: false,
    });
    let cleanup = fs::remove_dir_all(&root).map_err(|error| error.to_string());
    if result.is_ok() {
        return Err("check unexpectedly succeeded without a policy".to_string());
    }
    cleanup?;
    Ok(())
}
