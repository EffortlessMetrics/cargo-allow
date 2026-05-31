#[test]
fn readme_preserves_source_tree_boundary() {
    let readme = include_str!("../../../README.md");

    for required_text in [
        "`cargo-allow` is a source-tree exception ledger for Rust repositories.",
        "`cargo-allow` scans repository files directly.",
        "the primary UX is the standalone `cargo-allow` binary",
        "does **not** require a successful build",
        "does **not** invoke\nCargo metadata, Cargo commands, rustc, Clippy, build scripts, proc macros",
        "`Cargo.toml` and `Cargo.lock` are files in the scanned source tree, not required\nbuild metadata.",
        "the value is not Cargo metadata or build-membership\nproof.",
        "No new unreceipted findings were found in scanned source-tree inventory.",
        "They must not claim that no unsafe, panic, lint suppression, or other exception\nexists outside the syntax-visible surface that was scanned.",
    ] {
        assert!(
            readme.contains(required_text),
            "README should preserve source-tree boundary text: {required_text}"
        );
    }
}
