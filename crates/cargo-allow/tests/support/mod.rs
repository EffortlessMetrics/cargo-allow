use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

pub fn cargo_allow_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

pub fn assert_status(command: &str, result: &Output, should_succeed: bool) {
    assert_eq!(
        result.status.success(),
        should_succeed,
        "{command} status mismatch: stdout=`{}` stderr=`{}`",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

pub fn assert_stdout_empty(command: &str, result: &Output, message: &str) {
    assert!(
        result.stdout.is_empty(),
        "{command} {message}: `{}`",
        String::from_utf8_lossy(&result.stdout)
    );
}

pub fn assert_stderr_empty(command: &str, result: &Output, message: &str) {
    assert!(
        result.stderr.is_empty(),
        "{command} {message}: `{}`",
        String::from_utf8_lossy(&result.stderr)
    );
}

pub fn assert_file_contains(path: &std::path::Path, needle: &str, message: &str) {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read {}: {err}", path.display())));
    assert!(contents.contains(needle), "{message}");
}

pub fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|err| std::panic::panic_any(format!("system clock: {err}")))
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create temp root: {err}")));
    root
}

pub fn remove_temp_root(root: PathBuf) {
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => std::panic::panic_any(format!("remove temp root {}: {err}", root.display())),
    }
}
