mod support;

use support::{cargo_allow_command, remove_temp_root, temp_root};

#[test]
fn diff_rejects_option_like_base_before_git_can_create_output() {
    let root = temp_root("diff-option-like-base");
    let side_effect = root.join("git-option-side-effect.txt");
    let base = format!("--base=--output={}", side_effect.display());

    let result = cargo_allow_command()
        .arg("diff")
        .arg("--root")
        .arg(&root)
        .arg(base)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow diff: {err}")));

    assert!(!result.status.success(), "option-like base must fail");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("revision must not start with `-`"),
        "unexpected stderr: {stderr}"
    );
    assert!(
        !side_effect.exists(),
        "invalid revision must not create {}",
        side_effect.display()
    );

    remove_temp_root(root);
}
