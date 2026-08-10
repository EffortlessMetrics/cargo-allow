use std::fs::{self, File};
use std::io::{Read, Take};
use std::path::Path;

use crate::error::{SnapshotError, SnapshotErrorKind, SnapshotResult};

pub const SOURCE_FILE_READ_MAX_BYTES: u64 = 8 * 1024 * 1024;

pub fn read_file_capped(path: &Path) -> SnapshotResult<Vec<u8>> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_file() && meta.len() > SOURCE_FILE_READ_MAX_BYTES => {
            Err(SnapshotError::with_kind(
                SnapshotErrorKind::Scan,
                format!(
                    "file {} exceeds the {}-byte source-read limit",
                    path.display(),
                    SOURCE_FILE_READ_MAX_BYTES
                ),
            ))
        }
        Ok(_) => {
            let file = File::open(path).map_err(|source| {
                SnapshotError::with_kind(
                    SnapshotErrorKind::Scan,
                    format!("failed to open source file {}: {source}", path.display()),
                )
            })?;
            let mut limited: Take<File> = file.take(SOURCE_FILE_READ_MAX_BYTES.saturating_add(1));
            let mut bytes = Vec::new();
            limited.read_to_end(&mut bytes).map_err(|source| {
                SnapshotError::with_kind(
                    SnapshotErrorKind::Scan,
                    format!("failed to read source file {}: {source}", path.display()),
                )
            })?;
            if (bytes.len() as u64) > SOURCE_FILE_READ_MAX_BYTES {
                return Err(SnapshotError::with_kind(
                    SnapshotErrorKind::Scan,
                    format!(
                        "file {} exceeds the {}-byte source-read limit",
                        path.display(),
                        SOURCE_FILE_READ_MAX_BYTES
                    ),
                ));
            }
            Ok(bytes)
        }
        Err(source) => Err(SnapshotError::with_kind(
            SnapshotErrorKind::Scan,
            format!("failed to inspect source file {}: {source}", path.display()),
        )),
    }
}

pub fn source_tree_path_is_ignored(path: &Path, patterns: &[String]) -> bool {
    let normalized = normalize_path(path);
    patterns.iter().any(|pattern| {
        let pattern = pattern.replace('\\', "/");
        glob_matches(&pattern, &normalized)
            || pattern.strip_suffix("/**").is_some_and(|prefix| {
                normalized == prefix || normalized.starts_with(&format!("{prefix}/"))
            })
    })
}

fn normalize_path(path: &Path) -> String {
    let mut parts = Vec::new();
    let text = path.to_string_lossy().replace('\\', "/");
    for part in text.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    parts.join("/")
}

fn glob_matches(pattern: &str, path: &str) -> bool {
    glob_match_tokens(
        &pattern
            .split('/')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>(),
        &path
            .split('/')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>(),
    )
}

fn glob_match_tokens(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((head, tail)) if *head == "**" => {
            glob_match_tokens(tail, path)
                || path
                    .split_first()
                    .is_some_and(|(_, tail_path)| glob_match_tokens(pattern, tail_path))
        }
        Some((head, tail)) => path.split_first().is_some_and(|(path_head, path_tail)| {
            segment_matches(head, path_head) && glob_match_tokens(tail, path_tail)
        }),
    }
}

fn segment_matches(pattern: &str, text: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let text = text.chars().collect::<Vec<_>>();
    segment_match_chars(&pattern, &text)
}

fn segment_match_chars(pattern: &[char], text: &[char]) -> bool {
    let Some((&pattern_head, pattern_tail)) = pattern.split_first() else {
        return text.is_empty();
    };
    match pattern_head {
        '*' => {
            segment_match_chars(pattern_tail, text)
                || text
                    .split_first()
                    .is_some_and(|(_, text_tail)| segment_match_chars(pattern, text_tail))
        }
        '?' => text
            .split_first()
            .is_some_and(|(_, text_tail)| segment_match_chars(pattern_tail, text_tail)),
        ch => text.split_first().is_some_and(|(&text_head, text_tail)| {
            text_head == ch && segment_match_chars(pattern_tail, text_tail)
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_path(label: &str) -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("clock before epoch: {error}"))?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!(
            "effortless-repo-snapshot-util-{label}-{}-{nanos}",
            std::process::id()
        )))
    }

    #[test]
    fn bounded_read_accepts_limit_and_rejects_oversized_files() -> Result<(), String> {
        let path = fixture_path("bounded")?;
        std::fs::write(&path, b"hello").map_err(|error| format!("write fixture: {error}"))?;
        let bytes = read_file_capped(&path).map_err(|error| error.to_string())?;
        if bytes != b"hello" {
            return Err("bounded read returned unexpected bytes".to_string());
        }
        std::fs::write(&path, vec![b'x'; (SOURCE_FILE_READ_MAX_BYTES as usize) + 1])
            .map_err(|error| format!("write oversized fixture: {error}"))?;
        if read_file_capped(&path).is_ok() {
            return Err("oversized read unexpectedly succeeded".to_string());
        }
        let _ = std::fs::remove_file(path);
        Ok(())
    }

    #[test]
    fn ignored_paths_match_recursive_and_segment_globs() -> Result<(), String> {
        let patterns = vec!["target/**".to_string(), "src/**/*.rs".to_string()];
        if !source_tree_path_is_ignored(Path::new("./target/debug/app"), &patterns) {
            return Err("recursive target pattern did not match".to_string());
        }
        if !source_tree_path_is_ignored(Path::new("src/bin/main.rs"), &patterns) {
            return Err("recursive Rust pattern did not match".to_string());
        }
        if source_tree_path_is_ignored(Path::new("src/bin/main.toml"), &patterns) {
            return Err("Rust pattern matched a non-Rust path".to_string());
        }
        Ok(())
    }
}
