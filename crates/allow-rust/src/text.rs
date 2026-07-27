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
    // Split on commas, but skip commas inside double-quoted string literals
    // (#2659). A reason like `reason = "see policy: a, b"` should not produce
    // a spurious extra lint from the comma inside the string.
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;
    for ch in until_close.chars() {
        if escaped {
            escaped = false;
            current.push(ch);
            continue;
        }
        match ch {
            '\\' if in_string => {
                escaped = true;
                current.push(ch);
            }
            '"' => {
                in_string = !in_string;
                current.push(ch);
            }
            ',' if !in_string => {
                parts.push(std::mem::take(&mut current));
            }
            _ => current.push(ch),
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

pub(crate) fn source_column(source: &str, row: usize, byte_column: usize) -> u32 {
    // Fast path: find the start of the requested line by counting newlines
    // in the byte slice. This avoids the Lines iterator overhead which re-checks
    // for \r\n on every yield. Still O(row) per call, but with lower constant
    // factor. A full O(1) fix would require pre-computing line-start offsets
    // once per file scan — see #2666 for the batch approach.
    let bytes = source.as_bytes();
    let mut current_row = 0;
    let mut line_start = 0;
    for (i, &byte) in bytes.iter().enumerate() {
        if current_row == row {
            break;
        }
        if byte == b'\n' {
            current_row += 1;
            line_start = i + 1;
        }
    }
    if current_row < row {
        return 1;
    }
    // Find the line text from line_start to the next newline or end.
    let line_end = bytes
        .get(line_start..)
        .and_then(|slice| slice.iter().position(|&b| b == b'\n'))
        .map(|pos| line_start + pos)
        .unwrap_or(bytes.len());
    // Handle \r\n: strip trailing \r if present.
    let line_end = if line_end > line_start && bytes.get(line_end - 1) == Some(&b'\r') {
        line_end - 1
    } else {
        line_end
    };
    let line = source.get(line_start..line_end).unwrap_or("");
    byte_column_to_char_column(line, byte_column)
}

pub(crate) fn byte_column_to_char_column(line: &str, byte_column: usize) -> u32 {
    line.char_indices()
        .take_while(|(idx, _)| *idx < byte_column)
        .count() as u32
        + 1
}
