use crate::compat::compat_selected_policy_path;

#[test]
fn compat_kinds_select_their_legacy_policy_defaults() {
    // #4576 review: the artifact-collision guard protects the legacy
    // file the compat world actually loads; the mapping here must match
    // the loaders' defaults.
    let root = std::path::Path::new("/repo");
    let cases = [
        ("lint-exception", "policy/clippy-exceptions.toml"),
        ("network", "policy/network-allowlist.toml"),
        ("panic", "policy/no-panic-baseline.toml"),
        ("unsafe", "policy/unsafe-allowlist.toml"),
        ("process", "policy/process-allowlist.toml"),
        ("no-panic-allowlist", "policy/no-panic-allowlist.toml"),
        ("executable", "policy/executable-allowlist.toml"),
        ("workflow", "policy/workflow-allowlist.toml"),
        (
            "dependency-surface",
            "policy/dependency-surface-allowlist.toml",
        ),
        ("generated", "policy/generated-allowlist.toml"),
    ];
    for (kind, expected) in cases {
        assert_eq!(
            compat_selected_policy_path(None, root, Some(kind)),
            Some(root.join(expected)),
            "kind {kind} must select {expected}"
        );
    }
    // The generated surface accepts the parser's spellings (#4578
    // review): whitespace-trimmed, hyphen and underscore forms.
    assert_eq!(
        compat_selected_policy_path(None, root, Some(" generated-code ")),
        Some(root.join("policy/generated-allowlist.toml")),
    );
    assert_eq!(
        compat_selected_policy_path(None, root, Some("generated_code")),
        Some(root.join("policy/generated-allowlist.toml")),
    );
    // The parser is case-insensitive, and load_compat_world selects the
    // generated policy for uppercase spellings through it.
    assert_eq!(
        compat_selected_policy_path(None, root, Some("GENERATED")),
        Some(root.join("policy/generated-allowlist.toml")),
    );
    // An explicit --config overrides the per-kind default file.
    assert_eq!(
        compat_selected_policy_path(
            Some(std::path::Path::new("custom/legacy.toml")),
            root,
            Some("network"),
        ),
        Some(std::path::PathBuf::from("custom/legacy.toml")),
    );
}
