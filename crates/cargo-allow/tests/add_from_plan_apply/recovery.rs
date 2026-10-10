//! Execute the diagnostic's advertised command, including its shell syntax.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

use super::support::{cargo_allow_command, remove_temp_root, temp_root};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const DEFAULT_POLICY: &str = "policy/allow.toml";
const SELECTED_POLICY: &str = "policies/selected ' policy.toml";
const RECORDED_PLAN: &str = "saved plans/original plan.json";
const SOURCE_TEXT: &str = "pub fn load() -> usize { Some(1).unwrap() }\n";
const SOURCE_PATH: &str = if cfg!(windows) {
    "src/source & (literal).rs"
} else {
    "src/source ' $(touch SHELL-INJECTION) ;.rs"
};

fn require(condition: bool, message: &str) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

fn require_success(name: &str, output: &Output) -> TestResult {
    require(
        output.status.success(),
        &format!(
            "{name}: status={}, stdout={:?}, stderr={:?}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        ),
    )
}

// Affect only these child processes, including the shell's cargo-allow child.
// Another test may be using the parent process's repository environment.
fn isolated(mut command: Command) -> Command {
    for variable in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "CARGO_ALLOW_ROOT",
        "ENV",
        "BASH_ENV",
    ] {
        command.env_remove(variable);
    }
    command.env("NO_COLOR", "1");
    command
}

fn git(root: &Path, args: &[&str]) -> TestResult<Output> {
    let output = isolated(Command::new("git"))
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    require_success("git recovery fixture", &output)?;
    Ok(output)
}

pub(super) fn printed_regeneration_command<'a>(
    stderr: &'a str,
    recorded_plan: &Path,
) -> TestResult<&'a str> {
    let (_, tail) = stderr
        .split_once("; regenerate with ")
        .ok_or("stale binding rejection lacked regeneration advice")?;
    let printed = tail.lines().next().ok_or("empty regeneration command")?;
    // Also read the old one-line diagnostic so an old-source RED control tests
    // its actual argv, rather than failing only on the new line boundary.
    let old_suffix = format!(
        " ({} already exists and add-finding plans are never overwritten)",
        recorded_plan.display()
    );
    let printed = printed.strip_suffix(&old_suffix).unwrap_or(printed);
    require(
        printed.starts_with("cargo-allow why --plan "),
        "unexpected advertised regeneration command",
    )?;
    Ok(printed)
}

pub(super) fn run_printed_command(cwd: &Path, printed: &str) -> TestResult<Output> {
    let binary_dir = Path::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .parent()
        .ok_or("cargo-allow test binary has no parent directory")?;
    let mut search_path = vec![binary_dir.to_path_buf()];
    if let Some(ambient_path) = std::env::var_os("PATH") {
        search_path.extend(std::env::split_paths(&ambient_path));
    }
    #[cfg(windows)]
    let shell = {
        use std::os::windows::process::CommandExt;

        let mut command = Command::new("cmd.exe");
        // The existing renderer targets cmd.exe, not PowerShell. raw_arg avoids
        // Rust's C-argv escaping changing the command under test; /S removes
        // only the wrapper quotes. The printed command is otherwise unchanged.
        command
            .args(["/D", "/V:OFF", "/S", "/C"])
            .raw_arg(format!("\"{printed}\""))
            .env("PATHEXT", ".EXE");
        command
    };
    #[cfg(not(windows))]
    let shell = {
        let mut command = Command::new("sh");
        command.arg("-c").arg(printed);
        command
    };
    Ok(isolated(shell)
        .current_dir(cwd)
        .env("PATH", std::env::join_paths(search_path)?)
        .output()?)
}

fn init_repository(root: &Path, include_untracked: bool) -> TestResult {
    git(root, &["init"])?;
    git(root, &["config", "user.email", "fixture@example.com"])?;
    git(root, &["config", "user.name", "fixture"])?;
    git(root, &["config", "core.autocrlf", "false"])?;
    git(root, &["config", "core.hooksPath", ".git/no-hooks"])?;
    for policy in [DEFAULT_POLICY, SELECTED_POLICY] {
        let init = isolated(cargo_allow_command())
            .args(["init", "--root"])
            .arg(root)
            .args(["--config", policy])
            .output()?;
        require_success("init distinct recovery policies", &init)?;
    }
    fs::write(root.join(SOURCE_PATH), SOURCE_TEXT)?;
    git(root, &["add", "--", DEFAULT_POLICY, SELECTED_POLICY])?;
    if !include_untracked {
        git(root, &["add", "--", SOURCE_PATH])?;
    }
    git(root, &["commit", "-q", "-m", "recovery fixture"])?;
    // This distinguishes an accidentally added include-untracked flag even
    // when the selected source itself is tracked.
    fs::write(root.join("untracked note.txt"), "untracked inventory canary")?;
    Ok(())
}

