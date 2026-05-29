use std::fs;
use std::process::Command;

#[test]
fn audit_with_output_file_does_not_emit_human_status_to_stderr() {
    let root = temp_root("audit-output");
    fs::write(root.join("tracked.txt"), "tracked\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write tracked file: {err}")));
    let output = root.join("audit.json");

    let result = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .arg("audit")
        .arg("--root")
        .arg(&root)
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow audit: {err}")));

    assert!(
        result.status.success(),
        "audit should pass: stdout=`{}` stderr=`{}`",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        result.stderr.is_empty(),
        "audit --output should not emit human status to stderr: `{}`",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        fs::read_to_string(&output)
            .unwrap_or_else(|err| std::panic::panic_any(format!("read audit output: {err}")))
            .contains("\"schema_id\": \"cargo-allow.report.v1\""),
        "audit output should be a report artifact"
    );

    remove_temp_root(root);
}

fn temp_root(label: &str) -> std::path::PathBuf {
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

fn remove_temp_root(root: std::path::PathBuf) {
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => std::panic::panic_any(format!("remove temp root {}: {err}", root.display())),
    }
}
