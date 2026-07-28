use std::fs;
use std::process::Command;

/// `completions <shell>` writes a usable script to stdout and nothing to
/// stderr, so `cargo-allow completions bash > file` produces a clean file.
#[test]
fn completions_write_a_clean_script_to_stdout() {
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        let result = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
            .arg("completions")
            .arg(shell)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("run completions {shell}: {err}")));

        assert!(
            result.status.success(),
            "completions {shell} failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            result.stderr.is_empty(),
            "completions {shell} should not emit stderr: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let script = String::from_utf8_lossy(&result.stdout);
        assert!(
            script.contains("cargo-allow"),
            "completions {shell} should name the binary"
        );
    }
}

/// `--output` is the documented way to install a completion script, so the
/// file it writes must match what stdout produces.
#[test]
fn output_flag_writes_the_same_script_as_stdout() {
    let path = std::env::temp_dir().join(format!(
        "cargo-allow-completions-{}-{}.bash",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|err| std::panic::panic_any(format!("system clock: {err}")))
            .as_nanos()
    ));

    let piped = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .args(["completions", "bash"])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run completions: {err}")));
    assert!(piped.status.success(), "completions bash should succeed");

    let written = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .args(["completions", "bash", "--output"])
        .arg(&path)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run completions --output: {err}")));
    assert!(
        written.status.success(),
        "completions --output should succeed: {}",
        String::from_utf8_lossy(&written.stderr)
    );

    let from_file = fs::read_to_string(&path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read completion file: {err}")));
    let from_stdout = String::from_utf8_lossy(&piped.stdout);

    // `emit_text` writes files verbatim but prints to stdout with `println!`,
    // so the piped form carries one extra trailing newline. That is the shared
    // convention for every `--output` command, not something specific to
    // completions; the script content itself must be identical.
    assert_eq!(
        from_stdout.as_ref(),
        format!("{from_file}\n"),
        "--output should write the same script stdout produces"
    );
    assert!(
        from_file.ends_with('\n'),
        "a completion script file should end with a newline"
    );

    let _ = fs::remove_file(&path);
}

/// An unknown shell must fail loudly rather than emit an empty script that a
/// user would then `source`.
#[test]
fn unknown_shell_fails_with_a_nonzero_status() {
    let result = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .args(["completions", "nushell"])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run completions: {err}")));

    assert!(
        !result.status.success(),
        "an unsupported shell should be rejected"
    );
    assert!(
        result.stdout.is_empty(),
        "a rejected shell should not emit a partial script"
    );
}
