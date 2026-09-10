//! Real Cargo dispatch to the just-built sibling binary, without installation.
//!
//! Every invocation is bounded: a hung child is terminated and reaped
//! at a named deadline instead of hanging the suite (#4171). Cleanup
//! is fallible on the successful path and best-effort only in `Drop`.

use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

type TestResult = Result<(), Box<dyn Error>>;

/// Generous for loaded CI hosts; never an indefinite wait.
const DEADLINE: Duration = Duration::from_secs(120);
/// Short deadline for the controlled hanging-child cases.
const HANG_DEADLINE: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
/// Marker that turns this test executable into a hanging child.
const HANG_CHILD_ENV: &str = "CARGO_DISPATCH_HANG_CHILD";

struct Fixture {
    root: PathBuf,
    cargo_home: PathBuf,
    binary: PathBuf,
}

impl Fixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cargo-intent-dispatch-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root)?;
        let cargo_home = root.join("cargo-home");
        let binary = cargo_home
            .join("bin")
            .join(format!("cargo-intent{}", std::env::consts::EXE_SUFFIX));
        let fixture = Self {
            root,
            cargo_home,
            binary,
        };
        fs::create_dir_all(fixture.cargo_home.join("bin"))?;
        fs::create_dir(fixture.root.join("intent"))?;
        fs::copy(env!("CARGO_BIN_EXE_cargo-intent"), &fixture.binary)?;
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
            .env_remove("CARGO_ALIAS_INTENT");
        command
    }

    /// Fallible cleanup for the successful path: errors surface to the
    /// test instead of being dropped.
    fn cleanup(self) -> Result<(), Box<dyn Error>> {
        self.cleanup_with(|path| fs::remove_dir_all(path))
    }

    /// The cleanup core with an injectable remover so the retry and
    /// error-propagation behavior has a deterministic control.
    fn cleanup_with<F>(self, mut remove: F) -> Result<(), Box<dyn Error>>
    where
        F: FnMut(&Path) -> std::io::Result<()>,
    {
        let mut last: Option<std::io::Error> = None;
        for _ in 0..3 {
            match remove(&self.root) {
                Ok(()) => return Ok(()),
                Err(error) => {
                    last = Some(error);
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
        }
        Err(format!(
            "fixture cleanup failed for {}: {}",
            self.root.display(),
            last.map(|error| error.to_string()).unwrap_or_default()
        )
        .into())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Terminate and reap the fixture-owned process tree: on Windows Cargo
/// dispatch spawns (not execs) the tool, so the tree kill also reaps a
/// grandchild; on Unix Cargo replaces itself for external subcommands,
/// so the direct pid is the tool.
fn kill_tree(child: &mut Child) {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Bounded `Command::output()`: spawn with piped streams, poll to the
/// deadline, terminate and reap the fixture-owned tree on timeout. The
/// fixture commands emit well under one pipe buffer, so reading after
/// exit cannot deadlock.
fn bounded(command: &mut Command, deadline: Duration) -> Result<Output, Box<dyn Error>> {
    let started = Instant::now();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("spawn {}: {error}", command.get_program().display()))?;
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => break,
            Ok(None) => {
                if started.elapsed() >= deadline {
                    kill_tree(&mut child);
                    return Err(format!(
                        "bounded timeout: {} exceeded {deadline:?} and was terminated",
                        command.get_program().display()
                    )
                    .into());
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(error) => {
                kill_tree(&mut child);
                return Err(format!("wait on fixture child: {error}").into());
            }
        }
    }
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    if let Some(mut stream) = child.stdout.take() {
        stream.read_to_end(&mut stdout)?;
    }
    if let Some(mut stream) = child.stderr.take() {
        stream.read_to_end(&mut stderr)?;
    }
    let status = child.wait()?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
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

/// A hanging child loops forever on its own clock; the bounded runner
/// must terminate and reap it at the deadline.
fn hang_here_if_selected() {
    if std::env::var_os(HANG_CHILD_ENV).is_some() {
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}

#[test]
fn cargo_dispatch_preserves_identity_help_version_and_argument_boundaries() -> TestResult {
    hang_here_if_selected();
    let fixture = Fixture::new()?;
    let cases: &[&[&str]] = &[
        &["--version"],
        &["--help"],
        &["--format", "json", "identity"],
        &["--root", "intent", "--format", "json", "identity"],
    ];
    for args in cases {
        let direct = successful(
            bounded(fixture.direct().args(*args), DEADLINE)?,
            "direct binary",
        )?;
        let through_cargo = successful(
            bounded(fixture.cargo().arg("intent").args(*args), DEADLINE)?,
            "Cargo dispatch",
        )?;
        if direct != through_cargo {
            return Err(format!("Cargo changed output for {args:?}").into());
        }
    }

    let version = successful(
        bounded(fixture.direct().arg("--version"), DEADLINE)?,
        "version",
    )?;
    if String::from_utf8(version)?.trim() != format!("cargo-intent {}", env!("CARGO_PKG_VERSION")) {
        return Err("dispatch fixture must use this package's built version".into());
    }
    let direct_help = successful(
        bounded(fixture.direct().arg("--help"), DEADLINE)?,
        "direct help",
    )?;
    let cargo_help = successful(
        bounded(fixture.cargo().args(["help", "intent"]), DEADLINE)?,
        "Cargo help",
    )?;
    if direct_help != cargo_help {
        return Err("cargo help must preserve direct help".into());
    }

    for args in [["intent", "identity"], ["unknown-subcommand", "identity"]] {
        let output = bounded(fixture.cargo().arg("intent").args(args), DEADLINE)?;
        if output.status.success() || output.stderr.is_empty() {
            return Err(format!(
                "Cargo dispatch must reject extra prefix/unknown command: {args:?}"
            )
            .into());
        }
    }

    // Successful-path cleanup is fallible and must actually remove the
    // fixture-owned root.
    let root = fixture.root.clone();
    fixture.cleanup()?;
    assert!(!root.exists(), "the fixture root must be removed");

    Ok(())
}

#[test]
fn bounded_runner_terminates_and_reaps_a_hung_direct_child() -> TestResult {
    hang_here_if_selected();
    let fixture = Fixture::new()?;
    let mut hung = Command::new(std::env::current_exe()?);
    hung.env(HANG_CHILD_ENV, "1");
    let error = bounded(&mut hung, HANG_DEADLINE)
        .expect_err("a hung direct child must fail at the bounded deadline");
    let message = error.to_string();
    assert!(
        message.contains("bounded timeout"),
        "the timeout error is named: {message}"
    );

    // The hung child was reaped before the error returned, so the
    // fixture-owned root is still removable by the fallible cleanup.
    let root = fixture.root.clone();
    fixture.cleanup()?;
    assert!(!root.exists());
    Ok(())
}

#[test]
fn bounded_runner_terminates_a_hung_cargo_dispatch_tree() -> TestResult {
    hang_here_if_selected();
    let fixture = Fixture::new()?;
    // The copied sibling hangs as a Cargo external subcommand: on Unix
    // Cargo execs it (the direct pid is the tool); on Windows Cargo
    // spawns and waits, so the tree kill reaps the grandchild.
    let hanging_sibling = fixture
        .cargo_home
        .join("bin")
        .join(format!("cargo-intent-hang{}", std::env::consts::EXE_SUFFIX));
    fs::copy(std::env::current_exe()?, &hanging_sibling)?;

    let mut dispatch_command = fixture.cargo();
    // The positional filter selects a test whose body hangs, under
    // either Cargo arg-forwarding behavior.
    let dispatch = dispatch_command
        .arg("intent-hang")
        .arg("cargo_dispatch_preserves_identity_help_version_and_argument_boundaries");
    dispatch.env(HANG_CHILD_ENV, "1");
    let error = bounded(dispatch, HANG_DEADLINE)
        .expect_err("a hung Cargo dispatch must fail at the bounded deadline");
    assert!(
        error.to_string().contains("bounded timeout"),
        "the timeout error is named: {error}"
    );

    // Nothing fixture-owned may still hold the copied executable:
    // successful removal proves the tree was reaped.
    let root = fixture.root.clone();
    fixture.cleanup()?;
    assert!(!root.exists());
    Ok(())
}

#[test]
fn cleanup_reports_a_deterministic_removal_failure() -> TestResult {
    let fixture = Fixture::new()?;
    let root = fixture.root.clone();
    let outcome =
        fixture.cleanup_with(|_path| Err(std::io::Error::other("injected removal failure")));
    assert!(
        outcome.is_err(),
        "explicit cleanup cannot silently return success"
    );
    let message = outcome.unwrap_err().to_string();
    assert!(
        message.contains("injected removal failure"),
        "the removal error is surfaced: {message}"
    );
    let _ = fs::remove_dir_all(&root);
    Ok(())
}
