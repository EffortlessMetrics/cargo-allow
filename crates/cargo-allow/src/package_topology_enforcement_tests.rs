//! Package topology enforcement contract (#3365).
//!
//! `policy/product-package-topology-v2.toml` is the one typed package
//! manifest: every workspace package must be classified in it, every row
//! must carry the release-manifest fields (ci_lane, support_tier,
//! asset_roots, extraction_destination), and the lane/CI validators
//! derive their crate sets from it rather than maintaining competing
//! arrays. intent-model's parser enforces field presence and vocabulary;
//! these tests enforce the tree-level contracts the parser cannot see.

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

fn postures(root: &Path) -> Vec<intent_model::GovernancePackagePostureV2> {
    let text = read_workspace_file(root, "policy/product-package-topology-v2.toml");
    intent_model::parse_package_postures_v1(&text)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse package topology: {err}")))
}

/// Workspace member package names, resolved through each member's own
/// manifest (directory names are not package names for two crates).
fn workspace_package_names(root: &Path) -> BTreeSet<String> {
    read_workspace_file(root, "Cargo.toml")
        .lines()
        .filter_map(|line| line.trim().strip_prefix("\"crates/"))
        .map(|line| line.trim_end_matches("\",").to_string())
        .map(|dir| {
            let manifest = read_workspace_file(root, &format!("crates/{dir}/Cargo.toml"));
            manifest
                .lines()
                .find_map(|line| line.trim().strip_prefix("name = \""))
                .map(|rest| rest.trim_end_matches('"').to_string())
                .unwrap_or_else(|| {
                    std::panic::panic_any(format!("crates/{dir}/Cargo.toml has no name"))
                })
        })
        .collect()
}

/// The no-new rejection (#3365): an unclassified new workspace package
/// fails here in both directions — an unclassified member and a stale
/// topology row are both errors.
#[test]
fn topology_classifies_every_workspace_package_exactly() {
    let root = workspace_root();
    let topology: BTreeSet<String> = postures(&root)
        .into_iter()
        .map(|row| row.cargo_package_name)
        .collect();
    let members = workspace_package_names(&root);
    let unclassified: BTreeSet<&String> = members.difference(&topology).collect();
    assert!(
        unclassified.is_empty(),
        "workspace packages missing from the topology manifest (classify them \
         in policy/product-package-topology-v2.toml, #3365): {unclassified:?}"
    );
    let stale: BTreeSet<&String> = topology.difference(&members).collect();
    assert!(
        stale.is_empty(),
        "topology rows with no workspace package (remove or correct them): {stale:?}"
    );
}

/// Field presence is enforced by the parser (missing keys fail to parse);
/// this is the discriminating negative proof for each release-manifest
/// field (#3365 criterion: a package missing a required field fails).
#[test]
fn missing_release_manifest_fields_fail_validation() {
    let base = r#"
[[package]]
logical_id = "fixture-pkg"
cargo_package_name = "fixture-pkg"
version_line = "cargo-allow-0.2"
product_family = "cargo-allow"
posture = "CargoAllowSupported"
package_version = "0.2.0"
version_source = "WorkspaceProduct"
publication_state = "UnpublishedInternal"
publish = true
candidate_inclusion = true
release_order = 10
ci_lane = "test"
support_tier = "supported"
asset_roots = []
extraction_destination = "cargo-allow"
"#;
    let parses = |text: &str| intent_model::parse_package_postures_v1(text).is_ok();
    assert!(parses(base), "complete fixture must parse");
    for missing in [
        "\nci_lane = \"test\"\n",
        "\nsupport_tier = \"supported\"\n",
        "\nasset_roots = []\n",
        "\nextraction_destination = \"cargo-allow\"\n",
    ] {
        let broken = base.replacen(missing.trim_start_matches('\n'), "", 1);
        assert!(
            !parses(&broken),
            "topology missing the field line {missing:?} must fail validation"
        );
    }
    // Vocabulary negatives: recognized enums only.
    for bad in [
        base.replace("support_tier = \"supported\"", "support_tier = \"ultra\""),
        base.replace(
            "extraction_destination = \"cargo-allow\"",
            "extraction_destination = \"cargo-side\"",
        ),
        base.replace("ci_lane = \"test\"", "ci_lane = \"\""),
    ] {
        assert!(
            !parses(&bad),
            "invalid release-manifest vocabulary must fail validation"
        );
    }
}

