//! Durable, content-addressed persistence for the file-level scan cache.
//!
//! Extends the in-process [`ScanCache`](crate::ScanCache) across CLI
//! invocations (#2571): parsed findings are stored under
//! `<root>/target/cargo-allow/cache/` keyed by the SHA-256 of the exact text
//! the scanner evaluated. A warm hit requires a digest match, so invalidation
//! is content-exact — mtime churn never produces stale facts and preserved
//! mtimes can never mask changed content.
//!
//! Trust rules:
//!
//! - The store is trusted-local **performance state**, not authority: every
//!   entry is validated against the current file digest before use, so an
//!   accidentally corrupted or truncated store only causes a cold re-scan.
//! - Any read/decode failure discards the store (fail-open cold start).
//! - A generation string binds entries to one scanner build; scanner changes
//!   invalidate the whole store rather than silently mixing semantics.
//! - Skipped files (oversized, non-UTF-8, unreadable) are never persisted.
//!
//! The store lives under `target/`, which cargo gitignores, so persisted
//! facts never enter source-tree scans or receipts.

use allow_core::{Finding, read_file_capped_with_limit};
use std::collections::HashMap;
use std::fs::{File, TryLockError};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

/// Store schema identifier; also the on-disk magic header line.
const STORE_SCHEMA: &[u8] = b"cargo-allow.scan-cache.v2";
const CHECKSUM_LEN: usize = 74;
const MAX_ENTRY_COUNT: usize = 100_000;
const MAX_FINDINGS_PER_ENTRY: usize = 100_000;
const MAX_FIELD_BYTES: usize = 16 * 1024 * 1024;
const MAX_STORE_BYTES: usize = 128 * 1024 * 1024;
const MAX_RELATIVE_PATH_BYTES: usize = 4 * 1024;
const TEMP_FILE_PREFIX: &str = "scan-cache.v2.bin.tmp-";
const LOCK_FILE_NAME: &str = "scan-cache.v2.lock";
const STALE_ARTIFACT_AGE: Duration = Duration::from_secs(60 * 60);
static TEMP_NONCE: AtomicU64 = AtomicU64::new(0);

struct PathIdentity(same_file::Handle);

impl PathIdentity {
    fn from_path(path: &Path) -> Option<Self> {
        same_file::Handle::from_path(path).ok().map(Self)
    }

    fn from_file(file: &File) -> Option<Self> {
        let cloned = file.try_clone().ok()?;
        same_file::Handle::from_file(cloned).ok().map(Self)
    }

    fn matches_path(&self, path: &Path) -> bool {
        same_file::Handle::from_path(path).is_ok_and(|current| current == self.0)
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

/// One persisted scan result: the digest it was produced from plus the
/// scanner output for that exact input.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StoredEntry {
    content_digest: String,
    has_parse_error: bool,
    findings: Vec<Finding>,
}

/// Durable scan-facts store. Fail-open by construction: every fallible
/// operation either succeeds, degrades to a miss, or degrades to an empty
/// store — never fails a scan.
pub struct ScanCacheStore {
    dir: PathBuf,
    generation: String,
    entries: HashMap<PathBuf, StoredEntry>,
    dirty: bool,
    writable: bool,
}

impl ScanCacheStore {
    /// The default store directory for a scanned source-tree root.
    pub fn default_dir(root: &Path) -> PathBuf {
        root.join("target").join("cargo-allow").join("cache")
    }

    /// Open (or lazily create) the store under `dir`. Entries from a
    /// different `generation` are discarded rather than reused.
    pub fn open(dir: &Path, generation: impl Into<String>) -> Self {
        let generation = generation.into();
        let writable =
            !path_has_symlink_component(dir) && !path_is_unsafe(&dir.join("scan-cache.v2.bin"));
        let mut store = Self {
            dir: dir.to_path_buf(),
            generation,
            entries: HashMap::new(),
            dirty: false,
            writable,
        };
        if !store.writable {
            return store;
        }
        let path = store.store_path();
        if let Ok(bytes) = read_file_capped_with_limit(&path, MAX_STORE_BYTES as u64)
            && let Ok(decoded) = decode_store(&bytes, &store.generation)
        {
            store.entries = decoded;
        }
        store
    }

    fn store_path(&self) -> PathBuf {
        self.dir.join("scan-cache.v2.bin")
    }

