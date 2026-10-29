//! Byte-preserving refresh against the policy preimage used for selection.

use allow_core::{
    CargoAllowError, CargoAllowErrorKind, CargoAllowResult, LastSeen, sha256_v1_bytes,
};
use std::path::Path;

pub(super) fn refresh_bound_policy(
    path: &Path,
    expected_digest: &str,
    updates: &[(&str, &LastSeen)],
) -> CargoAllowResult<String> {
    let bytes = crate::plan_bindings::read_bound_file(path, "policy")?;
    if sha256_v1_bytes(&bytes) != expected_digest {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "policy changed before selected coordinates could be refreshed (policy unchanged)",
        ));
    }
    let mut text = std::str::from_utf8(&bytes)
        .map_err(|error| {
            CargoAllowError::with_kind(CargoAllowErrorKind::InvalidPolicy, error.to_string())
        })?
        .to_owned();
    // Every entry still flows through the same per-entry byte-preserving
    // apply (`refresh_policy_entry`); chaining over one digest-checked
    // preimage keeps a bulk `--all` refresh a single atomic replacement
    // instead of N interleaved writes (#4651).
    for (id, location) in updates {
        text = allow_policy::refresh_policy_entry(&text, id, location)?;
    }
    Ok(text)
}
