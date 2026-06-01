use allow_core::normalize_snippet;

pub(crate) fn detect_attr<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let outer = format!("#[{name}(");
    let inner = format!("#![{name}(");
    let bare = format!("{name}(");
    if let Some(rest) = line.strip_prefix(&outer) {
        Some(rest)
    } else if let Some(rest) = line.strip_prefix(&inner) {
        Some(rest)
    } else if let Some(rest) = line.strip_prefix(&bare) {
        Some(rest)
    } else {
        None
    }
}

pub(crate) fn extract_first_lint(text: &str) -> Option<String> {
    let until = text.split([',', ')']).next()?.trim();
    if until.is_empty() {
        None
    } else {
        Some(until.to_string())
    }
}

pub(crate) fn lint_policy_reference(text: &str) -> Option<String> {
    let (_, after) = text.split_once("policy:")?;
    let id = after
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
        .collect::<String>();
    if id.is_empty() { None } else { Some(id) }
}

#[cfg(test)]
pub(crate) fn column(line: &str, needle: &str) -> u32 {
    line.find(needle)
        .map(|idx| byte_column_to_char_column(line, idx))
        .unwrap_or(1)
}

pub(crate) fn source_column(source: &str, row: usize, byte_column: usize) -> u32 {
    source
        .lines()
        .nth(row)
        .map(|line| byte_column_to_char_column(line, byte_column))
        .unwrap_or(1)
}

pub(crate) fn byte_column_to_char_column(line: &str, byte_column: usize) -> u32 {
    line.char_indices()
        .take_while(|(idx, _)| *idx < byte_column)
        .count() as u32
        + 1
}

pub(crate) fn index_symbol(line: &str) -> String {
    let norm = normalize_snippet(line);
    norm.chars().take(100).collect()
}

pub(crate) fn index_target_fingerprint(line: &str) -> Option<String> {
    line.split('[').next().map(|s| {
        normalize_snippet(s)
            .chars()
            .rev()
            .take(40)
            .collect::<String>()
            .chars()
            .rev()
            .collect()
    })
}
