//! Byte-preserving refresh against the policy preimage used for selection.

use allow_core::{
    CargoAllowError, CargoAllowErrorKind, CargoAllowResult, LastSeen, sha256_v1_bytes,
};
use std::path::Path;

pub(super) fn refresh_bound_policy(
    path: &Path,
    expected_digest: &str,
    id: &str,
    location: &LastSeen,
) -> CargoAllowResult<String> {
    let bytes = crate::plan_bindings::read_bound_file(path, "policy")?;
    if sha256_v1_bytes(&bytes) != expected_digest {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "policy changed before selected coordinates could be refreshed (policy unchanged)",
        ));
    }
    let text = std::str::from_utf8(&bytes).map_err(|error| {
        CargoAllowError::with_kind(CargoAllowErrorKind::InvalidPolicy, error.to_string())
    })?;
    allow_policy::refresh_policy_entry(text, id, location)
}
