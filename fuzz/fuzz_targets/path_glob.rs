#![no_main]

use allow_core::{
    glob_matches_str, normalize_path, source_tree_path_is_ignored, source_tree_path_matches_filter,
    source_tree_scope_has_wildcard,
};
use libfuzzer_sys::fuzz_target;
use std::path::Path;

fn split_fields(data: &[u8]) -> Option<(&str, &str)> {
    let (&split_byte, rest) = data.split_first()?;
    let split = usize::from(split_byte).min(rest.len());
    let left = std::str::from_utf8(&rest[..split]).ok()?;
    let right = std::str::from_utf8(&rest[split..]).ok()?;
    Some((left, right))
}

fuzz_target!(|data: &[u8]| {
    let Some((pattern, path)) = split_fields(data) else {
        return;
    };

    let normalized = normalize_path(path);
    assert_eq!(normalize_path(&normalized), normalized);

    let _ = glob_matches_str(pattern, path);
    let _ = glob_matches_str(pattern, &normalized);
    let _ = source_tree_path_matches_filter(path, pattern);
    let _ = source_tree_scope_has_wildcard(pattern);
    let _ = source_tree_path_is_ignored(Path::new(path), &[pattern.to_string()]);
});
