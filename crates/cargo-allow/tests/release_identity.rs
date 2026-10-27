use std::error::Error;
use std::io;
use std::process::Command;

#[test]
fn release_identity_cli_accepts_valid_stable_and_rc() -> Result<(), Box<dyn Error>> {
    // 1. Stable release without tag (derived)
    let output = run_cmd(&["release-identity", "--version", "0.2.0"])?;
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["schema"], "cargo-allow.release-identity.v1");
    assert_eq!(json["result"], "validated");
    assert_eq!(json["version"], "0.2.0");
    assert_eq!(json["tag"], "v0.2.0");
    assert_eq!(json["tag_source"], "derived");
    assert_eq!(json["channel"], "stable");
    assert_eq!(json["rc_ordinal"], serde_json::Value::Null);
    assert_eq!(json["github_prerelease"], false);
    assert_eq!(json["precedence"]["major"], 0);
    assert_eq!(json["precedence"]["minor"], 2);
    assert_eq!(json["precedence"]["patch"], 0);
    assert_eq!(json["precedence"]["is_stable"], true);
    assert_eq!(json["precedence"]["rc_ordinal"], serde_json::Value::Null);

    // 2. Stable release with matching observed tag
    let output = run_cmd(&[
        "release-identity",
        "--version",
        "0.2.0",
        "--observed-tag",
        "v0.2.0",
    ])?;
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["tag_source"], "observed");
    assert_eq!(json["github_prerelease"], false);

    // 3. Numbered RC1 with tag
    let output = run_cmd(&[
        "release-identity",
        "--version",
        "0.2.0-rc.1",
        "--tag",
        "v0.2.0-rc.1",
    ])?;
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["version"], "0.2.0-rc.1");
    assert_eq!(json["tag"], "v0.2.0-rc.1");
    assert_eq!(json["channel"], "release_candidate");
    assert_eq!(json["rc_ordinal"], 1);
    assert_eq!(json["github_prerelease"], true);
    assert_eq!(json["precedence"]["is_stable"], false);
    assert_eq!(json["precedence"]["rc_ordinal"], 1);

    // 4. Numbered RC2 with --observed-tag
    let output = run_cmd(&[
        "release-identity",
        "--version",
        "0.2.0-rc.2",
        "--observed-tag",
        "v0.2.0-rc.2",
    ])?;
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["version"], "0.2.0-rc.2");
    assert_eq!(json["tag"], "v0.2.0-rc.2");
    assert_eq!(json["channel"], "release_candidate");
    assert_eq!(json["rc_ordinal"], 2);
    assert_eq!(json["github_prerelease"], true);

    // 5. Stable rollback baseline 0.1.11
    let output = run_cmd(&["release-identity", "--version", "0.1.11"])?;
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["version"], "0.1.11");
    assert_eq!(json["tag"], "v0.1.11");
    assert_eq!(json["github_prerelease"], false);

    Ok(())
}

#[test]
fn release_identity_cli_rejects_hostile_and_unsupported_inputs() -> Result<(), Box<dyn Error>> {
    const MALFORMED_VERSION_NEXT: &str = "pass a canonical supported version: stable MAJOR.MINOR.PATCH or numbered MAJOR.MINOR.PATCH-rc.N";

    let invalid_inputs = [
        "0.2.0-rc",
        "0.2.0-rc.0",
        "0.2.0-rc.01",
        "0.2.0-beta.1",
        "0.2.0-alpha",
        "0.2.0foo",
        "v0.2.0foo",
        "0.2",
        "01.2.0",
        "0.2.0+build",
        "0.2.0-rc.1+build",
        "0.2.0\n",
        " 0.2.0 ",
    ];

    for invalid in invalid_inputs {
        let output = run_cmd(&["release-identity", "--version", invalid])?;
        assert!(
            !output.status.success(),
            "expected version {invalid:?} to fail validation"
        );
        let stderr = String::from_utf8(output.stderr)?;
        assert!(
            stderr.contains("error[E0002_INVALID_CONFIG]"),
            "expected kind E0002 (invalid_config) for {invalid:?}, got: {stderr}"
        );
        assert_single_typed_next(&stderr, &format!("version {invalid:?}"))?;
        assert!(
            stderr.contains(MALFORMED_VERSION_NEXT),
            "expected the canonical version grammar next step for {invalid:?}, got: {stderr}"
        );
    }

    // Tag mismatches
    let output = run_cmd(&[
        "release-identity",
        "--version",
        "0.2.0",
        "--tag",
        "v0.2.0-rc.1",
    ])?;
    assert!(!output.status.success(), "expected tag mismatch to fail");
    let stderr = String::from_utf8(output.stderr)?;
    assert_single_typed_next(&stderr, "stable tag mismatch")?;
    assert!(
        stderr.contains("the canonical tag for this version is `v0.2.0`;"),
        "expected stable mismatch next step to name `v0.2.0`, got: {stderr}"
    );

    let output = run_cmd(&[
        "release-identity",
        "--version",
        "0.2.0-rc.1",
        "--tag",
        "v0.2.0",
    ])?;
    assert!(!output.status.success(), "expected tag mismatch to fail");
    let stderr = String::from_utf8(output.stderr)?;
    assert_single_typed_next(&stderr, "rc tag mismatch")?;
    assert!(
        stderr.contains("the canonical tag for this version is `v0.2.0-rc.1`;"),
        "expected rc mismatch next step to name `v0.2.0-rc.1`, got: {stderr}"
    );

    Ok(())
}

/// Every release-identity failure must render exactly one typed `Next:` line
/// and none of the policy-config remediation (#4652): the validator is
/// stateless and takes no policy input, so `cargo-allow init`/`--config`
/// advice would point at the wrong repair.
fn assert_single_typed_next(stderr: &str, context: &str) -> Result<(), Box<dyn Error>> {
    assert!(
        !stderr.contains("cargo-allow init"),
        "{context}: policy-init remediation leaked into a stateless validator error: {stderr}"
    );
    assert!(
        !stderr.contains("--config"),
        "{context}: --config remediation leaked into a stateless validator error: {stderr}"
    );
    let next_lines: Vec<&str> = stderr
        .lines()
        .filter(|line| line.starts_with("Next:"))
        .collect();
    assert_eq!(
        next_lines.len(),
        1,
        "{context}: expected exactly one Next line, got: {next_lines:?}"
    );
    Ok(())
}

fn run_cmd(args: &[&str]) -> io::Result<std::process::Output> {
    let bin = env!("CARGO_BIN_EXE_cargo-allow");
    Command::new(bin).args(args).output()
}