    /// Look up cached findings whose recorded digest matches `content_digest`.
    pub fn get(&self, rel: &Path, content_digest: &str) -> Option<(Vec<Finding>, bool)> {
        let entry = self.entries.get(rel)?;
        if entry.content_digest != content_digest {
            return None;
        }
        Some((entry.findings.clone(), entry.has_parse_error))
    }

    /// Record scan output for a file. Skipped files must not be passed here.
    pub fn put(
        &mut self,
        rel: &Path,
        content_digest: String,
        has_parse_error: bool,
        findings: Vec<Finding>,
    ) {
        if !valid_relative_path(rel)
            || rel
                .to_str()
                .is_none_or(|text| text.len() > MAX_RELATIVE_PATH_BYTES)
            || findings.len() > MAX_FINDINGS_PER_ENTRY
        {
            return;
        }
        if !self.entries.contains_key(rel) && self.entries.len() >= MAX_ENTRY_COUNT {
            return;
        }
        let stored = StoredEntry {
            content_digest,
            has_parse_error,
            findings,
        };
        if self
            .entries
            .get(rel)
            .is_some_and(|existing| *existing == stored)
        {
            return;
        }
        self.entries.insert(rel.to_path_buf(), stored);
        self.dirty = true;
    }

    /// Persist pending changes as a best-effort cache replacement:
    /// write a sibling temp file, then replace the store. Failure returns
    /// `false`; callers treat flush as advisory.
    pub fn flush(&mut self) -> bool {
        self.flush_with_temp_path(None)
    }

    fn flush_with_temp_path(&mut self, injected_temp: Option<&Path>) -> bool {
        self.flush_with_temp_path_and_wait_hook(injected_temp, None)
    }

    fn flush_with_temp_path_and_wait_hook(
        &mut self,
        injected_temp: Option<&Path>,
        wait_hook: Option<&dyn Fn()>,
    ) -> bool {
        self.flush_with_test_hooks(injected_temp, wait_hook, None)
    }

    fn flush_with_test_hooks(
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

        let mut temp_options = std::fs::OpenOptions::new();
        temp_options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            temp_options.custom_flags(no_follow_flag() | close_on_exec_flag());
        }
        let mut temp_file = match temp_options.open(&tmp)
        {
            Ok(file) => file,
            Err(_) => return false,
        };
        let Some(temp_identity) = bind_open_regular_file(&temp_file) else {
            return false;
        };
        if !regular_file_identity_matches(&tmp, &temp_identity) {
            remove_bound_file(&tmp, &temp_identity);
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

    /// Number of persisted entries currently held in memory.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no entries are held.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Drop entries which are not part of the current deterministic scan set.
    pub fn retain_paths(&mut self, paths: &[PathBuf]) {
        let retained: std::collections::HashSet<&Path> = paths
            .iter()
            .filter(|path| valid_relative_path(path))
            .map(PathBuf::as_path)
            .collect();
        let before = self.entries.len();
        self.entries
            .retain(|path, _| retained.contains(path.as_path()));
        self.dirty |= before != self.entries.len();
    }

    /// Remove facts for a path that could not be evaluated in this scan.
    pub fn remove(&mut self, rel: &Path) {
        if self.entries.remove(rel).is_some() {
            self.dirty = true;
        }
    }
}

fn path_has_symlink_component(path: &Path) -> bool {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if path_is_unsafe(&current) {
            return true;
        }
    }
    false
}

fn path_is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

#[cfg(unix)]
const fn no_follow_flag() -> i32 {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    { 0o400000 }
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
    { 0x100 }
}

#[cfg(unix)]
const fn close_on_exec_flag() -> i32 {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    { 0o2000000 }
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
    { 0x1000000 }
}

fn path_is_reparse_point(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        std::fs::symlink_metadata(path)
            .map(|metadata| metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}

fn path_is_unsafe(path: &Path) -> bool {
    path_is_symlink(path) || path_is_reparse_point(path)
}

fn temp_path(dir: &Path, nonce: u64) -> PathBuf {
    dir.join(format!("{TEMP_FILE_PREFIX}{}-{nonce}", std::process::id()))
}

fn next_temp_path(dir: &Path) -> PathBuf {
    temp_path(dir, TEMP_NONCE.fetch_add(1, Ordering::Relaxed))
}

struct WriterLock {
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
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(no_follow_flag() | close_on_exec_flag());
        }
        let file = options
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

fn remove_if_stale(path: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if metadata.file_type().is_symlink() || path_is_reparse_point(path) {
        return false;
    }
    let stale = metadata
        .modified()
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age >= STALE_ARTIFACT_AGE);
    stale && std::fs::remove_file(path).is_ok()
}

fn remove_stale_artifacts(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_temp = entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(TEMP_FILE_PREFIX));
        if is_temp {
            let _ = remove_if_stale(&path);
        }
    }
}

