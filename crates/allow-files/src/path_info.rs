use std::path::Path;

pub(crate) fn lower_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
}

pub(crate) fn lower_file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase())
        .unwrap_or_default()
}

pub(crate) fn fingerprint(path: &Path) -> Option<String> {
    lower_extension(path).or_else(|| {
        let file_name = lower_file_name(path);
        (!file_name.is_empty()).then_some(file_name)
    })
}
