//! Byte-preserving prune against the policy preimage used for selection.

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult, sha256_v1_bytes};
use std::path::Path;

pub(super) fn prune_bound_policy(
    path: &Path,
    expected_digest: &str,
    ids: &[&str],
) -> CargoAllowResult<String> {
    let bytes = crate::plan_bindings::read_bound_file(path, "policy")?;
    if sha256_v1_bytes(&bytes) != expected_digest {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "policy changed before selected entries could be pruned (policy unchanged)",
        ));
    }
    let text = std::str::from_utf8(&bytes).map_err(|error| {
        CargoAllowError::with_kind(CargoAllowErrorKind::InvalidPolicy, error.to_string())
    })?;
    allow_policy::prune_policy_entries(text, ids)
}
