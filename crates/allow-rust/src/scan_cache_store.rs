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
use std::io::{ErrorKind, Write};
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
        let Some(_lock) = WriterLock::acquire(&self.dir) else {
            return false;
        };
        remove_stale_artifacts(&self.dir);
        let bytes = match encode_store(&self.generation, &self.entries) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        let dest = self.store_path();
        let tmp = self.dir.join(format!(
            "{TEMP_FILE_PREFIX}{}-{}",
            std::process::id(),
            TEMP_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        let write = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .and_then(|mut file| {
                file.write_all(&bytes)?;
                file.sync_all()
            });
        if write.is_err() {
            let _ = std::fs::remove_file(&tmp);
            return false;
        }
        // std::fs::rename does not replace an existing destination on
        // Windows; drop the previous store first. The cache is advisory, so
        // a lost race here only costs a future cold start.
        if path_is_unsafe(&dest) {
            let _ = std::fs::remove_file(&tmp);
            return false;
        }
        let mut moved = std::fs::rename(&tmp, &dest);
        if moved.is_err() {
            if path_is_unsafe(&dest) || std::fs::remove_file(&dest).is_err() {
                let _ = std::fs::remove_file(&tmp);
                return false;
            }
            moved = std::fs::rename(&tmp, &dest);
        }
        if moved.is_err() {
            let _ = std::fs::remove_file(&tmp);
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

struct WriterLock {
    path: PathBuf,
}

impl WriterLock {
    fn acquire(dir: &Path) -> Option<Self> {
        let path = dir.join(LOCK_FILE_NAME);
        for _ in 0..100 {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(_) => return Some(Self { path }),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    if !remove_if_stale(&path) {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                }
                Err(_) => return None,
            }
        }
        None
    }
}

impl Drop for WriterLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
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
        let root = std::env::temp_dir().join(format!(
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
        let root = std::env::temp_dir().join(format!(
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
    fn concurrent_flushes_leave_a_decodable_store() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "allow-rust-cache-concurrent-{}-{}",
            std::process::id(),
            TEMP_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let mut workers = Vec::new();
        for index in 0..8_u8 {
            let worker_root = root.clone();
            workers.push(std::thread::spawn(move || {
                let mut store = ScanCacheStore::open(&worker_root, "generation");
                store.put(
                    &PathBuf::from(format!("src/{index}.rs")),
                    format!("digest-{index}"),
                    false,
                    Vec::new(),
                );
                store.flush()
            }));
        }
        for worker in workers {
            assert!(worker.join().map_err(|_| "worker panicked")?);
        }
        let store = ScanCacheStore::open(&root, "generation");
        assert!(!store.is_empty());
        assert!(root.join("scan-cache.v2.bin").exists());
        assert!(!root.join(LOCK_FILE_NAME).exists());
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
        let root = std::env::temp_dir().join(format!(
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
        let root = std::env::temp_dir().join(format!(
            "allow-rust-cache-temp-symlink-{}-{nonce}",
            std::process::id()
        ));
        let outside = root.join("outside.bin");
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        std::fs::write(&outside, b"sentinel").map_err(|error| error.to_string())?;
        let temp = root.join(format!("scan-cache.v2.bin.tmp-{}", std::process::id()));
        symlink(&outside, &temp).map_err(|error| error.to_string())?;
        let mut store = ScanCacheStore::open(&root, "generation");
        store.put(
            Path::new("src/lib.rs"),
            "digest".to_string(),
            false,
            Vec::new(),
        );
        assert!(!store.flush());
        assert_eq!(
            std::fs::read(&outside).map_err(|error| error.to_string())?,
            b"sentinel"
        );
        std::fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(())
    }
}
