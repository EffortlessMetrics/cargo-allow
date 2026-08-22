//! File-level scan cache for incremental re-evaluation.
//!
//! Caches parsed Rust findings keyed by `(path, mtime, size)`. On a
//! repeat scan within the same process, files whose mtime+size hasn't
//! changed are served from the cache instead of re-parsing.
//!
//! **Current state:** in-memory only (thread-local). A single CLI
//! invocation builds the cache, uses it once per file, and discards it
//! on process exit — so the current practical benefit is zero for the
//! standard `check` flow. The cache will pay off when a future CLI flow
//! does multiple scans per process (e.g. watch mode, LSP) or when disk
//! persistence with cross-version invalidation is added (#2523).
//!
//! The cache is conservative: any cache miss falls through to a full
//! re-parse. Correctness is never compromised — the cache only skips work
//! that would produce identical findings.

#[cfg(feature = "syntax")]
use crate::scan_rust_source_with_completeness;
use allow_core::{CargoAllowResult, Finding, sha256_v1_bytes};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::scan_cache_store::ScanCacheStore;

/// In-memory cache of parsed Rust findings keyed by file identity.
pub struct ScanCache {
    entries: HashMap<PathBuf, CacheEntry>,
}

struct CacheEntry {
    mtime: SystemTime,
    size: u64,
    content_digest: String,
    findings: Vec<Finding>,
    has_parse_error: bool,
}

impl ScanCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Scan a single Rust file, using the cache if the file hasn't changed.
    /// Falls through to a full re-parse on any cache miss.
    ///
    /// Returns `(findings, has_parse_error, skipped)`. When `skipped` is true,
    /// the file could not be read (oversized, binary, permission-denied) and
    /// the caller should count it as a skipped file (#2801).
    #[cfg(feature = "syntax")]
    pub fn scan_file(
        &mut self,
        root: &Path,
        rel: &Path,
    ) -> CargoAllowResult<(Vec<Finding>, bool, bool)> {
        let abs = root.join(rel);

        // Cold-cache fast path: if the cache is empty (first invocation),
        // skip the metadata() syscall entirely and go straight to read+parse.
        // The metadata is only needed to key the cache; on a cold cache there
        // is nothing to compare against, so the stat is pure overhead (#2839).
        if !self.entries.is_empty() {
            // Read metadata for cache key
            let metadata = match std::fs::metadata(&abs) {
                Ok(m) => m,
                Err(_) => {
                    // Can't read metadata — skip
                    return Ok((Vec::new(), false, true));
                }
            };

            let mtime = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let size = metadata.len();

            // Check cache
            if let Some(entry) = self.entries.get(rel)
                && entry.mtime == mtime
                && entry.size == size
            {
                return Ok((entry.findings.clone(), entry.has_parse_error, false));
            }
        }

        // Cache miss — read and parse the file
        let text = match allow_core::read_text_file_capped(&abs) {
            Ok(text) => text,
            Err(_) => return Ok((Vec::new(), false, true)),
        };
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let scan = scan_rust_source_with_completeness(rel, text);

        // Get metadata for cache key AFTER parsing (avoids redundant stat
        // on cold cache; on warm cache we already have it from above).
        let metadata = std::fs::metadata(&abs).ok();
        let mtime = metadata
            .as_ref()
            .and_then(|m| m.modified().ok())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let size = metadata.map(|m| m.len()).unwrap_or(0);

        // Store in cache
        self.entries.insert(
            rel.to_path_buf(),
            CacheEntry {
                mtime,
                size,
                content_digest: sha256_v1_bytes(text.as_bytes()),
                findings: scan.findings.clone(),
                has_parse_error: scan.has_parse_error,
            },
        );

        Ok((scan.findings, scan.has_parse_error, false))
    }

    /// Scan a single Rust file through the in-memory cache and the durable
    /// content-addressed store (#2571).
    ///
    /// The file is always read (the digest is the invalidation authority), but
    /// a digest match — in memory or on disk — skips the tree-sitter parse.
    /// Returns `(findings, has_parse_error, skipped)` with the same skipped
    /// semantics as [`ScanCache::scan_file`].
    #[cfg(feature = "syntax")]
    pub fn scan_file_with_store(
        &mut self,
        root: &Path,
        rel: &Path,
        store: &mut ScanCacheStore,
    ) -> CargoAllowResult<(Vec<Finding>, bool, bool)> {
        let abs = root.join(rel);
        let text = match allow_core::read_text_file_capped(&abs) {
            Ok(text) => text,
            Err(_) => return Ok((Vec::new(), false, true)),
        };
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let content_digest = sha256_v1_bytes(text.as_bytes());

        // In-memory hit: same evaluated bytes inside this invocation.
        if let Some(entry) = self.entries.get(rel)
            && entry.content_digest == content_digest
        {
            return Ok((entry.findings.clone(), entry.has_parse_error, false));
        }

        // Durable hit: a previous invocation parsed these exact bytes with
        // this scanner generation.
        if let Some((findings, has_parse_error)) = store.get(rel, &content_digest) {
            let metadata = std::fs::metadata(&abs).ok();
            self.entries.insert(
                rel.to_path_buf(),
                CacheEntry {
                    mtime: metadata
                        .as_ref()
                        .and_then(|m| m.modified().ok())
                        .unwrap_or(SystemTime::UNIX_EPOCH),
                    size: metadata.map(|m| m.len()).unwrap_or(0),
                    content_digest,
                    findings: findings.clone(),
                    has_parse_error,
                },
            );
            return Ok((findings, has_parse_error, false));
        }

        let scan = scan_rust_source_with_completeness(rel, text);
        let metadata = std::fs::metadata(&abs).ok();
        self.entries.insert(
            rel.to_path_buf(),
            CacheEntry {
                mtime: metadata
                    .as_ref()
                    .and_then(|m| m.modified().ok())
                    .unwrap_or(SystemTime::UNIX_EPOCH),
                size: metadata.map(|m| m.len()).unwrap_or(0),
                content_digest: content_digest.clone(),
                findings: scan.findings.clone(),
                has_parse_error: scan.has_parse_error,
            },
        );
        store.put(
            rel,
            content_digest,
            scan.has_parse_error,
            scan.findings.clone(),
        );

        Ok((scan.findings, scan.has_parse_error, false))
    }

    /// Scan multiple files, using the cache for unchanged files.
    /// Returns findings for all `.rs` files in the list.
    #[cfg(feature = "syntax")]
    pub fn scan_files(&mut self, root: &Path, files: &[PathBuf]) -> CargoAllowResult<Vec<Finding>> {
        let mut out = Vec::new();
        for rel in files {
            if rel.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let (findings, _has_parse_error, _skipped) = self.scan_file(root, rel)?;
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
