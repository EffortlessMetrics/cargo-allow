use super::*;
use std::path::Path;

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
