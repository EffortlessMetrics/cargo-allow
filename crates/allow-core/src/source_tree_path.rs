use std::path::Path;

pub fn normalize_path(path: impl AsRef<Path>) -> String {
    let text = path.as_ref().to_string_lossy().replace('\\', "/");
    let absolute = text.starts_with('/');
    let mut parts = Vec::new();
    for part in text.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|part| *part != "..") {
                    parts.pop();
                } else if !absolute {
                    parts.push(part);
                }
            }
            other => parts.push(other),
        }
    }
    let normalized = parts.join("/");
    if absolute {
        format!("/{normalized}")
    } else {
        normalized
    }
}

pub fn glob_matches(pattern: &str, path: &Path) -> bool {
    let path = normalize_path(path);
    glob_matches_str(pattern, &path)
}

pub fn glob_matches_str(pattern: &str, path: &str) -> bool {
    let p = pattern.replace('\\', "/");
    glob_match_tokens(&split_glob(&p), &split_glob(path))
}

pub fn source_tree_path_matches_filter(item_path: &str, filter_path: &str) -> bool {
    let item_path = normalize_path(item_path);
    let filter_path = normalize_path(filter_path);
    let filter_path = filter_path.trim_end_matches('/');
    if filter_path.is_empty() || filter_path == "." {
        return true;
    }
    item_path == filter_path
        || item_path
            .strip_prefix(filter_path)
            .map(|suffix| suffix.starts_with('/'))
            .unwrap_or(false)
        || (source_tree_scope_has_wildcard(&item_path) && glob_matches_str(&item_path, filter_path))
}

pub fn source_tree_path_is_ignored(path: impl AsRef<Path>, patterns: &[String]) -> bool {
    let path = path.as_ref();
    let normalized = normalize_path(path);
    patterns.iter().any(|pattern| {
        glob_matches(pattern, path)
            || pattern
                .strip_suffix("/**")
                .map(|prefix| {
                    let prefix = normalize_path(prefix);
                    normalized == prefix || normalized.starts_with(&format!("{prefix}/"))
                })
                .unwrap_or(false)
    })
}

pub fn source_tree_scope_has_wildcard(scope: &str) -> bool {
    scope
        .chars()
        .any(|ch| matches!(ch, '*' | '?' | '[' | ']' | '{' | '}'))
}

fn split_glob(s: &str) -> Vec<&str> {
    s.split('/').filter(|part| !part.is_empty()).collect()
}

fn glob_match_tokens(pattern: &[&str], path: &[&str]) -> bool {
    let Some((pattern_head, pattern_tail)) = pattern.split_first() else {
        return path.is_empty();
    };
    if *pattern_head == "**" {
        if glob_match_tokens(pattern_tail, path) {
            return true;
        }
        return path
            .split_first()
            .is_some_and(|(_, path_tail)| glob_match_tokens(pattern, path_tail));
    }
    path.split_first().is_some_and(|(path_head, path_tail)| {
        segment_matches(pattern_head, path_head) && glob_match_tokens(pattern_tail, path_tail)
    })
}

fn segment_matches(pattern: &str, text: &str) -> bool {
    segment_match_bytes(pattern.as_bytes(), text.as_bytes())
}

fn segment_match_bytes(pattern: &[u8], text: &[u8]) -> bool {
    let Some((&pattern_head, pattern_tail)) = pattern.split_first() else {
        return text.is_empty();
    };
    match pattern_head {
        b'*' => {
            segment_match_bytes(pattern_tail, text)
                || text
                    .split_first()
                    .is_some_and(|(_, text_tail)| segment_match_bytes(pattern, text_tail))
        }
        b'?' => text
            .split_first()
            .is_some_and(|(_, text_tail)| segment_match_bytes(pattern_tail, text_tail)),
        byte => text.split_first().is_some_and(|(&text_head, text_tail)| {
            byte == text_head && segment_match_bytes(pattern_tail, text_tail)
        }),
    }
}
