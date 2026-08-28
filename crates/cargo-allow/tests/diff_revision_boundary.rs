use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn cargo_allow_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|err| panic!("system clock: {err}"))
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| panic!("create temp root: {err}"));
    root
}

fn remove_temp_root(root: PathBuf) {
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => panic!("remove temp root {}: {err}", root.display()),
    }
}

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
        .unwrap_or_else(|err| panic!("run cargo-allow diff: {err}"));

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
