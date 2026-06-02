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
        "No new unreceipted findings were found in scanned source-tree inventory.",
        "They must not claim that no unsafe, panic, lint suppression, or other exception\nexists outside the syntax-visible surface that was scanned.",
    ] {
        assert!(
            readme.contains(required_text),
            "README should preserve source-tree boundary text: {required_text}"
        );
    }
}

#[test]
fn claim_boundary_docs_preserve_source_package_boundary() {
    let claim_boundaries = include_str!("../../../docs/claim-boundaries.md");

    for required_text in [
        "When cargo-allow reports `source_package`, that value is optional context read\nfrom source-tree `Cargo.toml` text",
        "Invalid, unreadable, or non-UTF8 manifests are ignored for that context so the\nsource scan can continue",
        "the value is not Cargo metadata or build-membership\nproof.",
    ] {
        assert!(
            claim_boundaries.contains(required_text),
            "claim boundary docs should preserve source_package boundary text: {required_text}"
        );
    }
}

#[test]
fn ci_docs_preserve_source_tree_scan_boundary() {
    let ci = include_str!("../../../docs/ci.md");

    for required_text in [
        "The examples install and run the standalone `cargo-allow` binary before\nscanning.",
        "`cargo allow ...` remains optional Cargo external subcommand compatibility.",
        "The scan itself is source-tree only.",
        "It does not invoke Cargo metadata, Cargo\ncommands, rustc, Clippy, build scripts, proc macros, external evidence tools,\nor repository code.",
        "the policy\nscan should remain usable even when the checked-out repository does not build.",
        "This is reviewer guidance for source-syntax and policy-ledger posture.",
        "It does\nnot claim macro expansion, type information, build awareness, proof adequacy, or\ncoverage.",
    ] {
        assert!(
            ci.contains(required_text),
            "CI docs should preserve source-tree scan boundary text: {required_text}"
        );
    }
}
