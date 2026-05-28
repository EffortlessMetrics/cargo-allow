use allow_core::normalize_snippet;

pub(crate) fn detect_attr<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let outer = format!("#[{name}(");
    let inner = format!("#![{name}(");
    if line.starts_with(&outer) {
        Some(&line[outer.len()..])
    } else if line.starts_with(&inner) {
        Some(&line[inner.len()..])
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

pub(crate) fn column(line: &str, needle: &str) -> u32 {
    line.find(needle).map(|idx| idx as u32 + 1).unwrap_or(1)
}

pub(crate) fn attribute_column(line: &str) -> u32 {
    line.find("#[")
        .or_else(|| line.find("#!["))
        .map_or(1, |idx| idx as u32 + 1)
}

pub(crate) fn receiver_before_method_column(line: &str, method_column: u32) -> String {
    let Some(dot_pos) = method_column.checked_sub(2).map(|pos| pos as usize) else {
        return String::new();
    };
    if dot_pos <= line.len() {
        receiver_before(line, dot_pos)
    } else {
        String::new()
    }
}

fn receiver_before(line: &str, pos: usize) -> String {
    let prefix = &line[..pos];
    let trimmed = normalize_snippet(prefix);
    trimmed
        .chars()
        .rev()
        .take(80)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

pub(crate) fn index_symbol(line: &str) -> String {
    let norm = normalize_snippet(line);
    norm.chars().take(100).collect()
}
