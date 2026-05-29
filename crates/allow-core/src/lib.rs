mod date;
mod error;
mod finding;
mod fingerprint;
mod json;
mod policy;
mod source_tree_path;
pub use date::SimpleDate;
pub use error::{CargoAllowError, CargoAllowResult};
pub use finding::{
    Finding, FindingKind, STRUCTURAL_IDENTITY_SCHEMA_ID, Span, StructuralIdentity,
    finding_identity_key,
};
pub use fingerprint::{maybe_line_distance_score, normalize_snippet, stable_hash_hex};
pub use json::json_escape;
pub use policy::{
    AllowConfig, AllowEntry, LastSeen, Lifecycle, MatchOutcome, MatchStatus, Requirements,
    Selector, WorkspaceConfig,
};
pub use source_tree_path::{
    glob_matches, glob_matches_str, normalize_path, source_tree_path_is_ignored,
    source_tree_path_matches_filter, source_tree_scope_has_wildcard,
};

#[cfg(test)]
mod tests;
