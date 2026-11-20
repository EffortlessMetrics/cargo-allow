use super::*;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

const LOCK_FILE_NAME: &str = "scan-cache.v2.lock";
const CHILD_WAIT_TIMEOUT: Duration = Duration::from_secs(10);

struct RootGuard(PathBuf);
impl RootGuard {
    fn new(label: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let temp_parent = std::env::temp_dir()
            .canonicalize()
            .unwrap_or_else(|_| std::env::temp_dir());
        Self(temp_parent.join(format!(
            "allow-rust-process-cache-{label}-{}-{stamp}",
            std::process::id()
        )))
    }
}
impl Drop for RootGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct ChildGuard {
    child: Child,
    mode: &'static str,
    reader: Option<thread::JoinHandle<Result<(), String>>>,
}
impl ChildGuard {
    fn wait(&mut self) -> Result<ExitStatus, String> {
        let status = self.wait_for_exit(CHILD_WAIT_TIMEOUT)?;
        self.join_reader()?;
        Ok(status)
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> Result<ExitStatus, String> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.child.try_wait().map_err(|e| e.to_string())? {
                Some(status) => return Ok(status),
                None if Instant::now() >= deadline => {
                    return Err("child did not exit before the deadline".to_string());
                }
                None => thread::sleep(Duration::from_millis(1)),
            }
        }
    }
    fn join_reader(&mut self) -> Result<(), String> {
        // The owned helper spawns no descendants. After reap its pipe has
        // no writer, so the reader can drain to EOF without blocking on it.
        if let Some(reader) = self.reader.take() {
            reader
                .join()
                .map_err(|_| "child stdout reader panicked".to_string())??;
        }
        Ok(())
    }

    fn kill_and_wait(&mut self) -> Result<ExitStatus, String> {
        if self.child.try_wait().map_err(|e| e.to_string())?.is_none()
            && let Err(error) = self.child.kill()
            && self.child.try_wait().map_err(|e| e.to_string())?.is_none()
        {
            return Err(format!("failed to kill child: {error}"));
        }
        self.wait()
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Err(error) = self.kill_and_wait() {
            eprintln!("{} child cleanup failed: {error}", self.mode);
        }
    }
}

fn spawn_helper(
    root: &Path,
    mode: &'static str,
) -> Result<(ChildGuard, BufReader<ChildStdout>), String> {
    let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    command
        .args([
            "--exact",
            "scan_cache_store::process_tests::child_helper",
            "--nocapture",
        ])
        .env("CACHE_PROCESS_ROOT", root)
        .env("CACHE_PROCESS_MODE", mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut child = ChildGuard {
        child: command.spawn().map_err(|e| e.to_string())?,
        mode,
        reader: None,
    };
    let stdout = child
        .child
        .stdout
        .take()
        .ok_or_else(|| "child stdout missing".to_string())?;
    Ok((child, BufReader::new(stdout)))
}

/// READY-handshake ceiling. Deliberately not raised blanket-style (#4215):
/// a timeout must remain a real signal, so the failure instead carries the
/// child state, elapsed time, and a bounded stdout tail.
const READY_TIMEOUT: Duration = Duration::from_secs(10);
const STDOUT_TAIL_LINES: usize = 10;
const STDOUT_LINE_BYTES: u64 = 1024;

/// Shared bounded tail of the child's stdout lines, for timeout diagnostics.
#[derive(Clone)]
struct StdoutTail(Arc<Mutex<Vec<String>>>);

impl StdoutTail {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(Vec::new())))
    }
    fn record(&self, line: &str) {
        // Recover from poison: the buffered lines are valid diagnostics even
        // if another thread panicked while holding the guard (#4215).
        let mut buffer = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        buffer.push(line.to_string());
        let length = buffer.len();
        if length > STDOUT_TAIL_LINES {
            buffer.drain(..length - STDOUT_TAIL_LINES);
        }
    }
    fn snapshot(&self) -> String {
        // Same poison recovery as record: the tail must survive to reach the
        // failure message (#4215).
        let buffer = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if buffer.is_empty() {
            "(no stdout lines)".to_string()
        } else {
            buffer.join("\n")
        }
    }
}

