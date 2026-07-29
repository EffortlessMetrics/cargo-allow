use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Ok(manifest_dir
        .parent()
        .and_then(Path::parent)
        .ok_or("cargo-allow manifest should have a repository root")?
        .to_path_buf())
}

fn require_contract(text: &str, needle: &str) -> Result<(), String> {
    if text.contains(needle) {
        Ok(())
    } else {
        Err(format!("action contract is missing `{needle}`"))
    }
}

#[test]
fn action_contract_has_closed_pinned_read_only_surface() -> Result<(), Box<dyn Error>> {
    let action = fs::read_to_string(repository_root()?.join("action.yml"))?;
    for needle in [
        "using: composite",
        "version:",
        "required: true",
        "default: source",
        "cargo install cargo-allow --version \"${VERSION}\" --locked",
        "dtolnay/rust-toolchain@f133eefe930d61f0d9371efd474daf0125ed3dd1",
        "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
        "allowed values are check, diff, audit, and doctor.",
        "The diff capability requires an exact base revision",
        "Unsupported command",
        "target/cargo-allow-action",
    ] {
        require_contract(&action, needle)?;
    }
    if action.contains("pull_request_target") || action.contains("git push") {
        return Err("action contract must not mutate GitHub or repository state".into());
    }
    Ok(())
}
