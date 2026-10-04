use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowErrorKind, CargoAllowResult,
    SOURCE_FILE_READ_MAX_BYTES,
};

use crate::render_entry::render_allow_entry;
use crate::render_sections::{
    render_lanes, render_policy_header, render_requirements, render_workspace,
};

pub fn render_policy(cfg: &AllowConfig) -> String {
    let mut out = String::new();
    render_policy_header(&mut out, cfg);
    render_workspace(&mut out, &cfg.workspace);
    render_requirements(&mut out, &cfg.requirements);
    render_lanes(&mut out, &cfg.lanes);
    for entry in &cfg.allow {
        render_allow_entry(&mut out, entry);
    }
    out
}

/// Append one canonical entry without changing any byte of an existing ledger.
///
/// Both the preimage and complete result are parsed and validated. TOML shapes
/// that cannot accept an array-table append (for example, `allow = []`) fail
/// explicitly; this never falls back to reformatting the existing document.
/// Entry paths must be representable as UTF-8 without replacement characters.
/// The complete result must remain within the normal policy loader's byte limit.
pub fn append_policy_entry(input: &str, entry: &AllowEntry) -> CargoAllowResult<String> {
    let mut expected = crate::parse_policy(input)?;
    if entry
        .path
        .as_ref()
        .is_some_and(|path| path.to_str().is_none())
    {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidPolicy,
            format!(
                "{} path must be valid UTF-8 to append without loss",
                entry.id
            ),
        ));
    }
    expected.allow.push(entry.clone());
    crate::validate_policy(&expected)?;

    // render_allow_entry starts with a newline, including when input ends in a
    // comment without a newline. Existing BOM, CRLF/LF, quotes and comments
    // remain untouched; only the appended block uses canonical LF formatting.
    let mut out = input.to_string();
    render_allow_entry(&mut out, entry);
    if out.len() as u64 > SOURCE_FILE_READ_MAX_BYTES {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidPolicy,
            format!(
                "appended policy would be {} bytes, exceeding the {}-byte read limit (policy unchanged)",
                out.len(),
                SOURCE_FILE_READ_MAX_BYTES,
            ),
        ));
    }
    let reparsed = crate::parse_policy(&out).map_err(|error| {
        error.with_message_prefix("cannot append an allow entry without rewriting policy: ")
    })?;
    // Compare the canonical projection because the existing renderer/parser
    // deliberately discard legacy selector.line_hint (it is not authority).
    if render_policy(&reparsed) != render_policy(&expected) {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidPolicy,
            "appended policy does not preserve the expected policy semantics",
        ));
    }
    Ok(out)
}
