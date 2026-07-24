use std::process::Command;

#[test]
fn binary_help_and_version() -> Result<(), String> {
    let bin = std::env::var("CARGO_BIN_EXE_cargo-proof")
        .map_err(|_| "CARGO_BIN_EXE_cargo-proof not set".to_string())?;
    let help = Command::new(&bin)
        .arg("--help")
        .output()
        .map_err(|err| err.to_string())?;
    if !help.status.success() {
        return Err("cargo-proof --help failed".to_string());
    }
    let help_text = String::from_utf8_lossy(&help.stdout);
    if !help_text.contains("cargo-proof") {
        return Err("help missing product name".to_string());
    }
    let version = Command::new(&bin)
        .arg("--version")
        .output()
        .map_err(|err| err.to_string())?;
    if !version.status.success() {
        return Err("cargo-proof --version failed".to_string());
    }
    Ok(())
}

#[test]
fn identity_subcommand_emits_json() -> Result<(), String> {
    let bin = std::env::var("CARGO_BIN_EXE_cargo-proof")
        .map_err(|_| "CARGO_BIN_EXE_cargo-proof not set".to_string())?;
    let output = Command::new(&bin)
        .args(["--format", "json", "identity"])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err("cargo-proof identity failed".to_string());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    if !text.contains("cargo-proof.product-identity.v1") {
        return Err("identity json missing schema id".to_string());
    }
    Ok(())
}

#[test]
fn dry_run_subcommand_emits_structured_argv() -> Result<(), String> {
    let bin = std::env::var("CARGO_BIN_EXE_cargo-proof")
        .map_err(|_| "CARGO_BIN_EXE_cargo-proof not set".to_string())?;
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/cargo-proof/proof-plan-smoke-v1.toml");
    let output = Command::new(&bin)
        .args([
            "dry-run",
            "--proof-plan",
            fixture
                .to_str()
                .ok_or_else(|| "fixture path not utf8".to_string())?,
        ])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("cargo-proof dry-run failed: {stderr}"));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    if !text.contains("[structured argv]") {
        return Err("dry-run output missing structured argv marker".to_string());
    }
    Ok(())
}