#[test]
fn stdout_tail_snapshot_tolerates_poisoned_mutex() -> Result<(), String> {
    // Poison by panicking while the guard is alive; the snapshot must fall
    // back to its unavailable marker instead of propagating the poison.
    let tail = StdoutTail(Arc::new(Mutex::new(Vec::new())));
    let poisoner = {
        let tail = tail.clone();
        std::thread::spawn(move || {
            let _guard = tail.0.lock().expect("unpoisoned lock");
            panic!("deliberate poison for the tolerance test");
        })
    };
    if poisoner.join().is_ok() {
        return Err("poisoner thread unexpectedly succeeded".to_string());
    }
    tail.record("after poison");
    let snapshot = tail.snapshot();
    assert!(
        snapshot.contains("after poison"),
        "recorded lines must survive poisoning; snapshot: {snapshot}"
    );
    Ok(())
}

fn describe_exit(status: &Option<ExitStatus>) -> String {
    match status {
        Some(status) => {
            if let Some(code) = status.code() {
                format!("exited with code {code}")
            } else {
                format!("exited with {status}")
            }
        }
        None => "still running".to_string(),
    }
}

fn finish_failure(
    child: &mut ChildGuard,
    tail: &StdoutTail,
    started: Instant,
    phase: &str,
) -> String {
    let exit_before = match child.child.try_wait() {
        Ok(status) => describe_exit(&status),
        Err(error) => format!("status unavailable: {error}"),
    };
    let cleanup = match child.kill_and_wait() {
        Ok(status) => format!(
            "final {}; stdout reader joined",
            describe_exit(&Some(status))
        ),
        Err(error) => format!("cleanup failed: {error}"),
    };
    format!(
        "READY handshake failed after {:?} (mode {}; {phase}; child {exit_before} at failure, {cleanup}); stdout tail (last {STDOUT_TAIL_LINES} lines):
{}",
        started.elapsed(),
        child.mode,
        tail.snapshot(),
    )
}

fn wait_ready(child: &mut ChildGuard, stdout: BufReader<ChildStdout>) -> Result<(), String> {
    wait_ready_with_timeout(child, stdout, READY_TIMEOUT)
}

fn wait_ready_with_timeout(
    child: &mut ChildGuard,
    mut stdout: BufReader<ChildStdout>,
    timeout: Duration,
) -> Result<(), String> {
    let started = Instant::now();
    let tail = StdoutTail::new();
    let (signal_tx, signal_rx) = mpsc::channel();
    let reader_tail = tail.clone();
    child.reader = Some(thread::spawn(move || {
        let mut signal_tx = Some(signal_tx);
        let mut line = Vec::new();
        loop {
            line.clear();
            let read = (|| {
                let count = (&mut stdout)
                    .take(STDOUT_LINE_BYTES + 1)
                    .read_until(b'\n', &mut line)?;
                let truncated = count as u64 > STDOUT_LINE_BYTES;
                if truncated && !line.ends_with(b"\n") {
                    stdout.skip_until(b'\n')?;
                }
                Ok::<_, std::io::Error>((count, truncated))
            })();
            match read {
                Ok((0, _)) => {
                    if let Some(sender) = signal_tx {
                        let _ = sender.send(Err("stdout closed before READY".to_string()));
                    }
                    return Ok(());
                }
                Ok((_, truncated)) => {
                    line.truncate(STDOUT_LINE_BYTES as usize);
                    let decoded = String::from_utf8_lossy(&line);
                    let trimmed = decoded.trim_end();
                    if truncated {
                        reader_tail.record(&format!("{trimmed} [truncated]"));
                    } else {
                        reader_tail.record(trimmed);
                        if trimmed == "READY"
                            && let Some(sender) = signal_tx.take()
                        {
                            let _ = sender.send(Ok(()));
                        }
                    }
                }
                Err(error) => {
                    let reason = format!("stdout read failed: {error}");
                    if let Some(sender) = signal_tx {
                        let _ = sender.send(Err(reason.clone()));
                    }
                    return Err(reason);
                }
            }
        }
    }));
    let remaining = timeout.saturating_sub(started.elapsed());
    let phase = if remaining.is_zero() {
        "READY deadline already passed at entry".to_string()
    } else {
        match signal_rx.recv_timeout(remaining) {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(reason)) => {
                // Pipe closure and process exit can be observed in either order.
                let _ = child.wait_for_exit(Duration::from_secs(1));
                reason
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                format!("exceeded the {}s timeout budget", timeout.as_secs())
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                "reader ended without a signal".to_string()
            }
        }
    };
    Err(finish_failure(child, &tail, started, &phase))
}

