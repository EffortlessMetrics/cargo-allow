//! Contract for the composite GitHub action surface (`action.yml`).
//!
//! #4669 design: the exact commit digests live in `action.yml` itself and
//! git history is the digest ledger. This hand-maintained contract pins
//! which actions the closed read-only surface names, that every `uses:`
//! ref is an immutable 40-hex commit digest with a version comment, and
//! that the action never mutates GitHub or repository state. A Dependabot
//! digest bump therefore needs no Rust edit — this contract follows the
//! file instead of pinning byte sequences that legitimate automation must
//! change (#4666).

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
        Err(format!(
            "action contract is missing `{needle}` — the closed pinned surface names exactly these actions; an action swap requires a deliberate contract edit."
        ))
    }
}

fn uses_shape_failure(line: &str) -> String {
    format!(
        "action.yml `uses:` ref must pin an immutable 40-hex commit digest with a version comment (repository pin law); got `{line}`. A Dependabot digest bump needs no Rust edit — this contract follows the file."
    )
}

/// Repository pin law for one `uses:` line: after the YAML list dash, the
/// ref must name an owner/repo, pin an immutable 40-character lowercase
/// hex commit digest, and carry a non-empty version comment. A local `./`
/// ref, mutable tag, branch name, short digest, or missing comment fails
/// loudly.
fn require_uses_digest_pin(line: &str) -> Result<(), String> {
    let trimmed = line.trim();
    let body = trimmed.strip_prefix("- ").unwrap_or(trimmed).trim();
    let rest = body
        .strip_prefix("uses:")
        .ok_or_else(|| uses_shape_failure(line))?;
    let (pin, comment) = match rest.trim_start().split_once('#') {
        Some((pin, comment)) => (pin.trim_end(), comment.trim()),
        None => (rest.trim_start().trim_end(), ""),
    };
    let Some((repo, digest)) = pin.split_once('@') else {
        return Err(uses_shape_failure(line));
    };
    let digest_immutable = digest.len() == 40
        && digest
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character));
    let named_owner_repo = repo
        .split_once('/')
        .is_some_and(|(owner, name)| !owner.is_empty() && !name.is_empty());
    let no_internal_whitespace = !pin.contains(char::is_whitespace);
    if digest_immutable && named_owner_repo && no_internal_whitespace && !comment.is_empty() {
        Ok(())
    } else {
        Err(uses_shape_failure(line))
    }
}

fn uses_lines(action: &str) -> impl Iterator<Item = &str> {
    action.lines().filter(|line| {
        let trimmed = line.trim();
        let body = trimmed.strip_prefix("- ").unwrap_or(trimmed).trim();
        body.starts_with("uses:")
    })
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
        "dtolnay/rust-toolchain@",
        "actions/upload-artifact@",
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

#[test]
fn action_contract_pins_every_uses_to_an_immutable_digest_with_version_comment()
-> Result<(), Box<dyn Error>> {
    let action = fs::read_to_string(repository_root()?.join("action.yml"))?;
    let mut pinned_steps = 0usize;
    for line in uses_lines(&action) {
        require_uses_digest_pin(line)?;
        pinned_steps += 1;
    }
    if pinned_steps == 0 {
        return Err(
            "action contract fixture premise: action.yml must declare at least one `uses:` step"
                .into(),
        );
    }
    Ok(())
}

#[test]
fn uses_shape_accepts_a_digest_pin_with_version_comment() {
    let line = "- uses: dtolnay/rust-toolchain@f133eefe930d61f0d9371efd474daf0125ed3dd1 # 1.95.0";
    assert_eq!(require_uses_digest_pin(line), Ok(()));
}

#[test]
fn uses_shape_rejects_a_mutable_tag_ref() {
    let line = "- uses: actions/upload-artifact@v7 # upload artifacts";
    assert!(require_uses_digest_pin(line).is_err());
}

#[test]
fn uses_shape_rejects_a_branch_ref() {
    let line = "- uses: actions/checkout@main # checkout the repository";
    assert!(require_uses_digest_pin(line).is_err());
}

#[test]
fn uses_shape_rejects_a_missing_version_comment() {
    let line = "- uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a";
    assert!(require_uses_digest_pin(line).is_err());
}

#[test]
fn uses_shape_rejects_a_short_digest() {
    let line = "- uses: actions/upload-artifact@043fb46d # v7.0.1";
    assert!(require_uses_digest_pin(line).is_err());
}

#[test]
fn uses_shape_rejects_a_local_path_ref() {
    let line = "- uses: ./";
    assert!(require_uses_digest_pin(line).is_err());
}

#[test]
fn dependabot_digest_bump_stays_green_without_a_rust_edit() -> Result<(), String> {
    // #4666 shape: Dependabot rewrites only the digest and version comment
    // of the upload-artifact step. The retired exact-digest needle went red
    // on exactly this change; the name needle plus the digest-shape pin
    // follow the file instead.
    let bumped =
        "- uses: actions/upload-artifact@1f5a5d6f0e9b3c7d2a4b6c8d0e2f4a6c8e0b2d4f # v8.0.0\n";
    require_contract(bumped, "actions/upload-artifact@")?;
    require_uses_digest_pin(bumped)?;
    assert!(
        !bumped.contains("043fb46d1a93c77aae656e7c1c64a875d1fc6a0a"),
        "fixture premise: the bumped digest must differ from the retired exact needle"
    );
    Ok(())
}
