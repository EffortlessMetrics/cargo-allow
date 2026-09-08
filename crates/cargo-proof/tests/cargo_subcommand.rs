//! Real Cargo dispatch to the just-built sibling binary, without installation.

use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

type TestResult = Result<(), Box<dyn Error>>;

struct Fixture {
    root: PathBuf,
    cargo_home: PathBuf,
    binary: PathBuf,
}

impl Fixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cargo-proof-dispatch-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root)?;
        let cargo_home = root.join("cargo-home");
        let binary = cargo_home
            .join("bin")
            .join(format!("cargo-proof{}", std::env::consts::EXE_SUFFIX));
        let fixture = Self {
            root,
            cargo_home,
            binary,
        };
        fs::create_dir_all(fixture.cargo_home.join("bin"))?;
        fs::create_dir(fixture.root.join("proof"))?;
        fs::copy(env!("CARGO_BIN_EXE_cargo-proof"), &fixture.binary)?;
        Ok(fixture)
    }

    fn direct(&self) -> Command {
        let mut command = Command::new(&self.binary);
        command.current_dir(&self.root);
        command
    }

    fn cargo(&self) -> Command {
        // Use the Cargo that built this test and its isolated external-tool
        // directory. Neither workspace aliases nor installed siblings supply it.
        let mut command = Command::new(env!("CARGO"));
        command
            .current_dir(&self.root)
            .env("CARGO_HOME", &self.cargo_home)
            .env("PATH", self.cargo_home.join("bin"))
            .env_remove("CARGO_ALIAS_PROOF");
        command
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn successful(output: Output, context: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    if !output.status.success() {
        return Err(format!(
            "{context} failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    if !output.stderr.is_empty() || output.stdout.is_empty() {
        return Err(format!("{context} must emit stdout without stderr").into());
    }
    Ok(output.stdout)
}

#[test]
fn cargo_dispatch_preserves_identity_help_version_and_argument_boundaries() -> TestResult {
    let fixture = Fixture::new()?;
    let cases: &[&[&str]] = &[
        &["--version"],
        &["--help"],
        &["--format", "json", "identity"],
        &["--root", "proof", "--format", "json", "identity"],
    ];
    for args in cases {
        let direct = successful(fixture.direct().args(*args).output()?, "direct binary")?;
        let through_cargo = successful(
            fixture.cargo().arg("proof").args(*args).output()?,
            "Cargo dispatch",
        )?;
        if direct != through_cargo {
            return Err(format!("Cargo changed output for {args:?}").into());
        }
    }

    let version = successful(fixture.direct().arg("--version").output()?, "version")?;
    if String::from_utf8(version)?.trim() != format!("cargo-proof {}", env!("CARGO_PKG_VERSION")) {
        return Err("dispatch fixture must use this package's built version".into());
    }
    let direct_help = successful(fixture.direct().arg("--help").output()?, "direct help")?;
    let cargo_help = successful(
        fixture.cargo().args(["help", "proof"]).output()?,
        "Cargo help",
    )?;
    if direct_help != cargo_help {
        return Err("cargo help must preserve direct help".into());
    }

    for args in [["proof", "identity"], ["unknown-subcommand", "identity"]] {
        let output = fixture.cargo().arg("proof").args(args).output()?;
        if output.status.success() || output.stderr.is_empty() {
            return Err(format!(
                "Cargo dispatch must reject extra prefix/unknown command: {args:?}"
            )
            .into());
        }
    }
    Ok(())
}
