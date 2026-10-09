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

/// The line-ending envelope detected in an existing policy document (#4279).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyLineEnding {
    /// `\r\n` dominates: a mutation transposes its rendered bytes to CRLF so a
    /// CRLF ledger stays CRLF instead of growing a mixed LF tail.
    Crlf,
    /// Lone `\n` dominates, or the document carries no line endings at all:
    /// a mutation keeps the canonical LF default.
    Lf,
    /// `\r\n` and lone `\n` counts tie with both present: the envelope is
    /// contested and mutating writers must refuse instead of guessing (#4337).
    Ambiguous,
}

/// Count the `\r\n` pairs and lone `\n` bytes of an existing document.
fn count_line_endings(bytes: &[u8]) -> (usize, usize) {
    let mut crlf = 0;
    let mut lone_lf = 0;
    let mut previous = 0u8;
    for &byte in bytes {
        if byte == b'\n' {
            if previous == b'\r' {
                crlf += 1;
            } else {
                lone_lf += 1;
            }
        }
        previous = byte;
    }
    (crlf, lone_lf)
}

/// Detect which line ending a mutation of an existing document should render
/// into, before any bytes are produced (#4279/#4337).
pub fn detect_dominant_line_ending(bytes: &[u8]) -> PolicyLineEnding {
    let (crlf, lone_lf) = count_line_endings(bytes);
    if crlf > lone_lf {
        PolicyLineEnding::Crlf
    } else if lone_lf > crlf || crlf == 0 {
        PolicyLineEnding::Lf
    } else {
        PolicyLineEnding::Ambiguous
    }
}

/// Transpose a canonically rendered, LF-only policy document to `ending`.
///
/// The renderer emits lone `\n` exclusively (string values escape control
/// characters), so this never touches escaped `\n`/`\r` sequences inside TOML
/// values. Any non-CRLF ending is returned unchanged; callers refuse
/// ambiguous envelopes before rendering.
pub fn transpose_policy_rendering(rendered: &str, ending: PolicyLineEnding) -> String {
    match ending {
        PolicyLineEnding::Crlf => rendered.replace('\n', "\r\n"),
        PolicyLineEnding::Lf | PolicyLineEnding::Ambiguous => rendered.to_string(),
    }
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

    // Match the ledger's dominant line ending so a CRLF ledger stays CRLF
    // instead of growing a mixed LF tail (#4279). A contested envelope fails
    // closed instead of guessing (#4337); the preimage is never rewritten.
    let ending = detect_dominant_line_ending(input.as_bytes());
    if ending == PolicyLineEnding::Ambiguous {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidPolicy,
            format!(
                "policy line endings are ambiguous (tied CRLF and lone LF counts); \
                 refusing to append {} without a dominant envelope (policy unchanged)",
                entry.id
            ),
        ));
    }

    // render_allow_entry starts with a newline, including when input ends in a
    // comment without a newline. Existing BOM, CRLF/LF, quotes and comments
    // remain untouched; only the appended block is transposed to the ledger's
    // dominant ending.
    let mut block = String::new();
    render_allow_entry(&mut block, entry);
    let mut out = input.to_string();
    out.push_str(&transpose_policy_rendering(&block, ending));
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
