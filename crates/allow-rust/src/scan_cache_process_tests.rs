use super::*;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc;
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
}
impl ChildGuard {
    fn wait(&mut self) -> Result<ExitStatus, String> {
        let deadline = Instant::now() + CHILD_WAIT_TIMEOUT;
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
    fn kill_and_wait(&mut self) -> Result<ExitStatus, String> {
        if self.child.try_wait().map_err(|e| e.to_string())?.is_none() {
            self.child
                .kill()
                .map_err(|error| format!("failed to kill child: {error}"))?;
        }
        self.wait()
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !matches!(self.child.try_wait(), Ok(None)) || self.child.kill().is_err() {
            return;
        }
        let deadline = Instant::now() + CHILD_WAIT_TIMEOUT;
        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(_)) | Err(_) => return,
                Ok(None) => thread::sleep(Duration::from_millis(1)),
            }
        }
    }
}

fn spawn_helper(root: &Path, mode: &str) -> Result<(ChildGuard, BufReader<ChildStdout>), String> {
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
    };
    let stdout = child
        .child
        .stdout
        .take()
        .ok_or_else(|| "child stdout missing".to_string())?;
    Ok((child, BufReader::new(stdout)))
}

fn wait_ready(stdout: BufReader<ChildStdout>) -> Result<(), String> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut stdout = stdout;
        let mut line = String::new();
        let mut ready = false;
        let result = loop {
            line.clear();
            match stdout.read_line(&mut line) {
                Ok(0) if ready => break Ok(()),
                Ok(0) => break Err("child exited before READY".to_string()),
                Ok(_) if line.trim() == "READY" && !ready => {
                    ready = true;
                    let _ = sender.send(Ok(()));
                }
                Ok(_) => {}
                Err(error) => break Err(error.to_string()),
            }
        };
        if !ready {
            let _ = sender.send(result);
        }
    });
    receiver
        .recv_timeout(Duration::from_secs(10))
        .map_err(|e| e.to_string())?
}

#[test]
fn child_process_lock_blocks_then_releases_for_flush() -> Result<(), String> {
    let root = RootGuard::new("lock");
    std::fs::create_dir_all(&root.0).map_err(|e| e.to_string())?;
    let (mut child, stdout) = spawn_helper(&root.0, "hold")?;
    wait_ready(stdout)?;
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
        Vec::new(),
    );
    let (contention_tx, contention_rx) = mpsc::channel();
    let flush_thread = thread::spawn(move || {
        let wait_hook = || {
            let _ = contention_tx.send(());
        };
        store.flush_with_test_hooks(None, Some(&wait_hook), None)
    });
    contention_rx
        .recv_timeout(Duration::from_secs(2))
        .map_err(|e| e.to_string())?;
    child
        .child
        .stdin
        .take()
        .ok_or_else(|| "child stdin missing".to_string())?
        .write_all(&[1])
        .map_err(|e| e.to_string())?;
    let status = child.wait()?;
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
    wait_ready(stdout)?;
    let status = child.kill_and_wait()?;
    if status.success() {
        return Err("killed lock child unexpectedly succeeded".to_string());
    }
    let mut store = ScanCacheStore::open(&root.0, "generation");
    store.put(
        Path::new("src/recovered.rs"),
        "recovered".to_string(),
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
        Vec::new(),
    );
    if !initial.flush() {
        return Err("initial production flush failed".to_string());
    }
    let destination = std::fs::read(root.0.join("scan-cache.v2.bin")).map_err(|e| e.to_string())?;
    let (mut child, stdout) = spawn_helper(&root.0, "flush-temp")?;
    wait_ready(stdout)?;
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
        Vec::new(),
    );
    if !next.flush() {
        return Err("recovery production flush failed".to_string());
    }
    if ScanCacheStore::open(&root.0, "generation")
        .get(Path::new("src/next.rs"), "next")
        .is_none()
    {
        return Err("recovery entry missing".to_string());
    }
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
