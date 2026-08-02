use allow_core::CargoAllowError;
use std::path::Path;

/// Attach the legacy document and the best available TOML line to a semantic
/// parser or converter error.
///
/// The legacy adapters intentionally parse into ordinary `toml::Value`s, so
/// semantic errors do not retain parser spans. This small source map anchors
/// those errors to the relevant entry header or `id =` assignment without
/// changing the legacy data model or introducing a second parser.
pub(crate) fn at_legacy_source(
    error: CargoAllowError,
    path: &Path,
    source: &str,
) -> CargoAllowError {
    if error.location().is_some() {
        return error;
    }

    let offset = source_offset_for_error(error.message(), source);
    let line = line_number_at(source, offset);
    error
        .with_toml_span(Some(path), source, Some(offset..offset))
        .with_message_prefix(format!("legacy source {}:{}: ", path.display(), line))
}

fn source_offset_for_error(message: &str, source: &str) -> usize {
    if let Some(offset) = offset_for_matching_id(message, source) {
        return offset;
    }

    if let Some(index) = entry_index(message) {
        let header = if message.contains("workflow") || message.contains("baseline") {
            "[[entry]]"
        } else {
            "[[allow]]"
        };
        if let Some(offset) = nth_line_start(source, header, index) {
            return offset;
        }
    }

    if let Some(offset) = first_entry_header(source) {
        return offset;
    }

    source
        .lines()
        .enumerate()
        .find_map(|(index, line)| {
            let trimmed = line.trim();
            (!trimmed.is_empty() && !trimmed.starts_with('#')).then(|| line_start(source, index))
        })
        .unwrap_or(0)
}

fn offset_for_matching_id(message: &str, source: &str) -> Option<usize> {
    source.lines().enumerate().find_map(|(index, line)| {
        let value = assignment_string_value(line, "id")?;
        (!value.is_empty() && message.contains(value)).then(|| line_start(source, index))
    })
}

fn entry_index(message: &str) -> Option<usize> {
    [
        "allow entry ",
        "workflow entry ",
        "clippy exception entry ",
        "entry ",
    ]
    .iter()
    .find_map(|prefix| {
        let start = message.find(prefix)?.saturating_add(prefix.len());
        let digits = message
            .get(start..)?
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>();
        (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
    })
}

fn first_entry_header(source: &str) -> Option<usize> {
    source.lines().enumerate().find_map(|(index, line)| {
        let trimmed = line.trim();
        (trimmed == "[[allow]]" || trimmed == "[[entry]]").then(|| line_start(source, index))
    })
}

fn nth_line_start(source: &str, header: &str, wanted: usize) -> Option<usize> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| line.trim() == header)
        .nth(wanted)
        .map(|(index, _)| line_start(source, index))
}

fn assignment_string_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = line.trim_start().strip_prefix(key)?.trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    let quote = rest.chars().next()?;
    if !matches!(quote, '\'' | '"') {
        return None;
    }
    let value = rest.get(quote.len_utf8()..)?;
    let end = value.find(quote)?;
    value.get(..end)
}

fn line_start(source: &str, line_index: usize) -> usize {
    source
        .split_inclusive('\n')
        .take(line_index)
        .map(str::len)
        .sum()
}

fn line_number_at(source: &str, offset: usize) -> usize {
    let prefix = match source.get(..offset.min(source.len())) {
        Some(prefix) => prefix,
        None => source,
    };
    prefix
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        .saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::CargoAllowError;
    use std::path::Path;

    #[test]
    fn anchors_missing_field_to_matching_entry_id() -> Result<(), String> {
        let source = r#"policy = "network-allowlist"

[[allow]]
id = "net-first"
destination = "crates.io"

[[allow]]
id = "net-second"
destination = "api.github.com"
"#;
        let error = at_legacy_source(
            CargoAllowError::new("net-second missing auth_required"),
            Path::new("policy/network.toml"),
            source,
        );

        let location = error
            .location()
            .ok_or_else(|| "semantic error has source location".to_string())?;
        assert_eq!(location.path.as_deref(), Some("policy/network.toml"));
        assert_eq!(location.line, 8);
        assert!(
            error
                .message()
                .contains(&format!("{}:8", Path::new("policy/network.toml").display()))
        );
        Ok(())
    }

    #[test]
    fn anchors_indexed_shape_error_to_the_indexed_entry_header() {
        let source = "policy = \"workflow-allowlist\"\n[[entry]]\npath = \"one.yml\"\n[[entry]]\nowner = \"ci\"\n";
        let error = at_legacy_source(
            CargoAllowError::new("workflow entry 1 missing path"),
            Path::new("workflow.toml"),
            source,
        );

        assert_eq!(error.location().map(|location| location.line), Some(4));
        assert!(
            error
                .message()
                .contains(&format!("{}:4", Path::new("workflow.toml").display()))
        );
    }

    #[test]
    fn anchors_document_level_error_to_first_meaningful_line_when_no_entry_exists() {
        let source = "# legacy policy\npolicy = \"network-allowlist\"\n";
        let error = at_legacy_source(
            CargoAllowError::new("network-allowlist missing allow entries"),
            Path::new("network.toml"),
            source,
        );

        assert_eq!(error.location().map(|location| location.line), Some(2));
        assert!(
            error
                .message()
                .contains(&format!("{}:2", Path::new("network.toml").display()))
        );
    }

    #[test]
    fn preserves_existing_location_without_rewriting_context() -> Result<(), String> {
        let source = "policy = \"network-allowlist\"\n";
        let located = CargoAllowError::new("already located").with_toml_span(
            Some(Path::new("network.toml")),
            source,
            Some(0..0),
        );
        let enriched = at_legacy_source(located, Path::new("network.toml"), source);

        assert_eq!(enriched.message(), "already located");
        assert_eq!(enriched.location().map(|location| location.line), Some(1));
        Ok(())
    }

    #[test]
    fn falls_back_to_first_meaningful_line_when_no_entry_header_exists() {
        let source = "# comment\nowner = \"repo\"\n";
        let error = at_legacy_source(
            CargoAllowError::new("legacy document is incomplete"),
            Path::new("legacy.toml"),
            source,
        );

        assert_eq!(error.location().map(|location| location.line), Some(2));
    }

    #[test]
    fn handles_empty_source_and_unquoted_assignments_without_panicking() {
        let error = at_legacy_source(
            CargoAllowError::new("empty legacy document"),
            Path::new("empty.toml"),
            "",
        );

        assert_eq!(error.location().map(|location| location.line), Some(1));
        assert_eq!(assignment_string_value("id = 42", "id"), None);
        assert_eq!(assignment_string_value("id = \"unterminated", "id"), None);
        assert_eq!(entry_index("entry x missing path"), None);
        assert_eq!(line_number_at("source", usize::MAX), 1);
    }
}
