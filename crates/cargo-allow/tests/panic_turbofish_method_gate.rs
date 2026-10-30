//! #4671: a fixture whose only Rust code is a turbofish on a method receiver
//! must fail `check --kind panic --mode no-new`. Before the fix the scanner
//! recognized `v.unwrap::<u8>()` as nothing, so the gate exited 0 and the
//! exception could never enter the ledger.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn cargo_allow_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create temp root: {err}")));
    root
}

fn remove_temp_root(root: PathBuf) {
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => std::panic::panic_any(format!("remove temp root {}: {err}", root.display())),
    }
}

#[test]
fn check_no_new_fails_when_the_only_finding_is_a_method_receiver_turbofish() {
    let root = temp_root("panic-turbofish-method-gate");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    // The only Rust code is the turbofish spelling (#4671 repro).
    fs::write(
        root.join("src/lib.rs"),
        "pub fn load(v: Option<u8>) -> u8 { v.unwrap::<u8>() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        r#"schema_version = "0.1"
policy = "cargo-allow"

[requirements]
owner_required = true
reason_required = true
classification_required = true
evidence_required = false
expires_or_review_after_required = true
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));

    let check = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg("policy/allow.toml")
        .arg("--kind")
        .arg("panic")
        .arg("--mode")
        .arg("no-new")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run check: {err}")));

    let stdout = String::from_utf8_lossy(&check.stdout);
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(
        !check.status.success(),
        "check --kind panic --mode no-new must fail on the turbofish-only fixture; \
         stdout=`{stdout}` stderr=`{stderr}`"
    );
    assert!(
        stdout.contains("unwrap") || stderr.contains("unwrap"),
        "the gate output must name the unledgered unwrap finding; \
         stdout=`{stdout}` stderr=`{stderr}`"
    );

    remove_temp_root(root);
}
