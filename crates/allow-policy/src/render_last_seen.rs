use allow_core::LastSeen;

pub(crate) fn render_last_seen(out: &mut String, last_seen: &LastSeen) {
    out.push_str("\n[allow.last_seen]\n");
    out.push_str(&format!(
        "line = {}\ncolumn = {}\n",
        last_seen.line, last_seen.column
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_last_seen_writes_section_line_and_column() {
        let mut out = String::from("prefix\n");
        let last_seen = LastSeen {
            line: 42,
            column: 7,
        };

        render_last_seen(&mut out, &last_seen);

        assert_eq!(out, "prefix\n\n[allow.last_seen]\nline = 42\ncolumn = 7\n");
    }
}
