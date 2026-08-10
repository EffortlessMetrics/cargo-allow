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
    let Some((pattern_index, pattern_head)) = pattern.char_indices().next() else {
        return text.is_empty();
    };
    match pattern_head {
        '*' => {
            let pattern_tail = &pattern[pattern_index + pattern_head.len_utf8()..];
            segment_matches(pattern_tail, text)
                || text
                    .char_indices()
                    .next()
                    .is_some_and(|(text_index, text_head)| {
                        segment_matches(pattern, &text[text_index + text_head.len_utf8()..])
                    })
        }
        '?' => text
            .char_indices()
            .next()
            .is_some_and(|(text_index, text_head)| {
                segment_matches(
                    &pattern[pattern_index + pattern_head.len_utf8()..],
                    &text[text_index + text_head.len_utf8()..],
                )
            }),
        ch => text
            .char_indices()
            .next()
            .is_some_and(|(text_index, text_head)| {
                text_head == ch
                    && segment_matches(
                        &pattern[pattern_index + pattern_head.len_utf8()..],
                        &text[text_index + text_head.len_utf8()..],
                    )
            }),
    }
}