/// Every ci_lane named by the topology must be a real lane in the CI lane
/// manifest — the derivation in ci_lane_topology_tests silently drops
/// rows whose lane does not exist, so this catches typos directly.
#[test]
fn topology_ci_lanes_exist_in_the_lane_manifest() {
    let root = workspace_root();
    let lane_ids: BTreeSet<String> = {
        let text = read_workspace_file(&root, "docs/ci-lanes.toml");
        text.lines()
            .filter_map(|line| line.strip_prefix("id = "))
            .map(|value| value.trim().trim_matches('"').to_string())
            .collect()
    };
    for row in postures(&root) {
        assert!(
            lane_ids.contains(&row.ci_lane),
            "package {} names ci_lane {}, which is not a lane in docs/ci-lanes.toml",
            row.cargo_package_name,
            row.ci_lane
        );
    }
}

/// Declared asset roots must exist as workspace directories; an empty
/// list is the explicit no-assets declaration.
#[test]
fn asset_roots_exist_as_workspace_directories() {
    let root = workspace_root();
    for row in postures(&root) {
        for asset_root in &row.asset_roots {
            let path = root.join(asset_root);
            assert!(
                path.is_dir(),
                "package {} declares asset_root {asset_root}, which is not a directory",
                row.cargo_package_name
            );
        }
    }
}

/// Package support tiers must agree with the product support matrix by
/// family, and extraction destinations must follow the family split
/// (shared protocols remain hosted by cargo-allow, #2559).
#[test]
fn tiers_and_destinations_follow_the_product_split() {
    let root = workspace_root();
    let expected: &[(&str, &str, &str)] = &[
        ("cargo-allow", "supported", "cargo-allow"),
        ("cargo-intent", "experimental-opt-in", "cargo-intent"),
        ("cargo-proof", "experimental-opt-in", "cargo-proof"),
        ("shared", "internal-stabilizing", "cargo-allow"),
    ];
    for row in postures(&root) {
        let (_, tier, destination) = expected
            .iter()
            .find(|(family, _, _)| *family == row.product_family)
            .unwrap_or_else(|| {
                std::panic::panic_any(format!(
                    "package {} has unknown product_family {}",
                    row.cargo_package_name, row.product_family
                ))
            });
        assert_eq!(
            row.support_tier, *tier,
            "package {} support_tier must mirror the product support matrix",
            row.cargo_package_name
        );
        assert_eq!(
            row.extraction_destination, *destination,
            "package {} extraction_destination must follow the family split",
            row.cargo_package_name
        );
    }
}

/// A schema is core-owned when its title or identifier is
/// cargo-allow-namespaced (#3369). Used both over the live catalog and
/// over a seeded intent-branded fixture that must be rejected.
fn schema_is_core_owned(text: &str) -> bool {
    let title = text
        .split("\"title\"")
        .nth(1)
        .and_then(|rest| rest.split('\"').nth(1))
        .unwrap_or_default();
    if title.starts_with("cargo-allow") {
        return true;
    }
    text.contains("/schemas/cargo-allow") || text.contains("schemas/cargo-allow")
}

/// Package assets must not embed cargo-intent (or other product) schemas
/// as if core-owned (#3369). Seeded check: an intent-branded schema is
/// rejected by the same predicate that accepts the live catalog.
#[test]
fn asset_roots_embed_no_foreign_product_schemas() {
    let root = workspace_root();
    let mut checked = 0usize;
    for row in postures(&root) {
        for asset_root in &row.asset_roots {
            let dir = root.join(asset_root);
            let entries = fs::read_dir(&dir)
                .unwrap_or_else(|err| std::panic::panic_any(format!("read {asset_root}: {err}")));
            for entry in entries {
                let path = entry
                    .unwrap_or_else(|err| std::panic::panic_any(format!("entry: {err}")))
                    .path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let text = fs::read_to_string(&path)
                    .unwrap_or_else(|err| std::panic::panic_any(format!("read schema: {err}")));
                assert!(
                    schema_is_core_owned(&text),
                    "schema {} in asset root {asset_root} is not cargo-allow-namespaced",
                    path.display()
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "expected to check at least one schema");

    // Seeded negative: a cargo-intent-branded schema is foreign.
    let intent_branded = "{\"title\": \"cargo-intent governance v1\", \"$id\": \"https://effortlessmetrics.dev/schemas/cargo-intent/governance.v1.schema.json\"}";
    assert!(
        !schema_is_core_owned(intent_branded),
        "an intent-branded schema must not pass the core-ownership predicate"
    );
}
