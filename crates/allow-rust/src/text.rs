use allow_core::normalize_snippet;

pub(crate) fn detect_attr<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let outer = format!("#[{name}(");
    let inner = format!("#![{name}(");
    if let Some(rest) = line.strip_prefix(&outer) {
        Some(rest)
    } else if let Some(rest) = line.strip_prefix(&inner) {
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

pub(crate) fn column(line: &str, needle: &str) -> u32 {
    line.find(needle).map(|idx| idx as u32 + 1).unwrap_or(1)
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
    let Some(prefix) = line.get(..pos) else {
        return String::new();
    };
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
