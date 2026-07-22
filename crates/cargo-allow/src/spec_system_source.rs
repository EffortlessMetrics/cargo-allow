//! Generic repository source view — canonical implementation in `repo-snapshot` (#2583-C).
//!
//! `cargo-allow` compiles the shared module via `include!` until `repo-snapshot` is
//! publishable and issue #2601 enables a normal dependency.

include!("../../repo-snapshot/src/source_view.rs");
