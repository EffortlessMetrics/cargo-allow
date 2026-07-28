use std::fs;
use std::process::{Command, Output};

const SHELLS: [&str; 5] = ["bash", "zsh", "fish", "powershell", "elvish"];

/// Single fallible boundary for the whole file: spawning the binary. Keeping
/// it in one place means one error path rather than one per call site.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow {args:?}: {err}")))
}

fn stdout_of(result: &Output) -> String {
    String::from_utf8_lossy(&result.stdout).into_owned()
}

fn stderr_of(result: &Output) -> String {
    String::from_utf8_lossy(&result.stderr).into_owned()
}

/// `completions <shell>` writes a usable script to stdout and nothing to
/// stderr, so `cargo-allow completions bash > file` produces a clean file.
#[test]
fn completions_write_a_clean_script_to_stdout() {
    for shell in SHELLS {
        let result = run(&["completions", shell]);

        assert!(
            result.status.success(),
            "completions {shell} failed: {}",
            stderr_of(&result)
        );
        assert!(
            result.stderr.is_empty(),
            "completions {shell} should not emit stderr: {}",
            stderr_of(&result)
        );
        assert!(
            stdout_of(&result).contains("cargo-allow"),
            "completions {shell} should name the binary"
        );
    }
}

/// `--output` is the documented way to install a completion script, so the
/// file it writes must match what stdout produces.
#[test]
fn output_flag_writes_the_same_script_as_stdout() {
    let path = std::env::temp_dir().join(format!(
        "cargo-allow-completions-{}.bash",
        std::process::id()
    ));
    let path_arg = path.display().to_string();

    let piped = run(&["completions", "bash"]);
    assert!(piped.status.success(), "completions bash should succeed");

    let written = run(&["completions", "bash", "--output", &path_arg]);
    assert!(
        written.status.success(),
        "completions --output should succeed: {}",
        stderr_of(&written)
    );

    let from_file = fs::read_to_string(&path).unwrap_or_default();
    assert!(
        !from_file.is_empty(),
        "completions --output should have written a script to {path_arg}"
    );

    // `emit_text` writes files verbatim but prints to stdout with `println!`,
    // so the piped form carries one extra trailing newline. That is the shared
    // convention for every `--output` command, not something specific to
    // completions; the script content itself must be identical.
    assert_eq!(
        stdout_of(&piped),
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
    let result = run(&["completions", "nushell"]);

    assert!(
        !result.status.success(),
        "an unsupported shell should be rejected"
    );
    assert!(
        result.stdout.is_empty(),
        "a rejected shell should not emit a partial script"
    );
}
