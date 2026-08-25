from pathlib import Path

PATH = Path("crates/allow-rust/src/scan_cache_store.rs")
text = PATH.read_text(encoding="utf-8")


def replace_once(old: str, new: str) -> None:
    global text
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected one source shape, found {count}: {old[:80]!r}")
    text = text.replace(old, new, 1)


replace_once(
    "use std::time::{Duration, SystemTime};\n",
    "use std::time::{Duration, SystemTime};\n",
)

replace_once(
    "static TEMP_NONCE: AtomicU64 = AtomicU64::new(0);\n\n/// One persisted scan result",
    r'''static TEMP_NONCE: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "syntax")]
struct PathIdentity(same_file::Handle);

#[cfg(not(feature = "syntax"))]
struct PathIdentity;

impl PathIdentity {
    fn from_path(path: &Path) -> Option<Self> {
        #[cfg(feature = "syntax")]
        {
            same_file::Handle::from_path(path).ok().map(Self)
        }
        #[cfg(not(feature = "syntax"))]
        {
            let _ = path;
            Some(Self)
        }
    }

    fn from_file(file: &File) -> Option<Self> {
        #[cfg(feature = "syntax")]
        {
            let cloned = file.try_clone().ok()?;
            same_file::Handle::from_file(cloned).ok().map(Self)
        }
        #[cfg(not(feature = "syntax"))]
        {
            let _ = file;
            Some(Self)
        }
    }

    fn matches_path(&self, path: &Path) -> bool {
        #[cfg(feature = "syntax")]
        {
            same_file::Handle::from_path(path).is_ok_and(|current| current == self.0)
        }
        #[cfg(not(feature = "syntax"))]
        {
            let _ = (self, path);
            true
        }
    }
}

enum ExistingFileIdentity {
    Absent,
    Present(PathIdentity),
}

impl ExistingFileIdentity {
    fn bind(path: &Path) -> Option<Self> {
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if !path_is_unsafe(path) && metadata.is_file() => {
                PathIdentity::from_path(path).map(Self::Present)
            }
            Ok(_) => None,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(Self::Absent),
            Err(_) => None,
        }
    }

    fn matches_path(&self, path: &Path) -> bool {
        match self {
            Self::Absent => std::fs::symlink_metadata(path)
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
            Self::Present(identity) => regular_file_identity_matches(path, identity),
        }
    }

    #[cfg(windows)]
    const fn was_present(&self) -> bool {
        matches!(self, Self::Present(_))
    }
}

fn bind_directory_identity(path: &Path) -> Option<PathIdentity> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if path_is_unsafe(path) || !metadata.is_dir() {
        return None;
    }
    PathIdentity::from_path(path)
}

fn directory_identity_matches(path: &Path, identity: &PathIdentity) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| {
        !path_is_unsafe(path) && metadata.is_dir() && identity.matches_path(path)
    })
}

fn bind_open_regular_file(file: &File) -> Option<PathIdentity> {
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    PathIdentity::from_file(file)
}

fn regular_file_identity_matches(path: &Path, identity: &PathIdentity) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| {
        !path_is_unsafe(path) && metadata.is_file() && identity.matches_path(path)
    })
}

fn remove_bound_file(path: &Path, identity: &PathIdentity) {
    if regular_file_identity_matches(path, identity) {
        let _ = std::fs::remove_file(path);
    }
}

/// One persisted scan result''',
)

old_flush_start = '''    fn flush_with_test_hooks(
        &mut self,
        injected_temp: Option<&Path>,
        wait_hook: Option<&dyn Fn()>,
        temp_sync_hook: Option<&dyn Fn()>,
    ) -> bool {
'''
start = text.find(old_flush_start)
if start < 0:
    raise SystemExit("flush function start not found")
end_marker = '''    /// Number of persisted entries currently held in memory.
'''
end = text.find(end_marker, start)
if end < 0:
    raise SystemExit("flush function end not found")
