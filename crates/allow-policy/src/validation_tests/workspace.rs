use super::*;

#[test]
fn rejects_empty_workspace_root() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [workspace]
                root = "   "
            "#,
    );

    assert!(err.contains("workspace root has empty path"));
}

#[test]
fn rejects_parent_workspace_root() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [workspace]
                root = "../outside"
            "#,
    );

    assert!(err.contains("workspace root path must not contain parent directory segments"));
}

#[test]
fn rejects_unsupported_workspace_inventory() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [workspace]
                inventory = "filesystem"
            "#,
    );

    assert!(err.contains("unsupported workspace inventory `filesystem`"));
}

#[test]
fn rejects_workspace_inventory_with_surrounding_whitespace() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [workspace]
                inventory = " git-tracked "
            "#,
    );

    assert!(err.contains("workspace inventory must not have leading or trailing whitespace"));
}

#[test]
fn accepts_artifact_style_git_tracked_workspace_inventory_alias() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [workspace]
                inventory = "git_tracked"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy should parse: {err}")));

    assert_eq!(cfg.workspace.inventory, "git-tracked");
}

#[test]
fn rejects_unsupported_workspace_default_mode() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [workspace]
                default_mode = "permissive"
            "#,
    );

    assert!(err.contains("unsupported workspace default_mode `permissive`"));
}

#[test]
fn rejects_workspace_default_mode_with_surrounding_whitespace() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [workspace]
                default_mode = " no-new "
            "#,
    );

    assert!(err.contains("workspace default_mode must not have leading or trailing whitespace"));
}

#[test]
fn rejects_invalid_workspace_ignored_glob() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [workspace]
                ignored = ["../target/**"]
            "#,
    );

    assert!(err.contains("source-tree ignored glob must not contain parent directory segments"));
}

#[test]
fn rejects_workspace_glob_with_surrounding_whitespace() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [workspace]
                ignored = [" scripts/** "]
            "#,
    );

    assert!(err.contains("source-tree ignored glob must not have leading or trailing whitespace"));
}

#[test]
fn rejects_invalid_workspace_generated_glob() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [workspace]
                generated = ["/generated/**"]
            "#,
    );

    assert!(err.contains("source-tree generated glob must be source-tree-relative"));
}

#[test]
fn rejects_unsupported_brace_glob_syntax() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [workspace]
                ignored = ["scripts/{a,b}.sh"]
            "#,
    );

    assert!(err.contains("unsupported glob token `{`"));
}

#[test]
fn rejects_non_whole_segment_workspace_globstar_syntax() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                [workspace]
                ignored = ["scripts/foo**/release.sh"]
            "#,
    );

    assert!(err.contains("source-tree ignored glob uses unsupported glob token `**`"));
}
