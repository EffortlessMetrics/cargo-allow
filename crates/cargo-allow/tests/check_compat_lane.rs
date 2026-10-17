use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn unique_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}

fn remove_root(root: &Path) {
    let _ = fs::remove_dir_all(root);
}

fn network_policy_fixture() -> &'static str {
    r#"schema_version = 1
policy = "network-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "net-crates-io-fetch"
destination = "crates.io"
auth_required = false
lane = "build"
owner = "release"
reason = "cargo fetch resolves and downloads crate dependencies."
created = "2026-05-09"
expires = "permanent"
"#
}

fn write_compat_fixture(label: &str) -> std::path::PathBuf {
    let root = unique_root(label);
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(
        root.join("policy/network-allowlist.toml"),
        network_policy_fixture(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write legacy policy: {err}")));
    fs::write(root.join("src/lib.rs"), "pub fn ok() {}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
    root
}

#[test]
fn check_compat_binds_legacy_policy_digest_and_guards_the_selected_policy() {
    // #4590/#4593: the compat world's inventory facts never carried a policy
    // digest, so `check --compat --emit` always failed the artifact-set
    // identity gate and `check --compat --receipt` dropped policy-byte
    // provenance; #4591: with no canonical policy the collision guard
    // no-opped and artifact files landed beside the live legacy ledger.
    let root = write_compat_fixture("check-compat-digest-guard");
    git_init(&root);

    // Digest + guard: the emit succeeds with the legacy policy's own digest
    // bound, and the collision guard protects the legacy file itself.
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .arg("check")
        .arg("--compat")
        .arg("--kind")
        .arg("network")
        .arg("--root")
        .arg(&root)
        .arg("--artifact-dir")
        .arg(root.join("policy"))
        .arg("--emit")
        .arg("json")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run compat check: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "a policy-colliding compat emit must fail: {stderr}"
    );
    assert!(
        stderr.contains("would contain the selected policy file"),
        "the collision guard must protect the compat-selected legacy policy: {stderr}"
    );
    assert!(
        !root.join("policy/check-json.json").is_file(),
        "no artifact may be written into the legacy policy directory"
    );

    // Safe artifact dir: the emit succeeds with the legacy policy digest
    // bound in both the manifest identity and the receipt provenance.
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .arg("check")
        .arg("--compat")
        .arg("--kind")
        .arg("network")
        .arg("--root")
        .arg(&root)
        .arg("--artifact-dir")
        .arg(root.join("artifacts"))
        .arg("--emit")
        .arg("json")
        .arg("--receipt")
        .arg(root.join("compat-receipt.json"))
        .arg("--format")
        .arg("json")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run compat check: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "a safe compat emit must succeed: {stderr}"
    );

    let manifest_text = fs::read_to_string(root.join("artifacts/check-artifact_set_manifest.json"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read manifest: {err}")));
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)
        .unwrap_or_else(|err| std::panic::panic_any(format!("manifest must be JSON: {err}")));
    let identity = manifest
        .get("resolved_config_identity")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    assert!(
        identity.starts_with("sha256:v1:"),
        "resolved_config_identity must bind the legacy policy digest: {identity}"
    );

    let receipt_text = fs::read_to_string(root.join("compat-receipt.json"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read receipt: {err}")));
    assert!(
        receipt_text.contains("policy_digest"),
        "the compat receipt must carry policy-byte provenance: {receipt_text}"
    );

    remove_root(&root);
}

fn git_init(root: &std::path::Path) {
    let git_invocations: [&[&str]; 5] = [
        &["init", "-q"],
        &["config", "user.email", "cargo-allow@example.invalid"],
        &["config", "user.name", "cargo-allow test"],
        &["add", "."],
        &["commit", "-m", "compat fixture"],
    ];
    for git_args in git_invocations {
        std::process::Command::new("git")
            .args(git_args)
            .current_dir(root)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("git {git_args:?}: {err}")));
    }
}
