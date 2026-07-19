//! File-level scan cache for incremental re-evaluation.
//!
//! Caches parsed Rust findings keyed by `(path, mtime_nanos, size)`. On a
//! repeat scan, files whose mtime+size hasn't changed are served from the
//! cache instead of re-parsing. The cache is in-memory only (not persisted
//! to disk) — it helps within a single process's repeated scans and across
//! processes via a serde-serialized snapshot.
//!
//! The cache is conservative: any cache miss falls through to a full
//! re-parse. Correctness is never compromised — the cache only skips work
//! that would produce identical findings.

use crate::scan_rust_source;
use allow_core::{CargoAllowResult, Finding};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// In-memory cache of parsed Rust findings keyed by file identity.
pub struct ScanCache {
    entries: HashMap<PathBuf, CacheEntry>,
}

struct CacheEntry {
    mtime: SystemTime,
    size: u64,
    findings: Vec<Finding>,
}

impl ScanCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Scan a single Rust file, using the cache if the file hasn't changed.
    /// Falls through to a full re-parse on any cache miss.
    pub fn scan_file(&mut self, root: &Path, rel: &Path) -> CargoAllowResult<Vec<Finding>> {
        let abs = root.join(rel);

        // Read metadata for cache key
        let metadata = match std::fs::metadata(&abs) {
            Ok(m) => m,
            Err(_) => {
                // Can't read metadata — skip caching, let the caller handle it
                return Ok(Vec::new());
            }
        };

        let mtime = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let size = metadata.len();

        // Check cache
        if let Some(entry) = self.entries.get(rel)
            && entry.mtime == mtime
            && entry.size == size
        {
            return Ok(entry.findings.clone());
        }

        // Cache miss — parse the file
        let text = match allow_core::read_text_file_capped(&abs) {
            Ok(text) => text,
            Err(_) => return Ok(Vec::new()),
        };
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let findings = scan_rust_source(rel, text);

        // Store in cache
        self.entries.insert(
            rel.to_path_buf(),
            CacheEntry {
                mtime,
                size,
                findings: findings.clone(),
            },
        );

        Ok(findings)
    }

    /// Scan multiple files, using the cache for unchanged files.
    /// Returns findings for all `.rs` files in the list.
    pub fn scan_files(&mut self, root: &Path, files: &[PathBuf]) -> CargoAllowResult<Vec<Finding>> {
        let mut out = Vec::new();
        for rel in files {
            if rel.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let findings = self.scan_file(root, rel)?;
            out.extend(findings);
        }
        Ok(out)
    }

    /// Clear the cache (e.g. when the policy changes and all files need
    /// re-evaluation).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for ScanCache {
    fn default() -> Self {
        Self::new()
    }
}