fn valid_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path.components().all(|component| {
            !matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
}

fn reserve(out: &[u8], additional: usize) -> Result<(), ()> {
    let limit = MAX_STORE_BYTES.checked_sub(CHECKSUM_LEN).ok_or(())?;
    let next = out.len().checked_add(additional).ok_or(())?;
    if next > limit {
        return Err(());
    }
    Ok(())
}

fn write_str(out: &mut Vec<u8>, value: &str) -> Result<(), ()> {
    reserve(out, 4usize.checked_add(value.len()).ok_or(())?)?;
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn write_opt_u32(out: &mut Vec<u8>, value: Option<u32>) -> Result<(), ()> {
    match value {
        Some(value) => {
            reserve(out, 5)?;
            out.push(1);
            out.extend_from_slice(&value.to_le_bytes());
        }
        None => {
            reserve(out, 1)?;
            out.push(0);
        }
    }
    Ok(())
}

fn encode_store(generation: &str, entries: &HashMap<PathBuf, StoredEntry>) -> Result<Vec<u8>, ()> {
    // Deterministic order so byte-identical states produce identical files.
    let mut ordered: Vec<(&PathBuf, &StoredEntry)> = entries.iter().collect();
    ordered.sort_by(|left, right| left.0.cmp(right.0));

    let mut out = Vec::new();
    reserve(&out, STORE_SCHEMA.len())?;
    out.extend_from_slice(STORE_SCHEMA);
    reserve(&out, 1)?;
    out.push(b'\n');
    write_str(&mut out, generation)?;
    reserve(&out, 4)?;
    out.extend_from_slice(&(ordered.len() as u32).to_le_bytes());
    for (rel, entry) in ordered {
        let rel_text = rel.to_str().ok_or(())?;
        write_str(&mut out, rel_text)?;
        write_str(&mut out, &entry.content_digest)?;
        reserve(&out, 1 + 4)?;
        out.push(u8::from(entry.has_parse_error));
        out.extend_from_slice(&(entry.findings.len() as u32).to_le_bytes());
        for finding in &entry.findings {
            encode_finding(&mut out, finding)?;
        }
    }
    let checksum = allow_core::sha256_v1_bytes(&out);
    out.extend_from_slice(checksum.as_bytes());
    Ok(out)
}

fn encode_finding(out: &mut Vec<u8>, finding: &Finding) -> Result<(), ()> {
    if finding.ledger.is_some() {
        // Scanner-output findings never carry ledger provenance; provenance is
        // attached later during policy matching and must not be persisted.
        return Err(());
    }
    out.push(finding_kind_tag(finding.kind));
    match &finding.family {
        Some(family) => {
            out.push(1);
            write_str(out, family)?;
        }
        None => out.push(0),
    }
    let path_text = finding.path.to_str().ok_or(())?;
    write_str(out, path_text)?;
    match &finding.span {
        Some(span) => {
            reserve(out, 1 + 8)?;
            out.push(1);
            out.extend_from_slice(&span.line.to_le_bytes());
            out.extend_from_slice(&span.column.to_le_bytes());
        }
        None => {
            reserve(out, 1)?;
            out.push(0)
        }
    }
    let identity = &finding.identity;
    write_str(out, &identity.language)?;
    write_str(out, &identity.ast_kind)?;
    for value in [
        &identity.crate_name,
        &identity.module,
        &identity.container,
        &identity.symbol,
        &identity.callee,
        &identity.macro_name,
        &identity.lint,
        &identity.receiver_fingerprint,
        &identity.target_fingerprint,
        &identity.normalized_snippet_hash,
    ] {
        match value {
            Some(text) => {
                reserve(out, 1)?;
                out.push(1);
                write_str(out, text)?;
            }
            None => {
                reserve(out, 1)?;
                out.push(0)
            }
        }
    }
    write_opt_u32(out, identity.line_hint)?;
    write_opt_u32(out, identity.column_hint)?;
    write_str(out, &finding.message)?;
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], ()> {
        let end = self.cursor.checked_add(count).ok_or(())?;
        let slice = self.bytes.get(self.cursor..end).ok_or(())?;
        self.cursor = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, ()> {
        self.take(1)?.first().copied().ok_or(())
    }

    fn u32(&mut self) -> Result<u32, ()> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().map_err(|_| ())?))
    }

    fn str(&mut self) -> Result<String, ()> {
        self.str_with_limit(MAX_FIELD_BYTES)
    }

    fn str_with_limit(&mut self, max_bytes: usize) -> Result<String, ()> {
        let len = self.u32()? as usize;
        if len > max_bytes {
            return Err(());
        }
        let bytes = self.take(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| ())
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.cursor)
    }

    fn opt_u32(&mut self) -> Result<Option<u32>, ()> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(self.u32()?)),
            _ => Err(()),
        }
    }

    fn opt_str(&mut self) -> Result<Option<String>, ()> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(self.str()?)),
            _ => Err(()),
        }
    }
}

