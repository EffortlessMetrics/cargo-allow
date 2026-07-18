//! Bounded text-file reads for source-tree scanners and policy discovery.
//!
//! cargo-allow never executes project code, but it still reads tracked files
//! whole for syntax and policy inspection. Without a byte ceiling, a multi-GB
//! tracked file can OOM CI. These helpers fail closed on oversized files before
//! allocating the full contents.

use std::fs::{self, File};
use std::io::{Read, Take};
use std::path::Path;

/// Maximum bytes cargo-allow will load from one source-tree text file.
///
/// Chosen to keep ordinary Rust sources and policy/docs readable while rejecting
/// pathological tracked files that would force unbounded memory use.
pub const SOURCE_FILE_READ_MAX_BYTES: u64 = 8 * 1024 * 1024;

/// Why a capped text read failed.
#[derive(Debug)]
pub enum CappedReadError {
    Io(std::io::Error),
    Oversized { len: Option<u64>, limit: u64 },
    NotUtf8(std::string::FromUtf8Error),
}

impl std::fmt::Display for CappedReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::Oversized {
                len: Some(len),
                limit,
            } => {
                write!(
                    f,
                    "file is {len} bytes, which exceeds the {limit}-byte source-read limit"
                )
            }
            Self::Oversized { len: None, limit } => {
                write!(f, "file exceeds the {limit}-byte source-read limit")
            }
            Self::NotUtf8(err) => write!(f, "file is not valid UTF-8: {err}"),
        }
    }
}

impl std::error::Error for CappedReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::NotUtf8(err) => Some(err),
            Self::Oversized { .. } => None,
        }
    }
}

impl CappedReadError {
    pub fn is_oversized(&self) -> bool {
        matches!(self, Self::Oversized { .. })
    }
}

/// Read a UTF-8 text file only when its size is within [`SOURCE_FILE_READ_MAX_BYTES`].
pub fn read_text_file_capped(path: &Path) -> Result<String, CappedReadError> {
    read_text_file_capped_with_limit(path, SOURCE_FILE_READ_MAX_BYTES)
}

/// Read arbitrary file bytes within the source-tree per-file limit.
pub fn read_file_capped(path: &Path) -> Result<Vec<u8>, CappedReadError> {
    read_file_capped_with_limit(path, SOURCE_FILE_READ_MAX_BYTES)
}

/// Read a UTF-8 text file only when its size is within `limit` bytes.
///
/// Uses `symlink_metadata` for an early regular-file size check, then opens the
/// path and reads through a `Take` so symlink targets and TOCTOU races still
/// cannot allocate unbounded buffers.
pub fn read_text_file_capped_with_limit(
    path: &Path,
    limit: u64,
) -> Result<String, CappedReadError> {
    let bytes = read_file_capped_with_limit(path, limit)?;
    String::from_utf8(bytes).map_err(CappedReadError::NotUtf8)
}

/// Read arbitrary file bytes only when the file stays within `limit`.
pub fn read_file_capped_with_limit(path: &Path, limit: u64) -> Result<Vec<u8>, CappedReadError> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_file() && meta.len() > limit {
                return Err(CappedReadError::Oversized {
                    len: Some(meta.len()),
                    limit,
                });
            }
        }
        Err(err) => return Err(CappedReadError::Io(err)),
    }

    let file = File::open(path).map_err(CappedReadError::Io)?;
    let mut limited: Take<File> = file.take(limit.saturating_add(1));
    let mut bytes = Vec::new();
    limited
        .read_to_end(&mut bytes)
        .map_err(CappedReadError::Io)?;
    if (bytes.len() as u64) > limit {
        return Err(CappedReadError::Oversized { len: None, limit });
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "cargo-allow-capped-read-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    #[test]
    fn reads_small_utf8_files() {
        let path = temp_path("small");
        fs::write(&path, "hello\n")
            .unwrap_or_else(|err| std::panic::panic_any(format!("write small fixture: {err}")));
        let text = read_text_file_capped_with_limit(&path, 64).unwrap_or_else(|err| {
            std::panic::panic_any(format!("small read should succeed: {err}"))
        });
        assert_eq!(text, "hello\n");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rejects_oversized_files_before_full_allocation() {
        let path = temp_path("oversized");
        let limit = 64u64;
        let mut file = File::create(&path).unwrap_or_else(|err| {
            std::panic::panic_any(format!("create oversized fixture: {err}"))
        });
        file.write_all(&vec![b'a'; (limit as usize) + 1])
            .unwrap_or_else(|err| std::panic::panic_any(format!("write oversized: {err}")));
        drop(file);

        let err = read_text_file_capped_with_limit(&path, limit).unwrap_err();
        assert!(err.is_oversized(), "expected oversized, got {err}");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn accepts_file_exactly_at_limit() {
        let path = temp_path("exact");
        let limit = 32u64;
        fs::write(&path, vec![b'b'; limit as usize])
            .unwrap_or_else(|err| std::panic::panic_any(format!("write exact fixture: {err}")));
        let text = read_text_file_capped_with_limit(&path, limit).unwrap_or_else(|err| {
            std::panic::panic_any(format!("exact-limit read should succeed: {err}"))
        });
        assert_eq!(text.len() as u64, limit);
        let _ = fs::remove_file(&path);
    }
}
