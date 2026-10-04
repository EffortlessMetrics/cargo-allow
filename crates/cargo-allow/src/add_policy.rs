//! Byte-preserving append against the same preimage used for add validation.

use allow_core::{
    AllowEntry, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, sha256_v1_bytes,
};
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
