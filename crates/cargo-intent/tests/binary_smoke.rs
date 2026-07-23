use std::process::Command;

#[test]
fn binary_help_and_version() -> Result<(), String> {
    let bin = std::env::var("CARGO_BIN_EXE_cargo-intent")
        .map_err(|_| "CARGO_BIN_EXE_cargo-intent not set".to_string())?;
    let help = Command::new(&bin)
        .arg("--help")
        .output()
        .map_err(|err| err.to_string())?;
    if !help.status.success() {
        return Err("cargo-intent --help failed".to_string());
    }
    let help_text = String::from_utf8_lossy(&help.stdout);
    if !help_text.contains("cargo-intent") {
        return Err("help missing product name".to_string());
    }
    let version = Command::new(&bin)
        .arg("--version")
        .output()
        .map_err(|err| err.to_string())?;
    if !version.status.success() {
        return Err("cargo-intent --version failed".to_string());
    }
    Ok(())
}

#[test]
fn identity_subcommand_emits_json() -> Result<(), String> {
    let bin = std::env::var("CARGO_BIN_EXE_cargo-intent")
        .map_err(|_| "CARGO_BIN_EXE_cargo-intent not set".to_string())?;
    let output = Command::new(&bin)
        .args(["--format", "json", "identity"])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err("cargo-intent identity failed".to_string());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    if !text.contains("cargo-intent.product-identity.v1") {
        return Err("identity json missing schema id".to_string());
    }
    Ok(())
}

#[test]
fn change_status_subcommand_emits_json() -> Result<(), String> {
    let bin = std::env::var("CARGO_BIN_EXE_cargo-intent")
        .map_err(|_| "CARGO_BIN_EXE_cargo-intent not set".to_string())?;
    let output = Command::new(&bin)
        .args([
            "--format",
            "json",
            "change",
            "status",
            "--staged",
            "--phase",
            "precommit",
        ])
        .output()
        .map_err(|err| err.to_string())?;
    let text = String::from_utf8_lossy(&output.stdout);
    if !text.contains("cargo-intent.change-status.v1") {
        return Err("change status json missing schema id".to_string());
    }
    Ok(())
}
