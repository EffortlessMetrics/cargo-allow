use allow_rust::ScanCacheStore;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, SystemTime};

const LOCK_FILE_NAME: &str = "scan-cache.v2.lock";

struct RootGuard(PathBuf);
impl RootGuard {
    fn new(label: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        Self(std::env::temp_dir().join(format!(
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
        self.child.wait().map_err(|e| e.to_string())
    }
    fn kill_and_wait(&mut self) -> Result<ExitStatus, String> {
        let _ = self.child.kill();
        self.wait()
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

fn spawn_helper(
    root: &Path,
    mode: &str,
    temp: Option<&Path>,
) -> Result<(ChildGuard, BufReader<ChildStdout>), String> {
    let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    command
        .args(["--exact", "child_helper", "--nocapture"])
        .env("CACHE_PROCESS_ROOT", root)
        .env("CACHE_PROCESS_MODE", mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if let Some(temp) = temp {
        command.env("CACHE_PROCESS_TEMP", temp);
    }
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
    std::thread::spawn(move || {
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
    let (mut child, stdout) = spawn_helper(&root.0, "hold", None)?;
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
    lock.try_lock().map_err(|e| e.to_string())?;
    lock.unlock().map_err(|e| e.to_string())?;
    let mut store = ScanCacheStore::open(&root.0, "generation");
    store.put(
        Path::new("src/child.rs"),
        "child".to_string(),
        false,
        Vec::new(),
    );
    assert!(store.flush());
    Ok(())
}

#[test]
fn killed_lock_holder_releases_os_lock() -> Result<(), String> {
    let root = RootGuard::new("kill-lock");
    std::fs::create_dir_all(&root.0).map_err(|e| e.to_string())?;
    let (mut child, stdout) = spawn_helper(&root.0, "hold", None)?;
    wait_ready(stdout)?;
    let status = child.kill_and_wait()?;
    assert!(!status.success());
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.0.join(LOCK_FILE_NAME))
        .map_err(|e| e.to_string())?;
    lock.try_lock().map_err(|e| e.to_string())?;
    lock.unlock().map_err(|e| e.to_string())?;
    let mut store = ScanCacheStore::open(&root.0, "generation");
    store.put(
        Path::new("src/recovered.rs"),
        "recovered".to_string(),
        false,
        Vec::new(),
    );
    assert!(store.flush());
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
    assert!(initial.flush());
    let destination = std::fs::read(root.0.join("scan-cache.v2.bin")).map_err(|e| e.to_string())?;
    let temp = root.0.join("scan-cache.v2.bin.tmp-child");
    let (mut child, stdout) = spawn_helper(&root.0, "temp", Some(&temp))?;
    wait_ready(stdout)?;
    let status = child.kill_and_wait()?;
    assert!(!status.success());
    assert_eq!(
        std::fs::read(root.0.join("scan-cache.v2.bin")).map_err(|e| e.to_string())?,
        destination
    );
    assert!(temp.exists());
    let mut next = ScanCacheStore::open(&root.0, "generation");
    next.put(
        Path::new("src/next.rs"),
        "next".to_string(),
        false,
        Vec::new(),
    );
    assert!(next.flush());
    assert!(
        ScanCacheStore::open(&root.0, "generation")
            .get(Path::new("src/next.rs"), "next")
            .is_some()
    );
    Ok(())
}

#[test]
fn child_helper() -> Result<(), String> {
    let Ok(root) = std::env::var("CACHE_PROCESS_ROOT") else {
        return Ok(());
    };
    let root = PathBuf::from(root);
    let mode = std::env::var("CACHE_PROCESS_MODE").map_err(|e| e.to_string())?;
    if mode == "temp" {
        let temp = PathBuf::from(std::env::var("CACHE_PROCESS_TEMP").map_err(|e| e.to_string())?);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temp)
            .map_err(|e| e.to_string())?;
        file.write_all(b"partially-written")
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        println!("READY");
        std::io::stdout().flush().map_err(|e| e.to_string())?;
        let mut input = [0_u8; 1];
        std::io::stdin()
            .read_exact(&mut input)
            .map_err(|e| e.to_string())?;
        Ok(())
    } else {
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
        Ok(())
    }
}
