pub(crate) fn detect_attr<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let outer = format!("#[{name}");
    let inner = format!("#![{name}");
    [outer.as_str(), inner.as_str(), name]
        .into_iter()
        .find_map(|prefix| line.strip_prefix(prefix))
        .and_then(|rest| rest.trim_start().strip_prefix('('))
}

pub(crate) fn extract_lints(text: &str) -> Vec<String> {
    // Walk the text tracking paren depth so that nested parens (e.g.
    // #[allow(clippy::foo(bar))] or #[expect(clippy::baz(reason = "x)y"))])
    // don't truncate the lint list at the inner ')' (#1879).
    let until_close: String = {
        let mut depth = 1i32; // we're already inside the outer (
        let mut out = String::new();
        for ch in text.chars() {
            match ch {
                '(' => {
                    depth += 1;
                    out.push(ch);
                }
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push(ch);
                }
                _ => out.push(ch),
            }
        }
        out
    };
    // Split on commas, but skip commas inside string literals (#2659, #2780).
    // Handles plain "..." with \ escapes, raw strings r"..." (no escapes),
    // and raw strings with hashes r#"..."# (matched hash count).
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut chars = until_close.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '"' {
            // Plain string literal
            current.push('"');
            loop {
                match chars.next() {
                    Some('\\') => {
                        current.push('\\');
                        if let Some(&next) = chars.peek() {
                            current.push(next);
                            chars.next();
                        }
                    }
                    Some('"') => {
                        current.push('"');
                        break;
                    }
                    Some(c) => current.push(c),
                    None => break,
                }
            }
            continue;
        }
        if ch == 'r' && matches!(chars.peek(), Some('"') | Some('#')) {
            // Raw string literal: r"...", r#"..."#, r##"..."##, etc.
            let mut hashes = 0;
            current.push('r');
            while chars.peek() == Some(&'#') {
                hashes += 1;
                current.push('#');
                chars.next();
            }
            if chars.peek() == Some(&'"') {
                current.push('"');
                chars.next();
                // Scan until closing " followed by N hashes
                loop {
                    match chars.next() {
                        Some('"') => {
                            let mut close_hashes = 0;
                            while close_hashes < hashes && chars.peek() == Some(&'#') {
                                close_hashes += 1;
                                current.push('#');
                                chars.next();
                            }
                            if close_hashes == hashes {
                                current.push('"');
                                break;
                            } else {
                                // Not enough hashes — this " was inside the raw string
                                current.insert(current.len() - close_hashes, '"');
                            }
                        }
                        Some(c) => current.push(c),
                        None => break,
                    }
                }
                continue;
            }
            // 'r' followed by '#' but not '"', treat as normal char
        }
        if ch == ',' {
            parts.push(std::mem::take(&mut current));
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
        .into_iter()
        .filter_map(|part| {
            let lint = part.trim();
            if lint.is_empty()
                || lint
                    .split('=')
                    .next()
                    .is_some_and(|name| name.trim() == "reason")
            {
                None
            } else {
                Some(lint.to_string())
            }
        })
        .collect()
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceLineIndex {
    line_starts: Vec<usize>,
}

impl SourceLineIndex {
    pub(crate) fn new(source: &str) -> Self {
        let mut line_starts = vec![0];
        for (index, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(index + 1);
            }
        }
        Self { line_starts }
    }

    pub(crate) fn source_column(&self, source: &str, row: usize, byte_column: usize) -> u32 {
        let Some(&line_start) = self.line_starts.get(row) else {
            return 1;
        };
        let raw_line_end = self
            .line_starts
            .get(row + 1)
            .copied()
            .unwrap_or(source.len());
        let bytes = source.as_bytes();
        let mut line_end = raw_line_end;
        if line_end > line_start && bytes.get(line_end - 1) == Some(&b'\n') {
            line_end -= 1;
        }
        if line_end > line_start && bytes.get(line_end - 1) == Some(&b'\r') {
            line_end -= 1;
        }
        let line = source.get(line_start..line_end).unwrap_or("");
        byte_column_to_char_column(line, byte_column)
    }
}

pub(crate) fn source_column(
    line_index: &SourceLineIndex,
    source: &str,
    row: usize,
    byte_column: usize,
) -> u32 {
    line_index.source_column(source, row, byte_column)
}

pub(crate) fn byte_column_to_char_column(line: &str, byte_column: usize) -> u32 {
    line.char_indices()
        .take_while(|(idx, _)| *idx < byte_column)
        .count() as u32
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_line_index_preserves_unicode_crlf_and_missing_rows() {
        let source = "zero\n  café\r\nthird\n";
        let index = SourceLineIndex::new(source);

        assert_eq!(index.source_column(source, 0, 0), 1);
        assert_eq!(index.source_column(source, 1, 5), 6);
        assert_eq!(index.source_column(source, 2, 2), 3);
        assert_eq!(index.source_column(source, 4, 0), 1);
    }
}
