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
//! - The store is **facts, not authority**: every entry is validated against
//!   the current file digest before use, so a corrupted, truncated, or
//!   tampered store can only cause a cold re-scan, never wrong findings.
//! - Any read/decode failure discards the store (fail-open cold start).
//! - A generation string binds entries to one scanner build; scanner changes
//!   invalidate the whole store rather than silently mixing semantics.
//! - Skipped files (oversized, non-UTF-8, unreadable) are never persisted.
//!
//! The store lives under `target/`, which cargo gitignores, so persisted
//! facts never enter source-tree scans or receipts.

use allow_core::Finding;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Store schema identifier; also the on-disk magic header line.
const STORE_SCHEMA: &[u8] = b"cargo-allow.scan-cache.v2";
const CHECKSUM_LEN: usize = 74;
const MAX_ENTRY_COUNT: usize = 100_000;
const MAX_FINDINGS_PER_ENTRY: usize = 100_000;
const MAX_FIELD_BYTES: usize = 16 * 1024 * 1024;
const MAX_STORE_BYTES: usize = 128 * 1024 * 1024;

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
        let mut store = Self {
            dir: dir.to_path_buf(),
            generation,
            entries: HashMap::new(),
            dirty: false,
        };
        if path_has_symlink_component(dir) {
            return store;
        }
        let path = store.store_path();
        if let Ok(bytes) = std::fs::read(&path)
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

    /// Persist pending changes atomically enough for a best-effort cache:
    /// write a sibling temp file, then replace the store. Failure returns
    /// `false`; callers treat flush as advisory.
    pub fn flush(&mut self) -> bool {
        if !self.dirty {
            return true;
        }
        if std::fs::create_dir_all(&self.dir).is_err() {
            return false;
        }
        let bytes = match encode_store(&self.generation, &self.entries) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        let dest = self.store_path();
        let tmp = self
            .dir
            .join(format!("scan-cache.v2.bin.tmp-{}", std::process::id()));
        let write = std::fs::File::create(&tmp).and_then(|mut file| {
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
        if dest.exists() {
            let _ = std::fs::remove_file(&dest);
        }
        let moved = std::fs::rename(&tmp, &dest);
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
}

fn path_has_symlink_component(path: &Path) -> bool {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if std::fs::symlink_metadata(&current)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

fn write_str(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn write_opt_u32(out: &mut Vec<u8>, value: Option<u32>) {
    match value {
        Some(value) => {
            out.push(1);
            out.extend_from_slice(&value.to_le_bytes());
        }
        None => out.push(0),
    }
}

fn encode_store(generation: &str, entries: &HashMap<PathBuf, StoredEntry>) -> Result<Vec<u8>, ()> {
    // Deterministic order so byte-identical states produce identical files.
    let mut ordered: Vec<(&PathBuf, &StoredEntry)> = entries.iter().collect();
    ordered.sort_by(|left, right| left.0.cmp(right.0));

    let mut out = Vec::new();
    out.extend_from_slice(STORE_SCHEMA);
    out.push(b'\n');
    write_str(&mut out, generation);
    out.extend_from_slice(&(ordered.len() as u32).to_le_bytes());
    for (rel, entry) in ordered {
        let rel_text = rel.to_str().ok_or(())?;
        write_str(&mut out, rel_text);
        write_str(&mut out, &entry.content_digest);
        out.push(u8::from(entry.has_parse_error));
        out.extend_from_slice(&(entry.findings.len() as u32).to_le_bytes());
        for finding in &entry.findings {
            encode_finding(&mut out, finding)?;
        }
    }
    if out.len() > MAX_STORE_BYTES {
        return Err(());
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
            write_str(out, family);
        }
        None => out.push(0),
    }
    let path_text = finding.path.to_str().ok_or(())?;
    write_str(out, path_text);
    match &finding.span {
        Some(span) => {
            out.push(1);
            out.extend_from_slice(&span.line.to_le_bytes());
            out.extend_from_slice(&span.column.to_le_bytes());
        }
        None => out.push(0),
    }
    let identity = &finding.identity;
    write_str(out, &identity.language);
    write_str(out, &identity.ast_kind);
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
                out.push(1);
                write_str(out, text);
            }
            None => out.push(0),
        }
    }
    write_opt_u32(out, identity.line_hint);
    write_opt_u32(out, identity.column_hint);
    write_str(out, &finding.message);
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
        let len = self.u32()? as usize;
        if len > MAX_FIELD_BYTES {
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
        let rel = reader.str()?;
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
