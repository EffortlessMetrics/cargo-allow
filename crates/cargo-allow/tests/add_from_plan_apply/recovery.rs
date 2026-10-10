//! Execute the diagnostic's advertised command, including its shell syntax.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

use super::support::{remove_temp_root, temp_root};
use super::{cargo_allow_command, isolated};

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

#[derive(Clone, Copy, Debug)]
enum PlanPlacement {
    Outside,
    Visible,
    IgnoredDirectory,
    IgnoredOriginal,
    NegatedRetry,
    PolicyIgnored,
}

static RECOVERY_FIXTURE_COUNTER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

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

fn git<S: AsRef<OsStr>>(root: &Path, args: &[S]) -> TestResult<Output> {
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

fn init_repository(root: &Path, source_path: &Path, include_untracked: bool) -> TestResult {
    git(root, &["init"])?;
    git(root, &["config", "user.email", "fixture@example.com"])?;
    git(root, &["config", "user.name", "fixture"])?;
    git(root, &["config", "core.autocrlf", "false"])?;
    // Keep the raw filename oracle independent of macOS Git's Unicode rewriting.
    git(root, &["config", "core.precomposeUnicode", "false"])?;
    git(root, &["config", "core.hooksPath", ".git/no-hooks"])?;
    // The placement controls exercise these explicit rules, independently of
    // a user's global excludes file or repository template.
    let excludes = root.join(".git/fixture-excludes");
    fs::write(&excludes, "")?;
    fs::create_dir_all(root.join(".git/info"))?;
    fs::write(root.join(".git/info/exclude"), "")?;
    git(
        root,
        &[
            OsStr::new("config"),
            OsStr::new("core.excludesFile"),
            excludes.as_os_str(),
        ],
    )?;
    for policy in [DEFAULT_POLICY, SELECTED_POLICY] {
        let init = cargo_allow_command()
            .args(["init", "--root"])
            .arg(root)
            .args(["--config", policy])
            .output()?;
        require_success("init distinct recovery policies", &init)?;
    }
    fs::write(root.join(source_path), SOURCE_TEXT)?;
    git(root, &["add", "--", DEFAULT_POLICY, SELECTED_POLICY])?;
    if !include_untracked {
        git(
            root,
            &[OsStr::new("add"), OsStr::new("--"), source_path.as_os_str()],
        )?;
    }
    git(root, &["commit", "-q", "-m", "recovery fixture"])?;
    // This distinguishes an accidentally added include-untracked flag even
    // when the selected source itself is tracked.
    fs::write(
        root.join("untracked note.txt"),
        "untracked inventory canary",
    )?;
    Ok(())
}

struct RecoveryFixture {
    container: PathBuf,
    root: PathBuf,
    caller: PathBuf,
    paste_cwd: PathBuf,
    source_path: PathBuf,
    plan_path: PathBuf,
    include_untracked: bool,
    policy_before: Vec<u8>,
    head_before: Vec<u8>,
    protected: Vec<(PathBuf, Vec<u8>)>,
}

impl RecoveryFixture {
    fn new(include_untracked: bool, source_path: &Path) -> TestResult<Self> {
        Self::with_placement(include_untracked, source_path, PlanPlacement::Outside)
    }

    fn with_placement(
        include_untracked: bool,
        source_path: &Path,
        placement: PlanPlacement,
    ) -> TestResult<Self> {
        let sequence = RECOVERY_FIXTURE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let container = temp_root(&format!("add-plan-shell-recovery-{sequence}"));
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
        init_repository(&root, source_path, include_untracked)?;
        let plan_path = match placement {
            PlanPlacement::Outside => caller.join(RECORDED_PLAN),
            PlanPlacement::PolicyIgnored => root.join("target/cargo-allow/original plan.json"),
            _ => root.join(RECORDED_PLAN),
        };
        let ignore = match placement {
            PlanPlacement::IgnoredDirectory => "saved plans/\n",
            PlanPlacement::IgnoredOriginal => "saved plans/original plan.json\n",
            PlanPlacement::NegatedRetry => "saved plans/*.json\n!saved plans/*.retry-*.json\n",
            _ => "",
        };
        fs::write(root.join(".gitignore"), ignore)?;
        git(&root, &["add", "--", ".gitignore"])?;
        fs::create_dir_all(plan_path.parent().ok_or("fixture plan has no parent")?)?;
        let tracked = git(&root, &["ls-files", "-z"])?;
        let source_is_tracked = tracked
            .stdout
            .split(|byte| *byte == 0)
            .any(|path| path == source_path.as_os_str().as_encoded_bytes());
        require(
            source_is_tracked != include_untracked,
            "fixture Git inventory must preserve the selected raw source spelling",
        )?;
        let mut fixture = Self {
            policy_before: fs::read(root.join(SELECTED_POLICY))?,
            head_before: git(&root, &["rev-parse", "HEAD"])?.stdout,
            container,
            root,
            caller,
            paste_cwd,
            source_path: source_path.to_path_buf(),
            plan_path,
            include_untracked,
            protected: Vec::new(),
        };
        let original = fixture.generate_plan(1, &fixture.plan_path)?;
        require(
            original.pointer("/finding/line").and_then(Value::as_u64) == Some(1)
                && original.pointer("/outcome/status").and_then(Value::as_str) == Some("new")
                && original.pointer("/policy/path").and_then(Value::as_str)
                    == Some(SELECTED_POLICY),
            "original plan must bind the selected policy and New line-1 target",
        )?;
        let control = fixture.generate_plan(1, &fixture.caller.join("initial control.json"))?;
        require(
            original.pointer("/inventory_basis_identity")
                == control.pointer("/inventory_basis_identity")
                && original
                    .pointer("/inventory_basis_identity")
                    .and_then(Value::as_str)
                    .is_some_and(|identity| identity.starts_with("sha256:v1:")),
            "writing the original plan must not already stale its inventory binding",
        )?;
        let first = fixture
            .plan_path
            .with_file_name("original plan.retry-1.json");
        let second = fixture
            .plan_path
            .with_file_name("original plan.retry-2.json");
        fs::write(&first, "occupied retry one")?;
        fs::create_dir(&second)?;
        let directory_canary = second.join("keep.txt");
        fs::write(&directory_canary, "occupied retry directory")?;
        fs::write(
            fixture.root.join(&fixture.source_path),
            format!("// same finding moved\n\n{SOURCE_TEXT}"),
        )?;
        if !include_untracked {
            git(
                &fixture.root,
                &[
                    OsStr::new("add"),
                    OsStr::new("--"),
                    fixture.source_path.as_os_str(),
                ],
            )?;
        }
        for path in [
            fixture.root.join(DEFAULT_POLICY),
            fixture.root.join(&fixture.source_path),
            fixture.root.join("untracked note.txt"),
            fixture.plan_path.clone(),
            first,
            directory_canary,
            fixture.root.join(".gitignore"),
            fixture.root.join(".git/index"),
            fixture.root.join(".git/HEAD"),
        ] {
            fixture.protected.push((path.clone(), fs::read(path)?));
        }
        Ok(fixture)
    }

    fn command(&self, verb: &str) -> Command {
        let mut command = cargo_allow_command();
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
        self.generate_plan_from(&self.caller, line, path)
    }

    fn generate_plan_from(&self, cwd: &Path, line: usize, path: &Path) -> TestResult<Value> {
        let output = self
            .command("why")
            .current_dir(cwd)
            .args(["--kind", "panic", "--path"])
            .arg(self.root.join(&self.source_path))
            .arg("--line")
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
            .args(["--kind", "panic", "--path"])
            .arg(self.root.join(&self.source_path))
            .args(["--line", "3", "--format", "json", "--output"])
            .arg(&matched_path)
            .output()?;
        require_success("inspect recovered finding", &matched)?;
        let matched_report: Value = serde_json::from_slice(&fs::read(matched_path)?)?;
        require(
            matched_report
                .pointer("/outcome/status")
                .and_then(Value::as_str)
                == Some("matched"),
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

fn context_recovery_runs_verbatim(include_untracked: bool, source_path: &Path) -> TestResult {
    context_recovery_at_placement(include_untracked, source_path, PlanPlacement::Outside)
}

fn context_recovery_at_placement(
    include_untracked: bool,
    source_path: &Path,
    placement: PlanPlacement,
) -> TestResult {
    let fixture = if matches!(placement, PlanPlacement::Outside) {
        RecoveryFixture::new(include_untracked, source_path)?
    } else {
        RecoveryFixture::with_placement(include_untracked, source_path, placement)?
    };
    let refused_receipt = fixture.caller.join("refused must not exist.json");
    let plan_argument = if matches!(placement, PlanPlacement::Outside) {
        Path::new(RECORDED_PLAN)
    } else {
        fixture.plan_path.as_path()
    };
    let refused = fixture.add(plan_argument, &refused_receipt)?;
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
    let printed = printed_regeneration_command(&rejection, plan_argument)?;
    let retry_path = fixture
        .plan_path
        .with_file_name("original plan.retry-3.json");
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
        "include_untracked={include_untracked}; placement={placement:?}; explicit selected-context line-3 control succeeded; protected bytes unchanged; printed={printed:?}; hint_status={}; hint_stderr={:?}",
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
    context_recovery_runs_verbatim(false, Path::new(SOURCE_PATH))
}

#[test]
fn untracked_stale_plan_recovery_preserves_context_through_shell() -> TestResult {
    context_recovery_runs_verbatim(true, Path::new(SOURCE_PATH))
}

#[test]
fn safe_retry_placements_generate_and_apply_from_another_cwd() -> TestResult {
    for (include_untracked, placement) in [
        (false, PlanPlacement::Visible),
        (true, PlanPlacement::Outside),
        (true, PlanPlacement::IgnoredDirectory),
        (true, PlanPlacement::PolicyIgnored),
    ] {
        context_recovery_at_placement(include_untracked, Path::new(SOURCE_PATH), placement)?;
    }
    Ok(())
}

#[test]
fn included_retry_filename_requires_manual_safe_placement() -> TestResult {
    for placement in [PlanPlacement::IgnoredOriginal, PlanPlacement::NegatedRetry] {
        let fixture = RecoveryFixture::with_placement(true, Path::new(SOURCE_PATH), placement)?;
        let retry_path = fixture
            .plan_path
            .with_file_name("original plan.retry-3.json");
        let refused_receipt = fixture.caller.join("refused must not exist.json");
        let refused = fixture.add(&fixture.plan_path, &refused_receipt)?;
        let rejection = String::from_utf8(refused.stderr)?;
        fixture.require_preserved()?;
        require(
            refused.status.code() == Some(2)
                && rejection.contains("E0001_USAGE")
                && rejection.contains("(policy unchanged)")
                && rejection.contains("source inventory changed since the plan was generated")
                && rejection.contains("; regenerate manually: ")
                && rejection.contains("outside the selected source-tree inventory")
                && rejection.contains("same root, selected policy and include-untracked setting")
                && !rejection.contains("; regenerate with ")
                && !retry_path.exists()
                && !refused_receipt.exists()
                && fs::read(fixture.root.join(SELECTED_POLICY))? == fixture.policy_before,
            &format!("included sibling must not receive a self-staling command: {rejection}"),
        )?;
        #[cfg(unix)]
        {
            // A spelling outside the root may alias an eligible directory
            // inside it. The containment check must use the existing parent.
            let alias = fixture.caller.join("source plan alias");
            let parent = fixture
                .plan_path
                .parent()
                .ok_or("original plan lacks parent")?;
            std::os::unix::fs::symlink(parent, &alias)?;
            let alias_plan = alias.join("original plan.json");
            let aliased = fixture.add(&alias_plan, &refused_receipt)?;
            let aliased_text = String::from_utf8(aliased.stderr)?;
            require(
                aliased.status.code() == Some(2)
                    && aliased_text.contains("; regenerate manually: ")
                    && aliased_text.contains("cannot be proved excluded")
                    && !aliased_text.contains("; regenerate with ")
                    && !retry_path.exists()
                    && std::fs::read_link(&alias)? == parent
                    && fs::read(fixture.root.join(SELECTED_POLICY))? == fixture.policy_before,
                "an outside alias must not advertise an eligible in-root retry",
            )?;
            fixture.require_preserved()?;
        }
        // Follow the manual guidance from the second caller, retaining the
        // selected context and using a fresh location outside the inventory.
        let safe_plan = fixture.caller.join("saved plans/manual safe recovery.json");
        let regenerated = fixture.generate_plan_from(&fixture.paste_cwd, 3, &safe_plan)?;
        require(
            regenerated.pointer("/finding/line").and_then(Value::as_u64) == Some(3)
                && regenerated.pointer("/policy/path").and_then(Value::as_str)
                    == Some(SELECTED_POLICY),
            "manual recovery must keep the selected policy and live finding",
        )?;
        fixture.apply_and_replay(&safe_plan)?;
        remove_temp_root(fixture.container);
    }
    Ok(())
}

#[test]
fn missing_tracked_retry_candidate_requires_manual_guidance() -> TestResult {
    let mut fixture =
        RecoveryFixture::with_placement(false, Path::new(SOURCE_PATH), PlanPlacement::Visible)?;
    let candidate = fixture
        .plan_path
        .with_file_name("original plan.retry-3.json");
    fs::write(&candidate, "tracked candidate before deletion")?;
    git(
        &fixture.root,
        &[OsStr::new("add"), OsStr::new("--"), candidate.as_os_str()],
    )?;
    fs::remove_file(&candidate)?;
    // The deliberate staging above is fixture setup. Capture the resulting
    // index bytes before the operation whose no-mutation promise is tested.
    let index_path = fixture.root.join(".git/index");
    for (path, before) in &mut fixture.protected {
        if path == &index_path {
            *before = fs::read(&index_path)?;
        }
    }
    let refused_receipt = fixture.caller.join("refused must not exist.json");
    let refused = fixture.add(&fixture.plan_path, &refused_receipt)?;
    let rejection = String::from_utf8(refused.stderr)?;
    fixture.require_preserved()?;
    require(
        refused.status.code() == Some(2)
            && rejection.contains("(policy unchanged)")
            && rejection.contains("; regenerate manually: ")
            && !rejection.contains("; regenerate with ")
            && !candidate.exists()
            && !refused_receipt.exists()
            && fs::read(fixture.root.join(SELECTED_POLICY))? == fixture.policy_before,
        &format!(
            "a deleted tracked retry must not be mistaken for an untracked output: {rejection}"
        ),
    )?;
    remove_temp_root(fixture.container);
    Ok(())
}

#[cfg(windows)]
#[test]
fn drive_relative_plan_recovery_is_manual_before_cross_cwd_regeneration() -> TestResult {
    use std::path::{Component, Prefix};

    let fixture = RecoveryFixture::new(false, Path::new(SOURCE_PATH))?;
    let drive = fixture
        .caller
        .components()
        .find_map(|component| match component {
            Component::Prefix(prefix) => match prefix.kind() {
                Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => Some(char::from(drive)),
                _ => None,
            },
            _ => None,
        })
        .ok_or("the native Windows fixture needs a drive-letter caller directory")?;
    let drive_relative = PathBuf::from(format!("{drive}:{RECORDED_PLAN}"));
    let refused_receipt = fixture.caller.join("refused must not exist.json");
    let refused = fixture.add(&drive_relative, &refused_receipt)?;
    let rejection = String::from_utf8(refused.stderr)?;
    fixture.require_preserved()?;
    require(
        refused.status.code() == Some(2)
            && rejection.contains("(policy unchanged)")
            && rejection.contains("; regenerate manually: ")
            && rejection.contains("not anchored absolutely")
            && !rejection.contains("; regenerate with ")
            && !refused_receipt.exists()
            && !fixture
                .plan_path
                .with_file_name("original plan.retry-3.json")
                .exists()
            && fs::read(fixture.root.join(SELECTED_POLICY))? == fixture.policy_before,
        &format!("drive-relative recovery must not advertise an unstable path: {rejection}"),
    )?;
    let safe_plan = fixture
        .caller
        .join("saved plans/absolute manual recovery.json");
    fixture.generate_plan_from(&fixture.paste_cwd, 3, &safe_plan)?;
    fixture.apply_and_replay(&safe_plan)?;
    remove_temp_root(fixture.container);
    Ok(())
}

#[test]
fn leading_dash_finding_recovery_runs_verbatim() -> TestResult {
    context_recovery_runs_verbatim(false, Path::new("-finding.rs"))
}

#[cfg(unix)]
#[test]
fn raw_unicode_and_backslash_finding_recovery_runs_verbatim() -> TestResult {
    for source_path in ["e\u{301}.rs", "literal\\file.rs"] {
        context_recovery_runs_verbatim(false, Path::new(source_path))?;
    }
    Ok(())
}

// This actual-file oracle requires a filesystem accepting arbitrary non-NUL
// bytes. The pure non-UTF-8 hint control also runs on the other Unix targets.
#[cfg(target_os = "linux")]
#[test]
fn non_utf8_finding_recovery_requires_manual_guidance_without_mutation() -> TestResult {
    use std::os::unix::ffi::OsStrExt;

    let source_path = Path::new(OsStr::from_bytes(b"non-utf8-\xff.rs"));
    let fixture = RecoveryFixture::new(false, source_path)?;
    let refused_receipt = fixture.caller.join("refused must not exist.json");
    let refused = fixture.add(Path::new(RECORDED_PLAN), &refused_receipt)?;
    let rejection = String::from_utf8(refused.stderr)?;
    // Prove that the raw filesystem target is live and independently plannable;
    // inability to render it through a text shell command is a narrower limit.
    let control_path = fixture.caller.join("saved plans/raw path control.json");
    let control = fixture.generate_plan(3, &control_path)?;
    require(
        control.pointer("/finding/line").and_then(Value::as_u64) == Some(3)
            && control.pointer("/outcome/status").and_then(Value::as_str) == Some("new"),
        "raw non-UTF-8 source must remain an independently plannable New finding",
    )?;
    fixture.require_preserved()?;
    require(
        refused.status.code() == Some(2)
            && rejection.contains("E0001_USAGE")
            && rejection.contains("(policy unchanged)")
            && rejection.contains("; regenerate manually: ")
            && rejection.contains("without data loss")
            && !rejection.contains("regenerate with")
            && !rejection.contains('\u{fffd}')
            && !refused_receipt.exists()
            && !fixture
                .caller
                .join("saved plans/original plan.retry-3.json")
                .exists()
            && fs::read(fixture.root.join(SELECTED_POLICY))? == fixture.policy_before,
        &format!("non-UTF-8 recovery must refuse without a lossy command or mutation: {rejection}"),
    )?;
    remove_temp_root(fixture.container);
    Ok(())
}
