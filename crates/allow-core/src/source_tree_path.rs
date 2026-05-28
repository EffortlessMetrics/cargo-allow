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

pub fn source_tree_scope_has_wildcard(scope: &str) -> bool {
    scope
        .chars()
        .any(|ch| matches!(ch, '*' | '?' | '[' | ']' | '{' | '}'))
}

fn split_glob(s: &str) -> Vec<&str> {
    s.split('/').filter(|part| !part.is_empty()).collect()
}

fn glob_match_tokens(pattern: &[&str], path: &[&str]) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }
    if pattern[0] == "**" {
        if glob_match_tokens(&pattern[1..], path) {
            return true;
        }
        return !path.is_empty() && glob_match_tokens(pattern, &path[1..]);
    }
    if path.is_empty() {
        return false;
    }
    segment_matches(pattern[0], path[0]) && glob_match_tokens(&pattern[1..], &path[1..])
}

fn segment_matches(pattern: &str, text: &str) -> bool {
    segment_match_bytes(pattern.as_bytes(), text.as_bytes())
}

fn segment_match_bytes(pattern: &[u8], text: &[u8]) -> bool {
    if pattern.is_empty() {
        return text.is_empty();
    }
    match pattern[0] {
        b'*' => {
            segment_match_bytes(&pattern[1..], text)
                || (!text.is_empty() && segment_match_bytes(pattern, &text[1..]))
        }
        b'?' => !text.is_empty() && segment_match_bytes(&pattern[1..], &text[1..]),
        byte => {
            !text.is_empty() && byte == text[0] && segment_match_bytes(&pattern[1..], &text[1..])
        }
    }
}
