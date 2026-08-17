//! Changie compatibility-matrix contract tests (#3614): the support
//! claim is exactly as broad as the evidence, patch releases without
//! lane evidence are explicitly unsupported, dispositions are reviewed,
//! future versions fail visibly, and the public wording stays earned.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap_or_else(|| std::panic::panic_any("workspace root not resolvable"))
        .to_path_buf()
}

fn read_workspace_file(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read {rel}: {err}")))
}

fn matrix_text() -> String {
    read_workspace_file(&workspace_root(), "policy/changie-compatibility-matrix.toml")
}

fn release_blocks(text: &str) -> Vec<String> {
    text.split("[[release]]")
        .skip(1)
        .map(|block| block.to_string())
        .collect()
}

#[test]
fn matrix_pins_one_exact_supported_release() {
    // Negative control 1: `1.25.x` must not be claimed from one patch
    // release's evidence. Exactly one SupportedExperimental block may
    // exist, and it must name an exact semantic version.
    let text = matrix_text();
    let blocks = release_blocks(&text);
    let supported: Vec<&String> = blocks
        .iter()
        .filter(|block| block.contains("status = \"SupportedExperimental\""))
        .collect();
    assert_eq!(
        supported.len(),
        1,
        "the first reviewed target supports one exact release, not a range"
    );
    assert!(
        supported[0].contains("version = \"1.25.2\""),
        "the supported release is the one with a hosted evidence lane"
    );
    let forbidden_list = text
        .split("forbidden_wording = [")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .unwrap_or_default();
    let claim = text
        .split("supported_claim = \"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_default();
    assert!(
        !claim.contains("1.25.x"),
        "the supported claim names the exact release: {claim}"
    );
    assert!(
        forbidden_list.contains("1.25.x compatible"),
        "the blanket range claim is explicitly forbidden wording"
    );
}

#[test]
fn unevaluated_patch_releases_are_explicitly_unsupported() {
    let blocks = release_blocks(&matrix_text());
    let versions: BTreeSet<String> = blocks
        .iter()
        .filter_map(|block| {
            block
                .lines()
                .find(|line| line.trim_start().starts_with("version = "))
                .and_then(|line| line.split('"').nth(1).map(str::to_string))
        })
        .collect();
    assert_eq!(
        versions,
        BTreeSet::from([
            "1.25.0".to_string(),
            "1.25.1".to_string(),
            "1.25.2".to_string()
        ]),
        "the matrix enumerates the 1.25 line explicitly"
    );
    for block in &blocks {
        if block.contains("UnsupportedPendingEvidence") {
            assert!(
                block.contains("evidence = []"),
                "an unevaluated release must carry no evidence list"
            );
        }
    }
}

#[test]
fn every_dimension_result_is_classified_and_reviewed_differences_exist() {
    let text = matrix_text();
    for dimension in [
        "OfficialConfigSchema",
        "UpstreamConfigLoad",
        "UpstreamNewAuthoring",
        "UpstreamBatchLoad",
        "RustStaticCompanion",
        "SourceSafety",
    ] {
        assert!(
            text.contains(&format!("dimension = \"{dimension}\"")),
            "the matrix keeps the {dimension} authority separate"
        );
    }
    // No precedence flattens the dimensions: batch is separately
    // classified, not folded into config schema or new.
    assert!(text.contains("dimension = \"UpstreamBatchLoad\"\nresult = \"NotProvenSeparately\""));
    // Every difference carries a reviewed disposition.
    let difference_blocks: Vec<&str> = text
        .split("[[release.differences]]")
        .skip(1)
        .collect();
    assert!(!difference_blocks.is_empty());
    for block in &difference_blocks {
        assert!(
            block.contains("disposition = "),
            "unreviewed differences block promotion: {block}"
        );
        assert!(block.contains("reviewed = true"));
    }
}

#[test]
fn future_versions_fail_visibly_instead_of_inheriting() {
    // Negative control 7: unknown generations must not fall back to the
    // nearest supported one. The matrix states the behavior and the
    // sensor's generation pin must match the matrix generation.
    let text = matrix_text();
    assert!(text.contains("behavior = \"UnsupportedWithVisibleFailure\""));
    let generation_line = text
        .lines()
        .find(|line| line.trim_start().starts_with("compatibility_generation = "))
        .unwrap_or_else(|| std::panic::panic_any("generation line missing"));
    let generation = generation_line.split('"').nth(1).unwrap_or_default();
    assert_eq!(
        generation,
        allow_files::changie::CHANGIE_COMPATIBILITY_GENERATION,
        "the matrix and the sensor pin the same compatibility generation"
    );
}

#[test]
fn hosted_evidence_is_offline_in_normal_checks() {
    // Negative control 5: normal lint/build tests never fetch upstream
    // artifacts. The matrix references retained identities only; the
    // hosted lane is CI-owned. The matrix itself must not embed any
    // http-lookup instruction for normal runs.
    let text = matrix_text();
    assert!(!text.contains("curl"));
    assert!(!text.contains("go install"));
    // The module identity is a retained string, not an instruction.
    assert!(text.contains("module = \"github.com/miniscruff/changie@v1.25.2\""));
    // And the hosted pin in CI matches the supported release exactly.
    let ub_review = read_workspace_file(&workspace_root(), ".github/workflows/ub-review.yml");
    assert!(
        ub_review.contains("go install github.com/miniscruff/changie@v1.25.2"),
        "the hosted lane installs exactly the supported release"
    );
}

#[test]
fn upstream_collaboration_gate_blocks_promotion_until_recorded() {
    // Negative controls 10 and 11: the discussion is a maintainer
    // action, pending until recorded, and is never endorsement.
    let text = matrix_text();
    assert!(text.contains("status = \"PendingMaintainerAction\""));
    assert!(text.contains("required_for_promotion = true"));
    assert!(text.contains("discussion_url = \"\""));
    assert!(text.contains("no automation posts or mutates upstream"));
    // The forbidden wording list keeps the endorsement boundary.
    assert!(text.contains("\"official Changie support\""));
}

#[test]
fn public_wording_matches_the_supported_release_and_boundary() {
    // The earned terminology names the exact release and the runtime
    // boundary; docs and the capability row use compatible wording.
    let root = workspace_root();
    let how_to = read_workspace_file(&root, "docs/how-to/run-changie-sensor.md");
    assert!(
        how_to.contains("1.25"),
        "the operator guide names the compatibility generation"
    );
    assert!(
        how_to.contains("never executes Changie, renders templates,"),
        "the guide states the render boundary"
    );
    let readme = read_workspace_file(&root, "crates/allow-files/README.md");
    assert!(
        readme.contains("never says `changie batch` ran"),
        "the package README keeps the verbatim boundary"
    );
    // The capability catalog row stays experimental.
    let capabilities = read_workspace_file(&root, "crates/cargo-allow/src/capabilities.rs");
    assert!(capabilities.contains("experimental-opt-in"));
}

#[test]
fn matrix_schema_is_self_describing_and_versioned() {
    let text = matrix_text();
    assert!(text.contains("schema_id = \"cargo-allow.changie-compatibility-matrix.v1\""));
    assert!(text.contains("controlling_issue = 3614"));
    // The claim text names the exact release.
    assert!(
        text.contains("Changie 1.25.2-compatible configuration and fragments")
    );
}