new_flush = r'''    fn flush_with_test_hooks(
        &mut self,
        injected_temp: Option<&Path>,
        wait_hook: Option<&dyn Fn()>,
        temp_sync_hook: Option<&dyn Fn()>,
    ) -> bool {
        if !self.writable
            || path_has_symlink_component(&self.dir)
            || path_is_unsafe(&self.store_path())
        {
            return false;
        }
        if !self.dirty {
            return true;
        }
        if std::fs::create_dir_all(&self.dir).is_err() {
            return false;
        }
        if path_has_symlink_component(&self.dir) || path_is_unsafe(&self.store_path()) {
            return false;
        }
        let Some(dir_identity) = bind_directory_identity(&self.dir) else {
            return false;
        };
        let Some(lock) = WriterLock::acquire(&self.dir, wait_hook) else {
            return false;
        };
        if !directory_identity_matches(&self.dir, &dir_identity) || !lock.is_current() {
            return false;
        }
        remove_stale_artifacts(&self.dir);
        if !directory_identity_matches(&self.dir, &dir_identity) || !lock.is_current() {
            return false;
        }
        let bytes = match encode_store(&self.generation, &self.entries) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        let dest = self.store_path();
        let Some(dest_identity) = ExistingFileIdentity::bind(&dest) else {
            return false;
        };
        let tmp = injected_temp
            .map(PathBuf::from)
            .unwrap_or_else(|| next_temp_path(&self.dir));
        if tmp.parent() != Some(self.dir.as_path()) || path_is_unsafe(&tmp) {
            return false;
        }
        if !directory_identity_matches(&self.dir, &dir_identity)
            || !lock.is_current()
            || !dest_identity.matches_path(&dest)
        {
            return false;
        }

        let mut temp_file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
        {
            Ok(file) => file,
            Err(_) => return false,
        };
        let Some(temp_identity) = bind_open_regular_file(&temp_file) else {
            return false;
        };
        if !regular_file_identity_matches(&tmp, &temp_identity) {
            return false;
        }
        if temp_file
            .write_all(&bytes)
            .and_then(|()| temp_file.sync_all())
            .is_err()
        {
            drop(temp_file);
            remove_bound_file(&tmp, &temp_identity);
            return false;
        }
        drop(temp_file);
        if let Some(temp_sync_hook) = temp_sync_hook {
            temp_sync_hook();
        }
        if !replacement_boundary_is_current(
            &self.dir,
            &dir_identity,
            &lock,
            &tmp,
            &temp_identity,
            &dest,
            &dest_identity,
        ) {
            remove_bound_file(&tmp, &temp_identity);
            return false;
        }
        if !move_bound_temp_into_place(
            &self.dir,
            &dir_identity,
            &lock,
            &tmp,
            &temp_identity,
            &dest,
            &dest_identity,
        ) {
            remove_bound_file(&tmp, &temp_identity);
            return false;
        }
        self.dirty = false;
        true
    }

'''
text = text[:start] + new_flush + text[end:]

replace_once(
    '''fn path_is_reparse_point(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::{FileTypeExt, MetadataExt};
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        std::fs::symlink_metadata(path)
            .map(|metadata| {
                metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
                    && (metadata.file_type().is_symlink_dir()
                        || metadata.file_type().is_symlink_file())
            })
            .unwrap_or(false)
    }
''',
    '''fn path_is_reparse_point(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        std::fs::symlink_metadata(path)
            .map(|metadata| metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
            .unwrap_or(false)
    }
''',
)

old_lock_start = '''struct WriterLock {
'''
lock_start = text.find(old_lock_start)
lock_end_marker = '''fn remove_if_stale(path: &Path) -> bool {
'''
lock_end = text.find(lock_end_marker, lock_start)
if lock_start < 0 or lock_end < 0:
    raise SystemExit("writer lock block not found")
new_lock = r'''struct WriterLock {
    file: File,
    path: PathBuf,
    identity: PathIdentity,
}

impl WriterLock {
    fn acquire(dir: &Path, wait_hook: Option<&dyn Fn()>) -> Option<Self> {
        let path = dir.join(LOCK_FILE_NAME);
        if path_has_symlink_component(dir) || path_is_unsafe(&path) {
            return None;
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .ok()?;
        let identity = bind_open_regular_file(&file)?;
        if !regular_file_identity_matches(&path, &identity) {
            return None;
        }
        for _ in 0..100 {
            match file.try_lock() {
                Ok(()) => {
                    let lock = Self {
                        file,
                        path,
                        identity,
                    };
                    if lock.is_current() {
                        return Some(lock);
                    }
                    return None;
                }
                Err(TryLockError::WouldBlock) => {
                    if let Some(wait_hook) = wait_hook {
                        wait_hook();
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => return None,
            }
        }
        None
    }

    fn is_current(&self) -> bool {
        regular_file_identity_matches(&self.path, &self.identity)
    }
}

impl Drop for WriterLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

fn replacement_boundary_is_current(
    dir: &Path,
    dir_identity: &PathIdentity,
    lock: &WriterLock,
    tmp: &Path,
    temp_identity: &PathIdentity,
    dest: &Path,
    dest_identity: &ExistingFileIdentity,
) -> bool {
    directory_identity_matches(dir, dir_identity)
        && lock.is_current()
        && regular_file_identity_matches(tmp, temp_identity)
        && dest_identity.matches_path(dest)
}

fn move_bound_temp_into_place(
    dir: &Path,
    dir_identity: &PathIdentity,
    lock: &WriterLock,
    tmp: &Path,
    temp_identity: &PathIdentity,
    dest: &Path,
    dest_identity: &ExistingFileIdentity,
) -> bool {
    if !replacement_boundary_is_current(
        dir,
        dir_identity,
        lock,
        tmp,
        temp_identity,
        dest,
        dest_identity,
    ) {
        return false;
    }
    if std::fs::rename(tmp, dest).is_ok() {
        return regular_file_identity_matches(dest, temp_identity);
    }

    #[cfg(windows)]
    {
        if !dest_identity.was_present()
            || !replacement_boundary_is_current(
                dir,
                dir_identity,
                lock,
                tmp,
                temp_identity,
                dest,
                dest_identity,
            )
            || std::fs::remove_file(dest).is_err()
            || !std::fs::symlink_metadata(dest)
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
            || !directory_identity_matches(dir, dir_identity)
            || !lock.is_current()
            || !regular_file_identity_matches(tmp, temp_identity)
            || std::fs::rename(tmp, dest).is_err()
        {
            return false;
        }
        regular_file_identity_matches(dest, temp_identity)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

'''
text = text[:lock_start] + new_lock + text[lock_end:]

