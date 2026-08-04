/// Normalize CRLF to LF so drift tests pass regardless of the checkout's line
/// endings (Windows `core.autocrlf = true` converts LF to CRLF on checkout).
fn normalize_lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

#[test]
fn readme_preserves_source_tree_boundary() {
    let readme = normalize_lf(include_str!("../../../README.md"));

    for required_text in [
        "`cargo-allow` is a source-tree exception ledger and policy scanner for Rust repositories.",
        "`cargo-allow` scans repository files directly.",
        "the primary UX is the standalone `cargo-allow` binary",
        "does **not** require a successful build",
        "does **not** invoke\nCargo metadata, Cargo commands, rustc, Clippy, build scripts, proc macros,",
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
fn readmes_preserve_green_checkmark_logo() {
    let root_readme = normalize_lf(include_str!("../../../README.md"));
    let crate_readme = normalize_lf(include_str!("../README.md"));
    let logo = normalize_lf(include_str!(
        "../../../docs/assets/cargo-allow-checkmark.svg"
    ));

    assert!(
        root_readme.contains("docs/assets/cargo-allow-checkmark.svg"),
        "root README should show the local cargo-allow logo asset"
    );
    assert!(
        crate_readme.contains(
            "https://raw.githubusercontent.com/EffortlessMetrics/cargo-allow/main/docs/assets/cargo-allow-checkmark.svg"
        ),
        "crate README should use an absolute logo URL for published crate rendering"
    );
    for readme in [root_readme, crate_readme] {
        assert!(
            readme.contains("alt=\"cargo-allow green checkmark logo\""),
            "README logo image should keep descriptive alt text"
        );
        assert!(
            readme.contains("width=\"96\"") && readme.contains("height=\"96\""),
            "README logo image should keep stable dimensions"
        );
    }
    assert!(
        logo.contains("<title id=\"title\">cargo-allow green checkmark logo</title>")
            && logo.contains("fill=\"#1f9d55\""),
        "logo asset should remain a green checkmark SVG"
    );
}

#[test]
fn claim_boundary_docs_preserve_source_package_boundary() {
    let claim_boundaries = normalize_lf(include_str!("../../../docs/claim-boundaries.md"));

    for required_text in [
        "When cargo-allow reports `source_package`, that value is optional context read\nfrom source-tree `Cargo.toml` text when a readable `[package].name` is present.",
        "Invalid, unreadable, or non-UTF8 manifests are ignored for that context so the\nsource scan can continue; the value is not Cargo metadata or build-membership",
        "the value is not Cargo metadata or build-membership\nproof.",
    ] {
        assert!(
            claim_boundaries.contains(required_text),
            "claim boundary docs should preserve source_package boundary text: {required_text}"
        );
    }
}

#[test]
fn manage_an_exception_guide_keeps_command_parity_and_claim_boundary() {
    let guide = normalize_lf(include_str!("../../../docs/how-to/manage-an-exception.md"));
    let how_to_index = normalize_lf(include_str!("../../../docs/how-to/README.md"));
    let getting_started = normalize_lf(include_str!("../../../docs/getting-started.md"));
    let onboarding = normalize_lf(include_str!("../../../docs/onboarding.md"));

    for command in [
        "cargo-allow audit",
        "cargo-allow check",
        "cargo-allow list",
        "cargo-allow explain",
        "cargo-allow why",
        "cargo-allow worklist",
        "cargo-allow propose",
        "cargo-allow add",
        "cargo-allow refresh",
        "cargo-allow prune",
        "cargo-allow diff",
    ] {
        assert!(
            guide.contains(command),
            "manage-an-exception guide should name current command `{command}`"
        );
    }

    for required in [
        "--dry-run",
        "--write",
        "--require-change-note",
        "--write-change-note-template",
        "omit `--write`",
        "signals, not approval",
        "does not author rationale",
        "does not execute repository code",
        "Go issue first",
    ] {
        assert!(
            guide.contains(required),
            "manage-an-exception guide should preserve lifecycle/claim text: {required}"
        );
    }

    assert!(
        how_to_index.contains("manage-an-exception.md"),
        "how-to index should link the manage-an-exception guide"
    );
    assert!(
        how_to_index.contains("explain-why-a-finding.md"),
        "how-to index should link the why guide"
    );
    assert!(
        getting_started.contains("how-to/manage-an-exception.md"),
        "getting-started should route to manage-an-exception"
    );
    assert!(
        getting_started.contains("Choose a product channel"),
        "getting-started should require channel selection before first commands"
    );
    assert!(
        getting_started.contains("Choose ONE bootstrap path"),
        "getting-started should present init and propose as alternatives"
    );
    assert!(
        getting_started.contains("Illustrative only"),
        "getting-started must not present allow-0042 as a runnable repo example"
    );
    assert!(
        onboarding.contains("how-to/manage-an-exception.md"),
        "onboarding should route to manage-an-exception"
    );
}

#[test]
fn source_exception_operations_guide_covers_issue_1887_commands() -> Result<(), String> {
    let guide = normalize_lf(include_str!(
        "../../../docs/how-to/operate-source-exception-ledger.md"
    ));
    let how_to_index = normalize_lf(include_str!("../../../docs/how-to/README.md"));

    for command in [
        "`audit`",
        "`check`",
        "`init`",
        "`add`",
        "`refresh`",
        "check --mode no-new",
        "`no-new`",
        "`audit`",
        "`strict`",
        "`release`",
        "`--deny STATUS`",
        "`--dry-run`",
        "`--write`",
        "`--update`",
    ] {
        if !guide.contains(command) {
            return Err(format!(
                "source-exception operations guide should document {command}"
            ));
        }
    }

    for claim_boundary in [
        "They do not execute project",
        "A passing audit is not approval",
        "does not expand what the source scanner observes",
        "They do not prove runtime safety",
    ] {
        if !guide.contains(claim_boundary) {
            return Err(format!(
                "source-exception operations guide should preserve claim boundary: {claim_boundary}"
            ));
        }
    }

    if !how_to_index.contains("operate-source-exception-ledger.md") {
        return Err("how-to index should link the source-exception operations guide".to_string());
    }

    Ok(())
}

#[test]
fn ci_docs_preserve_source_tree_scan_boundary() {
    let ci = normalize_lf(include_str!("../../../docs/ci.md"));

    for required_text in [
        "The examples install and run the standalone `cargo-allow` binary before\nscanning. They pin the published crates.io release by default:",
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
