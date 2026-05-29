use allow_core::{glob_matches, normalize_path};
use std::path::Path;

use crate::path_info::lower_file_name;

pub(crate) fn is_generated_path(path: &Path, generated_patterns: &[String]) -> bool {
    let text = normalize_path(path);
    let file_name = lower_file_name(path);
    generated_patterns
        .iter()
        .any(|pattern| glob_matches(pattern, path))
        || text.contains("/generated/")
        || text.starts_with("generated/")
        || file_name.contains(".generated.")
        || file_name.ends_with(".generated")
        || text.contains("/gen/")
        || text.starts_with("gen/")
}