#[test]
fn child_process_lock_blocks_then_releases_for_flush() -> Result<(), String> {
    let root = RootGuard::new("lock");
    std::fs::create_dir_all(&root.0).map_err(|e| e.to_string())?;
    let (mut child, stdout) = spawn_helper(&root.0, "hold")?;
    wait_ready(&mut child, stdout)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.0.join(LOCK_FILE_NAME))
        .map_err(|e| e.to_string())?;
    match lock.try_lock() {
        Err(std::fs::TryLockError::WouldBlock) => {}
        other => return Err(format!("unexpected child-lock probe result: {other:?}")),
    }
    drop(lock);
    let mut store = ScanCacheStore::open(&root.0, "generation");
    store.put(
        Path::new("src/child.rs"),
        "child".to_string(),
        false,
        false,
        Vec::new(),
    );
    let (contention_tx, contention_rx) = mpsc::channel();
    let flush_thread = thread::spawn(move || {
        let wait_hook = || {
            let _ = contention_tx.send(());
        };
        store.flush_with_test_hooks(None, Some(&wait_hook), None)
    });
    // 5s contention window: under parallel suite load the flush thread's
    // scheduler delay can exceed a tighter bound (#4215 flake family).
    contention_rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    child
        .child
        .stdin
        .take()
        .ok_or_else(|| "child stdin missing".to_string())?
        .write_all(&[1])
        .map_err(|e| e.to_string())?;
    let status = child.wait()?;
    assert!(
        child.reader.is_none(),
        "release must join the stdout reader"
    );
    if !status.success() {
        return Err(format!("lock child failed: {status}"));
    }
    if !flush_thread
        .join()
        .map_err(|_| "flush thread panicked".to_string())?
    {
        return Err("production flush did not recover after child released lock".to_string());
    }
    Ok(())
}

#[test]
fn killed_lock_holder_releases_os_lock() -> Result<(), String> {
    let root = RootGuard::new("kill-lock");
    std::fs::create_dir_all(&root.0).map_err(|e| e.to_string())?;
    let (mut child, stdout) = spawn_helper(&root.0, "hold")?;
    wait_ready(&mut child, stdout)?;
    let status = child.kill_and_wait()?;
    assert!(child.reader.is_none(), "kill must join the stdout reader");
    if status.success() {
        return Err("killed lock child unexpectedly succeeded".to_string());
    }
    let mut store = ScanCacheStore::open(&root.0, "generation");
    store.put(
        Path::new("src/recovered.rs"),
        "recovered".to_string(),
        false,
        false,
        Vec::new(),
    );
    if !store.flush() {
        return Err("production flush failed after child termination".to_string());
    }
    Ok(())
}

