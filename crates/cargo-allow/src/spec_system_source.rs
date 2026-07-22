//! Generic repository source view — canonical implementation in `repo-snapshot` (#2583-C).
//!
//! `cargo-allow` keeps a package-local copy of `repo-snapshot::source_view` so published
//! crates can compile without an unpublished path dependency. Keep in sync via
//! `source_view_package_copy_matches_repo_snapshot`.

include!("spec_system_source_view.rs");