insert_marker = '''    #[cfg(windows)]
    #[test]
    fn junction_cache_root_is_rejected_without_outside_write() -> Result<(), String> {
'''
if text.count(insert_marker) != 1:
    raise SystemExit("test insertion marker drift")
new_tests = r'''    #[cfg(feature = "syntax")]
    #[test]
    fn temp_regular_file_replacement_after_sync_is_rejected() -> Result<(), String> {
        let root = canonical_temp_dir().join(format!(
            "allow-rust-cache-temp-replacement-{}-{}",
            std::process::id(),
            TEMP_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let mut initial = ScanCacheStore::open(&root, "generation");
        initial.put(
            Path::new("src/original.rs"),
            "original".to_string(),
            false,
            Vec::new(),
        );
        assert!(initial.flush());
        let dest = root.join("scan-cache.v2.bin");
        let original = std::fs::read(&dest).map_err(|error| error.to_string())?;

        let temp = temp_path(&root, TEMP_NONCE.fetch_add(1, Ordering::Relaxed));
        let replacement = || {
            let _ = std::fs::remove_file(&temp);
            let _ = std::fs::write(&temp, b"replacement-temp");
        };
        let mut store = ScanCacheStore::open(&root, "generation");
        store.put(
            Path::new("src/replacement.rs"),
            "replacement".to_string(),
            false,
            Vec::new(),
        );
        assert!(!store.flush_with_test_hooks(
            Some(&temp),
            None,
            Some(&replacement)
        ));
        assert_eq!(
            std::fs::read(&dest).map_err(|error| error.to_string())?,
            original
        );
        assert_eq!(
            std::fs::read(&temp).map_err(|error| error.to_string())?,
            b"replacement-temp"
        );
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[cfg(feature = "syntax")]
    #[test]
    fn destination_regular_file_replacement_after_temp_sync_is_rejected() -> Result<(), String> {
        let root = canonical_temp_dir().join(format!(
            "allow-rust-cache-destination-replacement-{}-{}",
            std::process::id(),
            TEMP_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let mut initial = ScanCacheStore::open(&root, "generation");
        initial.put(
            Path::new("src/original.rs"),
            "original".to_string(),
            false,
            Vec::new(),
        );
        assert!(initial.flush());

        let dest = root.join("scan-cache.v2.bin");
        let temp = temp_path(&root, TEMP_NONCE.fetch_add(1, Ordering::Relaxed));
        let replace_destination = || {
            let _ = std::fs::remove_file(&dest);
            let _ = std::fs::write(&dest, b"replacement-destination");
        };
        let mut store = ScanCacheStore::open(&root, "generation");
        store.put(
            Path::new("src/replacement.rs"),
            "replacement".to_string(),
            false,
            Vec::new(),
        );
        assert!(!store.flush_with_test_hooks(
            Some(&temp),
            None,
            Some(&replace_destination)
        ));
        assert_eq!(
            std::fs::read(&dest).map_err(|error| error.to_string())?,
            b"replacement-destination"
        );
        assert!(!temp.exists());
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[cfg(all(feature = "syntax", unix))]
    #[test]
    fn lock_regular_file_replacement_while_waiting_is_rejected() -> Result<(), String> {
        let root = canonical_temp_dir().join(format!(
            "allow-rust-cache-lock-replacement-{}-{}",
            std::process::id(),
            TEMP_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let lock_path = root.join(LOCK_FILE_NAME);
        let held = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|error| error.to_string())?;
        held.lock().map_err(|error| error.to_string())?;

        let worker_root = root.clone();
        let (waiting_tx, waiting_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut store = ScanCacheStore::open(&worker_root, "generation");
            store.put(
                Path::new("src/replacement.rs"),
                "replacement".to_string(),
                false,
                Vec::new(),
            );
            let wait_hook = || {
                let _ = waiting_tx.send(());
            };
            store.flush_with_temp_path_and_wait_hook(None, Some(&wait_hook))
        });
        waiting_rx.recv().map_err(|error| error.to_string())?;
        std::fs::remove_file(&lock_path).map_err(|error| error.to_string())?;
        std::fs::write(&lock_path, b"replacement-lock").map_err(|error| error.to_string())?;
        held.unlock().map_err(|error| error.to_string())?;
        drop(held);

        assert!(!worker.join().map_err(|_| "worker panicked")?);
        assert!(!root.join("scan-cache.v2.bin").exists());
        assert_eq!(
            std::fs::read(&lock_path).map_err(|error| error.to_string())?,
            b"replacement-lock"
        );
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }

'''
text = text.replace(insert_marker, new_tests + insert_marker, 1)

PATH.write_text(text, encoding="utf-8")