fn decode_store(
    bytes: &[u8],
    expected_generation: &str,
) -> Result<HashMap<PathBuf, StoredEntry>, ()> {
    if bytes.len() > MAX_STORE_BYTES {
        return Err(());
    }
    if bytes.len() < CHECKSUM_LEN {
        return Err(());
    }
    let (payload, checksum) = bytes.split_at(bytes.len() - CHECKSUM_LEN);
    if checksum != allow_core::sha256_v1_bytes(payload).as_bytes() {
        return Err(());
    }
    if !payload.starts_with(STORE_SCHEMA) {
        return Err(());
    }
    let mut reader = Reader::new(payload);
    reader.take(STORE_SCHEMA.len())?;
    if reader.u8()? != b'\n' {
        return Err(());
    }
    let generation = reader.str()?;
    if generation != expected_generation {
        return Err(());
    }
    let entry_count = reader.u32()? as usize;
    if entry_count > MAX_ENTRY_COUNT {
        return Err(());
    }
    let mut entries = HashMap::with_capacity(entry_count.min(65_536));
    for _ in 0..entry_count {
        let rel = reader.str_with_limit(MAX_RELATIVE_PATH_BYTES)?;
        let rel_path = PathBuf::from(&rel);
        let invalid_path = rel.trim().is_empty()
            || rel_path.is_absolute()
            || rel_path.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            });
        let content_digest = reader.str()?;
        let has_parse_error = match reader.u8()? {
            0 => false,
            1 => true,
            _ => return Err(()),
        };
        let finding_count = reader.u32()? as usize;
        if finding_count > MAX_FINDINGS_PER_ENTRY {
            return Err(());
        }
        let mut findings = Vec::with_capacity(finding_count.min(4096));
        for _ in 0..finding_count {
            findings.push(decode_finding(&mut reader)?);
        }
        if !invalid_path {
            entries.insert(
                rel_path,
                StoredEntry {
                    content_digest,
                    has_parse_error,
                    findings,
                },
            );
        }
    }
    if reader.remaining() != 0 {
        return Err(());
    }
    Ok(entries)
}

fn decode_finding(reader: &mut Reader<'_>) -> Result<Finding, ()> {
    let kind_tag = reader.u8()?;
    let kind = finding_kind_from_tag(kind_tag).ok_or(())?;
    let family = reader.opt_str()?;
    let path = PathBuf::from(reader.str()?);
    let span = match reader.u8()? {
        0 => None,
        1 => {
            let line = reader.u32()?;
            let column = reader.u32()?;
            Some(allow_core::Span { line, column })
        }
        _ => return Err(()),
    };
    let language = reader.str()?;
    let ast_kind = reader.str()?;
    let crate_name = reader.opt_str()?;
    let module = reader.opt_str()?;
    let container = reader.opt_str()?;
    let symbol = reader.opt_str()?;
    let callee = reader.opt_str()?;
    let macro_name = reader.opt_str()?;
    let lint = reader.opt_str()?;
    let receiver_fingerprint = reader.opt_str()?;
    let target_fingerprint = reader.opt_str()?;
    let normalized_snippet_hash = reader.opt_str()?;
    let line_hint = reader.opt_u32()?;
    let column_hint = reader.opt_u32()?;
    let message = reader.str()?;
    let mut identity = allow_core::StructuralIdentity::new(language, ast_kind);
    identity.crate_name = crate_name;
    identity.module = module;
    identity.container = container;
    identity.symbol = symbol;
    identity.callee = callee;
    identity.macro_name = macro_name;
    identity.lint = lint;
    identity.receiver_fingerprint = receiver_fingerprint;
    identity.target_fingerprint = target_fingerprint;
    identity.normalized_snippet_hash = normalized_snippet_hash;
    identity.line_hint = line_hint;
    identity.column_hint = column_hint;
    Ok(Finding {
        kind,
        family,
        path,
        span,
        identity,
        message,
        ledger: None,
    })
}