#[test]
fn killed_child_after_temp_sync_preserves_destination_and_allows_next_flush() -> Result<(), String>
{
    let root = RootGuard::new("kill-temp");
    std::fs::create_dir_all(&root.0).map_err(|e| e.to_string())?;
    let mut initial = ScanCacheStore::open(&root.0, "generation");
    initial.put(
        Path::new("src/original.rs"),
        "original".to_string(),
        false,
        false,
        Vec::new(),
    );
    if !initial.flush() {
        return Err("initial production flush failed".to_string());
    }
    let destination = std::fs::read(root.0.join("scan-cache.v2.bin")).map_err(|e| e.to_string())?;
    let (mut child, stdout) = spawn_helper(&root.0, "flush-temp")?;
    wait_ready(&mut child, stdout)?;
    let status = child.kill_and_wait()?;
    if status.success() {
        return Err("interrupted flush child unexpectedly succeeded".to_string());
    }
    if std::fs::read(root.0.join("scan-cache.v2.bin")).map_err(|e| e.to_string())? != destination {
        return Err("destination changed during interrupted flush".to_string());
    }
    let orphaned: Vec<PathBuf> = std::fs::read_dir(&root.0)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(TEMP_FILE_PREFIX))
        })
        .collect();
    if orphaned.len() != 1 {
        return Err(format!(
            "expected exactly one orphaned temp, found {}",
            orphaned.len()
        ));
    }
    let orphan = orphaned
        .first()
        .ok_or_else(|| "orphan temp missing".to_string())?;
    let metadata = std::fs::symlink_metadata(orphan).map_err(|e| e.to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() == 0 {
        return Err("orphaned temp is not a nonempty regular file".to_string());
    }
    std::fs::File::open(orphan)
        .and_then(|mut file| {
            let mut bytes = [0_u8; 1];
            file.read_exact(&mut bytes)
        })
        .map_err(|e| format!("orphaned temp is unreadable: {e}"))?;
    let mut next = ScanCacheStore::open(&root.0, "generation");
    next.put(
        Path::new("src/next.rs"),
        "next".to_string(),
        false,
        false,
        Vec::new(),
    );
    if !next.flush() {
        return Err("recovery production flush failed".to_string());
    }
    if ScanCacheStore::open(&root.0, "generation")
        .get(Path::new("src/next.rs"), "next", false)
        .is_none()
    {
        return Err("recovery entry missing".to_string());
    }
    Ok(())
}

fn spawn_silent_child(root: &Path) -> Result<(ChildGuard, BufReader<ChildStdout>), String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    let (mut child, stdout) = spawn_helper(root, "silent-hold")?;
    let flushed = root.join("stdout-flushed");
    let deadline = Instant::now() + READY_TIMEOUT;
    while !flushed.try_exists().map_err(|error| error.to_string())? {
        if let Some(status) = child.child.try_wait().map_err(|error| error.to_string())? {
            return Err(format!(
                "silent child exited before flushing stdout: {status}"
            ));
        }
        if Instant::now() >= deadline {
            return Err(
                "silent child did not flush stdout before the fixture deadline".to_string(),
            );
        }
        thread::sleep(Duration::from_millis(1));
    }
    Ok((child, stdout))
}

fn assert_silent_child_settled(child: &mut ChildGuard, message: &str) -> Result<(), String> {
    assert!(
        child.reader.is_none(),
        "timeout returned with an unjoined stdout reader"
    );
    let status = child.child.try_wait().map_err(|error| error.to_string())?;
    assert!(
        status.is_some_and(|status| !status.success()),
        "child status: {status:?}"
    );
    assert!(message.contains("mode silent-hold"), "{message}");
    assert!(
        message.contains("child still running at failure"),
        "{message}"
    );
    assert!(message.contains("final exited with"), "{message}");
    assert!(message.contains("stdout reader joined"), "{message}");
    assert!(message.contains("silent child final marker"), "{message}");
    assert!(message.contains("READY [truncated]"), "{message}");
    assert!(!message.contains("noise line 0\n"), "{message}");
    Ok(())
}

#[test]
fn expired_ready_deadline_reaps_child_and_joins_reader() -> Result<(), String> {
    let root = RootGuard::new("expired-ready");
    let (mut child, stdout) = spawn_silent_child(&root.0)?;
    let Err(message) = wait_ready_with_timeout(&mut child, stdout, Duration::ZERO) else {
        return Err("already-expired READY deadline succeeded".to_string());
    };
    assert!(
        message.contains("READY deadline already passed at entry"),
        "{message}"
    );
    assert_silent_child_settled(&mut child, &message)
}

