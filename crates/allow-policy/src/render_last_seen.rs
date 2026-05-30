use allow_core::LastSeen;

pub(crate) fn render_last_seen(out: &mut String, last_seen: &LastSeen) {
    out.push_str("\n[allow.last_seen]\n");
    out.push_str(&format!(
        "line = {}\ncolumn = {}\n",
        last_seen.line, last_seen.column
    ));
}
