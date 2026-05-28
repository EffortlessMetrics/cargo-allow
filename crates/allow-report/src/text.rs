use allow_core::json_escape;

pub(crate) fn markdown_inline_code(value: &str) -> String {
    json_escape(value).replace('`', "\\`")
}

pub(crate) fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('`', "\\`")
}

pub(crate) fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