#[test]
fn silent_child_timeout_reports_child_state_and_stdout_tail() -> Result<(), String> {
    let root = RootGuard::new("silent-hold");
    let (mut child, stdout) = spawn_silent_child(&root.0)?;
    let silent_deadline = Duration::from_secs(3);
    let started = Instant::now();
    let message = wait_ready_with_timeout(&mut child, stdout, silent_deadline)
        .expect_err("silent child must time out");
    assert!(
        started.elapsed() >= silent_deadline,
        "timeout returned too early"
    );
    assert!(
        message.contains("exceeded the 3s timeout budget"),
        "{message}"
    );
    assert_silent_child_settled(&mut child, &message)
}

#[test]
fn child_exit_before_ready_surfaces_immediately_with_exit_status() -> Result<(), String> {
    let started = std::time::Instant::now();
    let root = RootGuard::new("exit-before-ready");
    std::fs::create_dir_all(&root.0).map_err(|e| e.to_string())?;
    let (mut child, stdout) = spawn_helper(&root.0, "unknown")?;
    let message = wait_ready(&mut child, stdout).expect_err("exiting child must not be silent");
    assert!(child.reader.is_none(), "EOF must join the stdout reader");
    let elapsed = started.elapsed();
    assert!(
        elapsed < READY_TIMEOUT,
        "child exit must surface immediately, not after the deadline: {elapsed:?}"
    );
    assert!(
        message.contains("stdout closed before READY"),
        "message: {message}"
    );
    assert!(
        message.contains("exited with code 101"),
        "child exit status must be attached (the unknown mode exits 101); message: {message}"
    );
    Ok(())
}

#[test]
fn unknown_child_mode_fails_closed() -> Result<(), String> {
    let root = RootGuard::new("unknown-mode");
    std::fs::create_dir_all(&root.0).map_err(|e| e.to_string())?;
    let (mut child, _stdout) = spawn_helper(&root.0, "unknown")?;
    if child.wait()?.success() {
        return Err("unknown CACHE_PROCESS_MODE unexpectedly succeeded".to_string());
    }
    Ok(())
}

#[test]
fn child_helper() -> Result<(), String> {
    let Ok(root) = std::env::var("CACHE_PROCESS_ROOT") else {
        return Ok(());
    };
    let root = PathBuf::from(root);
    let mode = std::env::var("CACHE_PROCESS_MODE").map_err(|e| e.to_string())?;
    if mode == "flush-temp" {
        let mut store = ScanCacheStore::open(&root, "generation");
        store.put(
            Path::new("src/child.rs"),
            "child".to_string(),
            false,
            false,
            Vec::new(),
        );
        let stop = || {
            println!("READY");
            let _ = std::io::stdout().flush();
            let mut byte = [0_u8; 1];
            let _ = std::io::stdin().read_exact(&mut byte);
        };
        if !store.flush_with_test_hooks(None, None, Some(&stop)) {
            return Err("production flush failed before stop point".to_string());
        }
        return Ok(());
    }
    if mode == "silent-hold" {
        for index in 0..12 {
            println!("noise line {index}");
        }
        // The fixture signals only after this marker is in the pipe. The
        // parent can exercise either deadline immediately after observing it.
        println!("READY{}", " ".repeat(STDOUT_LINE_BYTES as usize));
        println!("silent child final marker");
        std::io::stdout()
            .flush()
            .map_err(|error| error.to_string())?;
        std::fs::write(root.join("stdout-flushed"), []).map_err(|error| error.to_string())?;
        let mut input = [0_u8; 1];
        std::io::stdin()
            .read_exact(&mut input)
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    if mode == "hold" {
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join(LOCK_FILE_NAME))
            .map_err(|e| e.to_string())?;
        lock.lock().map_err(|e| e.to_string())?;
        println!("READY");
        std::io::stdout().flush().map_err(|e| e.to_string())?;
        let mut input = [0_u8; 1];
        std::io::stdin()
            .read_exact(&mut input)
            .map_err(|e| e.to_string())?;
        lock.unlock().map_err(|e| e.to_string())?;
        return Ok(());
    }
    Err(format!("unknown CACHE_PROCESS_MODE: {mode}"))
}