fn finding_kind_tag(kind: allow_core::FindingKind) -> u8 {
    match kind {
        allow_core::FindingKind::Panic => 1,
        allow_core::FindingKind::Unsafe => 2,
        allow_core::FindingKind::LintException => 3,
        allow_core::FindingKind::NonRustFile => 4,
        allow_core::FindingKind::GeneratedCode => 5,
        allow_core::FindingKind::PolicyException => 6,
        // Keep unknown future finding categories out of the known cache-tag
        // space. They will miss the cache rather than aliasing another kind.
        _ => 0,
    }
}

fn finding_kind_from_tag(tag: u8) -> Option<allow_core::FindingKind> {
    Some(match tag {
        1 => allow_core::FindingKind::Panic,
        2 => allow_core::FindingKind::Unsafe,
        3 => allow_core::FindingKind::LintException,
        4 => allow_core::FindingKind::NonRustFile,
        5 => allow_core::FindingKind::GeneratedCode,
        6 => allow_core::FindingKind::PolicyException,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn canonical_temp_dir() -> PathBuf {
        std::env::temp_dir()
            .canonicalize()
            .unwrap_or_else(|_| std::env::temp_dir())
    }

    #[test]
    fn checksum_valid_but_trailing_payload_is_rejected() {
        let encoded = encode_store("generation", &HashMap::new()).unwrap_or_default();
        let payload_len = encoded.len() - CHECKSUM_LEN;
        let mut payload = encoded.get(..payload_len).unwrap_or_default().to_vec();
        payload.push(0);
        let checksum = allow_core::sha256_v1_bytes(&payload);
        payload.extend_from_slice(checksum.as_bytes());
        assert!(decode_store(&payload, "generation").is_err());
    }

    #[test]
    fn checksum_valid_oversized_relative_path_is_rejected() {
        let mut entries = HashMap::new();
        entries.insert(
            PathBuf::from(format!("src/{}", "a".repeat(MAX_RELATIVE_PATH_BYTES))),
            StoredEntry {
                content_digest: "digest".to_string(),
                has_parse_error: false,
                findings: Vec::new(),
            },
        );
        let encoded = encode_store("generation", &entries).unwrap_or_default();
        assert!(decode_store(&encoded, "generation").is_err());
    }

    #[test]
    fn put_rejects_invalid_paths_and_entry_or_finding_overflow() {
        let mut store = ScanCacheStore::open(Path::new("target/cache-test"), "generation");
        store.put(
            Path::new("../escape.rs"),
            "digest".to_string(),
            false,
            Vec::new(),
        );
        assert!(store.is_empty());
        store.put(
            Path::new("src/lib.rs"),
            "digest".to_string(),
            false,
            (0..=MAX_FINDINGS_PER_ENTRY)
                .map(|_| Finding {
                    kind: allow_core::FindingKind::Panic,
                    family: None,
                    path: PathBuf::from("src/lib.rs"),
                    span: None,
                    identity: allow_core::StructuralIdentity::new("rust", "function"),
                    message: String::new(),
                    ledger: None,
                })
                .collect(),
        );
        assert!(store.is_empty());
    }

    #[test]
    fn retain_paths_prunes_stale_entries_deterministically() {
        let mut store = ScanCacheStore::open(Path::new("target/cache-test"), "generation");
        store.put(Path::new("src/a.rs"), "a".to_string(), false, Vec::new());
        store.put(Path::new("src/b.rs"), "b".to_string(), false, Vec::new());
        store.retain_paths(&[PathBuf::from("src/a.rs")]);
        assert!(store.get(Path::new("src/a.rs"), "a").is_some());
        assert!(store.get(Path::new("src/b.rs"), "b").is_none());
    }

    #[test]
    fn oversized_store_fails_open_before_decode_allocation() -> Result<(), String> {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let root = canonical_temp_dir().join(format!(
            "allow-rust-cache-oversized-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let path = root.join("scan-cache.v2.bin");
        let file = std::fs::File::create(&path).map_err(|error| error.to_string())?;
        file.set_len((MAX_STORE_BYTES as u64) + 1)
            .map_err(|error| error.to_string())?;
        drop(file);
        let store = ScanCacheStore::open(&root, "generation");
        assert!(store.is_empty());
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn stale_temp_is_removed_before_atomic_flush() -> Result<(), String> {
        let root = canonical_temp_dir().join(format!(
            "allow-rust-cache-stale-temp-{}-{}",
            std::process::id(),
            TEMP_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let stale = root.join(format!("{TEMP_FILE_PREFIX}stale"));
        let file = std::fs::File::create(&stale).map_err(|error| error.to_string())?;
        let old = SystemTime::now()
            .checked_sub(STALE_ARTIFACT_AGE + Duration::from_secs(1))
            .ok_or_else(|| "stale timestamp underflow".to_string())?;
        file.set_times(std::fs::FileTimes::new().set_modified(old))
            .map_err(|error| error.to_string())?;
        drop(file);

        let mut store = ScanCacheStore::open(&root, "generation");
        store.put(
            Path::new("src/lib.rs"),
            "digest".to_string(),
            false,
            Vec::new(),
        );
        assert!(store.flush());
        assert!(!stale.exists());
        assert!(root.join("scan-cache.v2.bin").exists());
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn active_writer_lock_is_retried_deterministically() -> Result<(), String> {
        let root = canonical_temp_dir().join(format!(
            "allow-rust-cache-lock-contention-{}-{}",
            std::process::id(),
            TEMP_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let lock = root.join(LOCK_FILE_NAME);
        let held = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock)
            .map_err(|error| error.to_string())?;
        held.lock().map_err(|error| error.to_string())?;
        let worker_root = root.clone();
        let (contention_tx, contention_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut store = ScanCacheStore::open(&worker_root, "generation");
            store.put(
                Path::new("src/worker.rs"),
                "worker".to_string(),
                false,
                Vec::new(),
            );
            let wait_hook = || {
                let _ = contention_tx.send(());
            };
            let flushed = store.flush_with_temp_path_and_wait_hook(None, Some(&wait_hook));
            Ok::<_, String>(flushed)
        });
        contention_rx.recv().map_err(|error| error.to_string())?;
        held.unlock().map_err(|error| error.to_string())?;
        drop(held);
        let flushed = worker.join().map_err(|_| "worker panicked".to_string())??;
        assert!(flushed);
        assert!(
            ScanCacheStore::open(&root, "generation")
                .get(Path::new("src/worker.rs"), "worker")
                .is_some()
        );
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn concurrent_flushes_are_serialized_and_last_writer_wins() -> Result<(), String> {
        let root = canonical_temp_dir().join(format!(
            "allow-rust-cache-concurrent-{}-{}",
            std::process::id(),
            TEMP_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let ready = std::sync::Arc::new(std::sync::Barrier::new(3));
        let mut workers = Vec::new();
        for index in 0..2_u8 {
            let worker_root = root.clone();
            let worker_ready = ready.clone();
            workers.push(std::thread::spawn(move || {
                let mut store = ScanCacheStore::open(&worker_root, "generation");
                store.put(
                    &PathBuf::from(format!("src/{index}.rs")),
                    format!("digest-{index}"),
                    false,
                    Vec::new(),
                );
                worker_ready.wait();
                store.flush()
            }));
        }
        ready.wait();
        for worker in workers {
            assert!(worker.join().map_err(|_| "worker panicked")?);
        }
        let store = ScanCacheStore::open(&root, "generation");
        // Both writers started from the same snapshot. The advisory cache
        // intentionally accepts last-writer wins instead of merging entries.
        assert_eq!(store.len(), 1);
        assert!(root.join("scan-cache.v2.bin").exists());
        assert!(root.join(LOCK_FILE_NAME).exists());
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn flush_fails_closed_when_cache_root_becomes_symlink() -> Result<(), String> {
        use std::os::unix::fs::symlink;

        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let root = canonical_temp_dir().join(format!(
            "allow-rust-cache-symlink-{}-{nonce}",
            std::process::id()
        ));
        let cache_dir = root.join("cache");
        let outside = root.join("outside");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&cache_dir).map_err(|error| error.to_string())?;
        std::fs::create_dir_all(&outside).map_err(|error| error.to_string())?;
        let mut store = ScanCacheStore::open(&cache_dir, "generation");
        store.put(
            Path::new("src/lib.rs"),
            "digest".to_string(),
            false,
            Vec::new(),
        );
        std::fs::remove_dir_all(&cache_dir).map_err(|error| error.to_string())?;
        symlink(&outside, &cache_dir).map_err(|error| error.to_string())?;
        assert!(!store.flush());
        assert!(!outside.join("scan-cache.v2.bin").exists());
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn precreated_temp_symlink_cannot_redirect_create_new_write() -> Result<(), String> {
        use std::os::unix::fs::symlink;

        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let root = canonical_temp_dir().join(format!(
            "allow-rust-cache-temp-symlink-{}-{nonce}",
            std::process::id()
        ));
        let outside = root.join("outside.bin");
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        std::fs::write(&outside, b"sentinel").map_err(|error| error.to_string())?;
        let nonce = TEMP_NONCE.fetch_add(1, Ordering::Relaxed);
        let temp = temp_path(&root, nonce);
        symlink(&outside, &temp).map_err(|error| error.to_string())?;
        let mut store = ScanCacheStore::open(&root, "generation");
        store.put(
            Path::new("src/lib.rs"),
            "digest".to_string(),
            false,
            Vec::new(),
        );
        assert!(!store.flush_with_temp_path(Some(&temp)));
        assert_eq!(
            std::fs::read(&outside).map_err(|error| error.to_string())?,
            b"sentinel"
        );
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn precreated_lock_symlink_is_rejected_without_outside_creation() -> Result<(), String> {
        use std::os::unix::fs::symlink;

        let root = canonical_temp_dir().join(format!(
            "allow-rust-cache-lock-symlink-{}-{}",
            std::process::id(),
            TEMP_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        let outside = root.join("outside.lock");
        let lock = root.join(LOCK_FILE_NAME);
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        symlink(&outside, &lock).map_err(|error| error.to_string())?;
        let mut store = ScanCacheStore::open(&root, "generation");
        store.put(
            Path::new("src/lib.rs"),
            "digest".to_string(),
            false,
            Vec::new(),
        );
        assert!(!store.flush());
        assert!(!outside.exists());
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }

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
        assert!(!store.flush_with_test_hooks(Some(&temp), None, Some(&replacement)));
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
        assert!(!store.flush_with_test_hooks(Some(&temp), None, Some(&replace_destination)));
        assert_eq!(
            std::fs::read(&dest).map_err(|error| error.to_string())?,
            b"replacement-destination"
        );
        assert!(!temp.exists());
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[cfg(unix)]
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

    #[cfg(windows)]
    #[test]
    fn junction_cache_root_is_rejected_without_outside_write() -> Result<(), String> {
        let root = canonical_temp_dir().join(format!(
            "allow-rust-cache-junction-{}-{}",
            std::process::id(),
            TEMP_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        let outside = root.join("outside");
        let junction = root.join("cache");
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        std::fs::create_dir_all(&outside).map_err(|error| error.to_string())?;
        let status = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                &junction.to_string_lossy(),
                &outside.to_string_lossy(),
            ])
            .status()
            .map_err(|error| error.to_string())?;
        if !status.success() {
            return Err("mklink /J failed; Windows reparse-point proof unavailable".to_string());
        }
        let mut store = ScanCacheStore::open(&junction, "generation");
        store.put(
            Path::new("src/lib.rs"),
            "digest".to_string(),
            false,
            Vec::new(),
        );
        assert!(!store.flush());
        assert!(!outside.join("scan-cache.v2.bin").exists());
        Ok(())
    }
}

#[cfg(test)]
#[path = "scan_cache_process_tests.rs"]
mod process_tests;
