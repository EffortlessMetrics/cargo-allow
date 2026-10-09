//! Byte-preserving policy mutation writer helpers (#4279).

use allow_core::{
    AllowEntry, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, sha256_v1_bytes,
};
use allow_policy::{PolicyLineEnding, detect_dominant_line_ending, transpose_policy_rendering};
use std::path::Path;

pub(super) fn append_to_bound_policy(
    path: &Path,
    expected_digest: &str,
    entry: &AllowEntry,
) -> CargoAllowResult<String> {
    let bytes = crate::plan_bindings::read_bound_file(path, "policy")?;
    if sha256_v1_bytes(&bytes) != expected_digest {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "policy changed before the entry could be appended (policy unchanged)",
        ));
    }
    let text = std::str::from_utf8(&bytes).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidPolicy,
            format!("policy is not UTF-8: {error}"),
        )
    })?;
    allow_policy::append_policy_entry(text, entry)
}

/// Transpose a whole-document rendering to an existing target's dominant
/// line-ending envelope before overwriting it (#4279).
///
/// A missing target keeps the canonical LF default. An existing target with a
/// contested (tied) ending envelope refuses so the writer never guesses
/// (#4337); the target bytes stay untouched on every refusal.
pub(crate) fn transpose_for_existing_target(
    rendered: &str,
    target: &Path,
) -> CargoAllowResult<String> {
    let bytes = match std::fs::read(target) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(rendered.to_string());
        }
        Err(error) => {
            return Err(CargoAllowError::from(error).with_message_prefix(format!(
                "failed to read {} to detect its line-ending envelope: ",
                target.display()
            )));
        }
    };
    match detect_dominant_line_ending(&bytes) {
        PolicyLineEnding::Ambiguous => Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidPolicy,
            format!(
                "{} line endings are ambiguous (tied CRLF and lone LF counts); \
                 refusing to overwrite with a guessed envelope (target unchanged)",
                target.display()
            ),
        )),
        ending => Ok(transpose_policy_rendering(rendered, ending)),
    }
}
