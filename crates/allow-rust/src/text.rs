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
    until_close
        .split(',')
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
