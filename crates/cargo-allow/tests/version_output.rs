use std::process::{Command, Output};

#[test]
fn root_version_flag_prints_package_version() {
    let result = cargo_allow_command()
        .arg("--version")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow --version: {err}")));

    assert_status("--version", &result, true);
    assert_stderr_empty("--version", &result, "should not emit stderr");
    assert_eq!(
        String::from_utf8_lossy(&result.stdout).trim(),
        format!("cargo-allow {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn cargo_subcommand_compat_version_flag_prints_package_version() {
    let result = cargo_allow_command()
        .arg("allow")
        .arg("--version")
        .output()
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("run cargo-allow allow --version: {err}"))
        });

    assert_status("allow --version", &result, true);
    assert_stderr_empty("allow --version", &result, "should not emit stderr");
    assert_eq!(
        String::from_utf8_lossy(&result.stdout).trim(),
        format!("cargo-allow {}", env!("CARGO_PKG_VERSION"))
    );
}

fn cargo_allow_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

fn assert_status(command: &str, result: &Output, should_succeed: bool) {
    assert_eq!(
        result.status.success(),
        should_succeed,
        "{command} status mismatch: stdout=`{}` stderr=`{}`",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

fn assert_stderr_empty(command: &str, result: &Output, message: &str) {
    assert!(
        result.stderr.is_empty(),
        "{command} {message}: `{}`",
        String::from_utf8_lossy(&result.stderr)
    );
}