struct RecoveryFixture {
    container: PathBuf,
    root: PathBuf,
    caller: PathBuf,
    paste_cwd: PathBuf,
    include_untracked: bool,
    policy_before: Vec<u8>,
    head_before: Vec<u8>,
    protected: Vec<(PathBuf, Vec<u8>)>,
}

impl RecoveryFixture {
    fn new(include_untracked: bool) -> TestResult<Self> {
        let container = temp_root("add-plan-shell-recovery");
        let root = container.join("selected repo ' & (literal)");
        let caller = container.join("original caller outside repo");
        let paste_cwd = container.join("paste caller outside repo");
        for directory in [
            root.join("src"),
            caller.join("saved plans"),
            paste_cwd.clone(),
        ] {
            fs::create_dir_all(directory)?;
        }
        for cwd in [&caller, &paste_cwd] {
            let probe = isolated(Command::new("git"))
                .current_dir(cwd)
                .args(["rev-parse", "--show-toplevel"])
                .output()?;
            require(
                !probe.status.success(),
                "recovery caller must be outside any Git tree",
            )?;
        }
        init_repository(&root, include_untracked)?;
        let mut fixture = Self {
            policy_before: fs::read(root.join(SELECTED_POLICY))?,
            head_before: git(&root, &["rev-parse", "HEAD"])?.stdout,
            container,
            root,
            caller,
            paste_cwd,
            include_untracked,
            protected: Vec::new(),
        };
        let original = fixture.generate_plan(1, &fixture.caller.join(RECORDED_PLAN))?;
        require(
            original.pointer("/finding/line").and_then(Value::as_u64) == Some(1)
                && original.pointer("/outcome/status").and_then(Value::as_str) == Some("new")
                && original.pointer("/policy/path").and_then(Value::as_str)
                    == Some(SELECTED_POLICY),
            "original plan must bind the selected policy and New line-1 target",
        )?;
        let first = fixture.caller.join("saved plans/original plan.retry-1.json");
        let second = fixture.caller.join("saved plans/original plan.retry-2.json");
        fs::write(&first, "occupied retry one")?;
        fs::create_dir(&second)?;
        let directory_canary = second.join("keep.txt");
        fs::write(&directory_canary, "occupied retry directory")?;
        fs::write(
            fixture.root.join(SOURCE_PATH),
            format!("// same finding moved\n\n{SOURCE_TEXT}"),
        )?;
        if !include_untracked {
            git(&fixture.root, &["add", "--", SOURCE_PATH])?;
        }
        for path in [
            fixture.root.join(DEFAULT_POLICY),
            fixture.root.join(SOURCE_PATH),
            fixture.root.join("untracked note.txt"),
            fixture.caller.join(RECORDED_PLAN),
            first,
            directory_canary,
        ] {
            fixture.protected.push((path.clone(), fs::read(path)?));
        }
        Ok(fixture)
    }

    fn command(&self, verb: &str) -> Command {
        let mut command = isolated(cargo_allow_command());
        command
            .current_dir(&self.caller)
            .arg(verb)
            .arg("--root")
            .arg(&self.root)
            .args(["--config", SELECTED_POLICY]);
        if self.include_untracked {
            command.arg("--include-untracked");
        }
        command
    }

    fn generate_plan(&self, line: usize, path: &Path) -> TestResult<Value> {
        let output = self
            .command("why")
            .args(["--kind", "panic", "--path", SOURCE_PATH, "--line"])
            .arg(line.to_string())
            .arg("--plan")
            .arg(path)
            .output()?;
        require_success("explicit selected-context plan control", &output)?;
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }

    fn add(&self, plan_path: &Path, receipt_path: &Path) -> TestResult<Output> {
        Ok(self
            .command("add")
            .arg("--from-plan")
            .arg(plan_path)
            .args([
                "--owner",
                "fixture",
                "--reason",
                "context-safe plan recovery fixture",
                "--update",
                "--summary-format",
                "json",
                "--summary-output",
            ])
            .arg(receipt_path)
            .output()?)
    }

    fn require_preserved(&self) -> TestResult {
        for (path, bytes) in &self.protected {
            require(
                fs::read(path)? == *bytes,
                &format!("recovery changed protected file {}", path.display()),
            )?;
        }
        require(
            git(&self.root, &["rev-parse", "HEAD"])?.stdout == self.head_before,
            "recovery must not change fixture HEAD",
        )?;
        for cwd in [&self.root, &self.caller, &self.paste_cwd] {
            require(
                !cwd.join("SHELL-INJECTION").exists(),
                "quoted source-path text must not execute as a shell command",
            )?;
        }
        Ok(())
    }

