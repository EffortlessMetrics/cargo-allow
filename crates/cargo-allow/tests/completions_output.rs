use std::fs;
use std::process::{Command, Output};

const SHELLS: [&str; 5] = ["bash", "zsh", "fish", "powershell", "elvish"];

/// Single fallible boundary for the whole file: spawning the binary. Keeping
/// it in one place means one error path rather than one per call site.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("run cargo-allow {args:?}: {err}"))
}

fn stdout_of(result: &Output) -> String {
    String::from_utf8_lossy(&result.stdout).into_owned()
}

/// `completions <shell>` writes a usable script to stdout and nothing to
/// stderr, so `cargo-allow completions bash > file` produces a clean file.
#[test]
fn completions_write_a_clean_script_to_stdout() {
    for shell in SHELLS {
        let result = run(&["completions", shell]);
        let script = stdout_of(&result);

        assert!(result.status.success(), "{shell} should succeed");
        assert!(result.stderr.is_empty(), "{shell} should not emit stderr");
        assert!(script.contains("cargo-allow"), "{shell} should name binary");
        assert!(script.ends_with('\n'), "{shell} ends in newline");
    }
}

/// `--output` is the documented way to install a completion script, so the
/// file it writes must match what stdout produces.
///
/// `emit_text` writes files verbatim but prints to stdout with `println!`, so
/// the piped form carries one extra trailing newline. That is the shared
/// convention for every `--output` command, not something specific to
/// completions; the script content itself must be identical.
#[test]
fn output_flag_writes_the_same_script_as_stdout() {
    let path = std::env::temp_dir().join(format!("cargo-allow-comp-{}.bash", std::process::id()));
    let path_arg = path.display().to_string();

    let piped = run(&["completions", "bash"]);
    let written = run(&["completions", "bash", "--output", &path_arg]);
    let from_file = fs::read_to_string(&path).unwrap_or_default();
    let _ = fs::remove_file(&path);

    assert!(piped.status.success(), "piped form should succeed");
    assert!(written.status.success(), "--output form should succeed");
    assert!(!from_file.is_empty(), "--output should write a script");
    assert!(from_file.ends_with('\n'), "file should end in a newline");
    assert_eq!(stdout_of(&piped), format!("{from_file}\n"), "same script");
}

/// An unknown shell must fail loudly rather than emit an empty script that a
/// user would then `source`.
#[test]
fn unknown_shell_fails_with_a_nonzero_status() {
    let result = run(&["completions", "nushell"]);

    assert!(!result.status.success(), "unknown shell should be rejected");
    assert!(result.stdout.is_empty(), "no partial script on rejection");
}
