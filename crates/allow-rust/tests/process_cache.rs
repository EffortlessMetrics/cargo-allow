use allow_rust::ScanCacheStore;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};

const LOCK_FILE_NAME: &str = "scan-cache.v2.lock";

fn unique_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "allow-rust-process-cache-{label}-{}",
        std::process::id()
    ))
}

fn spawn_helper(
    root: &Path,
    mode: &str,
    temp: Option<&Path>,
) -> Result<(Child, BufReader<ChildStdout>), String> {
    let mut command = Command::new(std::env::current_exe().map_err(|error| error.to_string())?);
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
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "child stdout missing".to_string())?;
    Ok((child, BufReader::new(stdout)))
}

fn wait_ready(stdout: &mut BufReader<ChildStdout>) -> Result<(), String> {
    let mut line = String::new();
    loop {
        line.clear();
        if stdout
            .read_line(&mut line)
            .map_err(|error| error.to_string())?
            == 0
        {
            return Err("child exited before READY".to_string());
        }
        if line.trim() == "READY" {
            return Ok(());
        }
    }
}

fn remove_root(root: &Path) -> Result<(), String> {
    std::fs::remove_dir_all(root).map_err(|error| error.to_string())
}

#[test]
fn child_process_lock_blocks_then_releases_for_flush() -> Result<(), String> {
    let root = unique_root("lock");
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let (mut child, mut stdout) = spawn_helper(&root, "hold", None)?;
    wait_ready(&mut stdout)?;
    let lock_path = root.join(LOCK_FILE_NAME);
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| error.to_string())?;
    assert!(matches!(
        lock.try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));
    child
        .stdin
        .take()
        .ok_or_else(|| "child stdin missing".to_string())?
        .write_all(&[1])
        .map_err(|error| error.to_string())?;
    let status = child.wait().map_err(|error| error.to_string())?;
    if !status.success() {
        return Err(format!("lock child failed: {status}"));
    }
    lock.try_lock().map_err(|error| error.to_string())?;
    lock.unlock().map_err(|error| error.to_string())?;
    let mut store = ScanCacheStore::open(&root, "generation");
    store.put(
        Path::new("src/child.rs"),
        "child".to_string(),
        false,
        Vec::new(),
    );
    assert!(store.flush());
    remove_root(&root)
}

#[test]
fn crashed_lock_holder_releases_os_lock() -> Result<(), String> {
    let root = unique_root("crash-lock");
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let (mut child, mut stdout) = spawn_helper(&root, "crash-lock", None)?;
    wait_ready(&mut stdout)?;
    let status = child.wait().map_err(|error| error.to_string())?;
    assert!(!status.success());
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.join(LOCK_FILE_NAME))
        .map_err(|error| error.to_string())?;
    lock.try_lock().map_err(|error| error.to_string())?;
    lock.unlock().map_err(|error| error.to_string())?;
    let mut store = ScanCacheStore::open(&root, "generation");
    store.put(
        Path::new("src/recovered.rs"),
        "recovered".to_string(),
        false,
        Vec::new(),
    );
    assert!(store.flush());
    remove_root(&root)
}

#[test]
fn crashed_synced_temp_preserves_destination_and_allows_next_flush() -> Result<(), String> {
    let root = unique_root("crash-temp");
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let mut initial = ScanCacheStore::open(&root, "generation");
    initial.put(
        Path::new("src/original.rs"),
        "original".to_string(),
        false,
        Vec::new(),
    );
    assert!(initial.flush());
    let destination =
        std::fs::read(root.join("scan-cache.v2.bin")).map_err(|error| error.to_string())?;
    let temp = root.join("scan-cache.v2.bin.tmp-child");
    let (mut child, mut stdout) = spawn_helper(&root, "crash-temp", Some(&temp))?;
    wait_ready(&mut stdout)?;
    let status = child.wait().map_err(|error| error.to_string())?;
    assert!(!status.success());
    assert_eq!(
        std::fs::read(root.join("scan-cache.v2.bin")).map_err(|error| error.to_string())?,
        destination
    );
    assert!(temp.exists());
    let mut next = ScanCacheStore::open(&root, "generation");
    next.put(
        Path::new("src/next.rs"),
        "next".to_string(),
        false,
        Vec::new(),
    );
    assert!(next.flush());
    assert!(
        ScanCacheStore::open(&root, "generation")
            .get(Path::new("src/next.rs"), "next")
            .is_some()
    );
    std::fs::remove_file(&temp).map_err(|error| error.to_string())?;
    remove_root(&root)
}

#[test]
fn child_helper() -> Result<(), String> {
    let Ok(root) = std::env::var("CACHE_PROCESS_ROOT") else {
        return Ok(());
    };
    let root = PathBuf::from(root);
    let mode = std::env::var("CACHE_PROCESS_MODE").map_err(|error| error.to_string())?;
    if mode == "crash-temp" {
        let temp =
            PathBuf::from(std::env::var("CACHE_PROCESS_TEMP").map_err(|error| error.to_string())?);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temp)
            .map_err(|error| error.to_string())?;
        file.write_all(b"partially-written")
            .map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        println!("READY");
        std::io::stdout()
            .flush()
            .map_err(|error| error.to_string())?;
        return Err("simulated child interruption after temp sync".to_string());
    }
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(LOCK_FILE_NAME))
        .map_err(|error| error.to_string())?;
    lock.lock().map_err(|error| error.to_string())?;
    println!("READY");
    std::io::stdout()
        .flush()
        .map_err(|error| error.to_string())?;
    if mode == "hold" {
        let mut input = [0_u8; 1];
        std::io::stdin()
            .read_exact(&mut input)
            .map_err(|error| error.to_string())?;
        return Ok(());
    }
    Err("simulated child interruption while holding lock".to_string())
}
