use allow_core::normalize_path;
use std::path::Path;

pub fn source_tree_path_text(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if let Some(stripped) = text.strip_prefix("//?/UNC/") {
        return format!("//{stripped}");
    }
    if let Some(stripped) = text.strip_prefix("//?/") {
        return stripped.to_string();
    }
    if let Some(stripped) = text.strip_prefix("/?/") {
        return stripped.to_string();
    }
    normalize_path(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_tree_path_text_strips_windows_verbatim_prefix() {
        assert_eq!(
            source_tree_path_text(Path::new(r"\\?\H:\Code\Rust\cargo-allow")),
            "H:/Code/Rust/cargo-allow"
        );
        assert_eq!(
            source_tree_path_text(Path::new(r"\\?\UNC\server\share\repo")),
            "//server/share/repo"
        );
    }
}