    fn apply_and_replay(&self, retry_path: &Path) -> TestResult {
        let retry_before = fs::read(retry_path)?;
        let receipt_path = self.caller.join("application receipt.json");
        let applied = self.add(retry_path, &receipt_path)?;
        require_success("apply regenerated selected-context plan", &applied)?;
        let policy_after = fs::read(self.root.join(SELECTED_POLICY))?;
        let receipt: Value = serde_json::from_slice(&fs::read(&receipt_path)?)?;
        require(
            policy_after != self.policy_before
                && receipt.get("target_ledger").and_then(Value::as_str) == Some(SELECTED_POLICY)
                && receipt.get("targeted_recheck").and_then(Value::as_str) == Some("matched"),
            "application must receipt the same finding in the selected policy",
        )?;
        let matched_path = self.caller.join("matched report.json");
        let matched = self
            .command("why")
            .args([
                "--kind",
                "panic",
                "--path",
                SOURCE_PATH,
                "--line",
                "3",
                "--format",
                "json",
                "--output",
            ])
            .arg(&matched_path)
            .output()?;
        require_success("inspect recovered finding", &matched)?;
        let matched_report: Value = serde_json::from_slice(&fs::read(matched_path)?)?;
        require(
            matched_report.pointer("/outcome/status").and_then(Value::as_str) == Some("matched"),
            "the recovered finding must be matched after application",
        )?;
        let replay_receipt = self.caller.join("replay must not exist.json");
        let replay = self.add(retry_path, &replay_receipt)?;
        let replay_text = String::from_utf8(replay.stderr)?;
        require(
            replay.status.code() == Some(2)
                && replay_text.contains("E0001_USAGE")
                && replay_text.contains("status `matched`")
                && replay_text.contains("(policy unchanged)")
                && !replay_text.contains("regenerate ")
                && !replay_receipt.exists(),
            &format!("matched replay must refuse without regeneration advice: {replay_text}"),
        )?;
        require(
            fs::read(self.root.join(SELECTED_POLICY))? == policy_after
                && fs::read(retry_path)? == retry_before,
            "replay must preserve the applied policy and regenerated plan bytes",
        )?;
        self.require_preserved()
    }
}

fn context_recovery_runs_verbatim(include_untracked: bool) -> TestResult {
    let fixture = RecoveryFixture::new(include_untracked)?;
    let refused_receipt = fixture.caller.join("refused must not exist.json");
    let refused = fixture.add(Path::new(RECORDED_PLAN), &refused_receipt)?;
    let rejection = String::from_utf8(refused.stderr)?;
    require(
        refused.status.code() == Some(2)
            && rejection.contains("E0001_USAGE")
            && rejection.contains("(policy unchanged)")
            && rejection.contains("source inventory changed since the plan was generated")
            && !refused_receipt.exists(),
        &format!("original stale plan must refuse before mutation: {rejection}"),
    )?;
    fixture.require_preserved()?;
    require(
        fs::read(fixture.root.join(SELECTED_POLICY))? == fixture.policy_before,
        "stale refusal changed the selected policy",
    )?;
    let printed = printed_regeneration_command(&rejection, Path::new(RECORDED_PLAN))?;
    let retry_path = fixture.caller.join("saved plans/original plan.retry-3.json");
    require(
        !retry_path.exists(),
        "third retry path must initially be unused",
    )?;
    // Change cwd again before pasting: output belongs beside the original plan,
    // and neither cwd is the selected repository. Execute the string unchanged.
    let hinted = run_printed_command(&fixture.paste_cwd, printed)?;
    fixture.require_preserved()?;
    let control_path = fixture.caller.join("saved plans/explicit control.json");
    let control = fixture.generate_plan(3, &control_path)?;
    require(
        fs::read(fixture.root.join(SELECTED_POLICY))? == fixture.policy_before
            && control.pointer("/finding/line").and_then(Value::as_u64) == Some(3)
            && control.pointer("/outcome/status").and_then(Value::as_str) == Some("new")
            && control.pointer("/policy/path").and_then(Value::as_str) == Some(SELECTED_POLICY),
        "independent exact live-line control must succeed without mutation",
    )?;
    // Keep the positive control above the expected-red assertion so failures
    // distinguish a broken printed command from an unplannable source fixture.
    eprintln!(
        "include_untracked={include_untracked}; explicit selected-context line-3 control succeeded; protected bytes unchanged; printed={printed:?}; hint_status={}; hint_stderr={:?}",
        hinted.status,
        String::from_utf8_lossy(&hinted.stderr),
    );
    require_success("verbatim context-safe regeneration", &hinted)?;
    let regenerated: Value = serde_json::from_slice(&fs::read(&retry_path)?)?;
    for pointer in [
        "/repository",
        "/inventory_basis_identity",
        "/policy",
        "/finding",
        "/outcome",
    ] {
        require(
            regenerated.pointer(pointer) == control.pointer(pointer)
                && control.pointer(pointer).is_some(),
            &format!("printed recovery changed the selected live binding {pointer}"),
        )?;
    }
    fixture.apply_and_replay(&retry_path)?;
    remove_temp_root(fixture.container);
    Ok(())
}

#[test]
fn tracked_stale_plan_recovery_preserves_context_through_shell() -> TestResult {
    context_recovery_runs_verbatim(false)
}

#[test]
fn untracked_stale_plan_recovery_preserves_context_through_shell() -> TestResult {
    context_recovery_runs_verbatim(true)
}
