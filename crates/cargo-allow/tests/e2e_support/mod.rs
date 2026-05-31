use std::fs;
use std::process::Output;

use crate::support::{assert_status, assert_stderr_empty, assert_stdout_empty};

pub fn assert_success_and_quiet(command: &str, result: &Output) {
    assert_status(command, result, true);
    assert_stdout_empty(
        command,
        result,
        "should not emit primary output when output files are configured",
    );
    assert_stderr_empty(
        command,
        result,
        "should not emit side-channel status when output files are configured",
    );
}

pub fn write_file(root: &std::path::Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap_or_else(|| {
        std::panic::panic_any(format!("fixture path has no parent: {}", path.display()))
    }))
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("create fixture parent {}: {err}", path.display()))
    });
    fs::write(&path, contents)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write {}: {err}", path.display())));
}
