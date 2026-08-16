//! CI lane topology contract (#3358).
//!
//! `docs/ci-lanes.toml` is the checked authority for the CI lane topology
//! and gating posture; this test validates `.github/workflows/ci.yml`
//! against it. The load-bearing property is the seeded-failure proof: an
//! experimental product lane failure must be unable to block a
//! cargo-allow required lane — expressed as job disjointness, absence of
//! `needs:` edges from required to non-required lanes, and the guarantee
//! that no required lane's body references an experimental product
//! except through a deliberately declared cross-product reference.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LaneManifest {
    schema_version: u8,
    schema_id: String,
    controlling_issue: u32,
    lane: Vec<Lane>,
    declared_cross_product_reference: Vec<DeclaredRef>,
}

/// Lane crate sets are derived from the package topology's `ci_lane`
/// field (#3365), never maintained as a second truth table.
#[derive(Debug, Clone)]
struct CrateSets {
    cargo_allow_release_set: Vec<String>,
    intent_set: Vec<String>,
    proof_set: Vec<String>,
    shared_protocol_set: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Lane {
    id: String,
    product: String,
    posture: String,
    scope: String,
    note: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DeclaredRef {
    lane: String,
    referenced_product: String,
    token: String,
    reason: String,
}

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

/// Split the workflow into (job id, job body) pairs by top-level job keys.
/// Only lines after the `jobs:` key are considered, so trigger blocks like
/// `on: push:` are never misread as jobs.
fn ci_jobs(ci_yml: &str) -> Vec<(String, String)> {
    let mut jobs = Vec::new();
    let mut current: Option<(String, String)> = None;
    let mut pending = String::new();
    let mut in_jobs = false;
    let mut previous_blank = true;
    for line in ci_yml.lines() {
        if !in_jobs {
            if line.trim_end() == "jobs:" {
                in_jobs = true;
            }
            continue;
        }
        let rest = line.strip_prefix("  ");
        let is_job_header = match rest {
            Some(rest) => {
                !rest.starts_with(' ')
                    && rest.ends_with(':')
                    && rest.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                    && !rest.starts_with('-')
            }
            None => false,
        };
        // A two-space comment directly after a blank line introduces the
        // next job's documentation block; lines from there belong to that
        // job, not the one above it.
        let starts_between_jobs_comment = previous_blank && line.starts_with("  #");
        if is_job_header {
            if let Some((id, body)) = current.take() {
                jobs.push((id, body));
            }
            let id = rest.unwrap_or("").trim_end_matches(':').to_string();
            let body = std::mem::take(&mut pending);
            current = Some((id, body));
        } else if starts_between_jobs_comment && current.is_some() {
            if let Some((id, body)) = current.take() {
                jobs.push((id, body));
            }
            pending.push_str(line);
            pending.push('\n');
        } else if current.is_some() {
            if let Some((_, body)) = current.as_mut() {
                body.push_str(line);
                body.push('\n');
            }
        } else {
            pending.push_str(line);
            pending.push('\n');
        }
        previous_blank = line.trim().is_empty();
    }
    if let Some((id, body)) = current {
        jobs.push((id, body));
    }
    jobs
}

fn load_manifest(root: &Path) -> LaneManifest {
    let text = read_workspace_file(root, "docs/ci-lanes.toml");
    toml::from_str(&text)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse docs/ci-lanes.toml: {err}")))
}

fn lane_crate_sets(root: &Path) -> CrateSets {
    let topology_text = read_workspace_file(root, "policy/product-package-topology-v2.toml");
    let postures = intent_model::parse_package_postures_v1(&topology_text)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse package topology: {err}")));
    let by_lane = |lane: &str| {
        postures
            .iter()
            .filter(|row| row.ci_lane == lane)
            .map(|row| row.cargo_package_name.clone())
            .collect::<Vec<String>>()
    };
    CrateSets {
        cargo_allow_release_set: by_lane("test"),
        intent_set: by_lane("test-intent-experimental"),
        proof_set: by_lane("test-proof-experimental"),
        shared_protocol_set: by_lane("test-shared-protocol"),
    }
}

fn manifest_and_jobs() -> (CrateSets, LaneManifest, Vec<(String, String)>) {
    let root = workspace_root();
    let manifest = load_manifest(&root);
    let sets = lane_crate_sets(&root);
    let ci = read_workspace_file(&root, ".github/workflows/ci.yml");
    let jobs = ci_jobs(&ci);
    (sets, manifest, jobs)
}

#[test]
fn manifest_parses_and_covers_the_declared_lane_topology() {
    let (_, manifest, _) = manifest_and_jobs();
    assert_eq!(manifest.schema_id, "cargo-allow.ci-lanes.v1");
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.controlling_issue, 3358);
    for lane in &manifest.lane {
        if let Some(note) = lane.note.as_deref() {
            assert!(
                !note.trim().is_empty(),
                "lane {} carries an empty note",
                lane.id
            );
        }
    }
    let postures: BTreeSet<&str> = manifest.lane.iter().map(|l| l.posture.as_str()).collect();
    for expected in [
        "required",
        "experimental-informational",
        "integrated-informational",
    ] {
        assert!(
            postures.contains(expected),
            "lane manifest must classify at least one lane as {expected}"
        );
    }
    let ids: BTreeSet<&str> = manifest.lane.iter().map(|l| l.id.as_str()).collect();
    assert_eq!(ids.len(), manifest.lane.len(), "duplicate lane ids");

    // Declared cross-product references must name real lanes, recognized
    // products, and carry a non-empty justification — a declaration without
    // a reason is how silent coupling sneaks back in.
    for declared in &manifest.declared_cross_product_reference {
        assert!(
            ids.contains(declared.lane.as_str()),
            "declaration names unknown lane {}",
            declared.lane
        );
        assert!(
            manifest
                .lane
                .iter()
                .any(|l| l.product == declared.referenced_product),
            "declaration names unknown product {}",
            declared.referenced_product
        );
        assert!(
            !declared.reason.trim().is_empty(),
            "declaration for lane {} carries no reason",
            declared.lane
        );
    }
}

#[test]
fn every_manifest_lane_exists_as_a_ci_job() {
    let (_, manifest, jobs) = manifest_and_jobs();
    let job_ids: BTreeSet<&str> = jobs.iter().map(|(id, _)| id.as_str()).collect();
    for lane in &manifest.lane {
        assert!(
            job_ids.contains(lane.id.as_str()),
            "manifest lane {} is not a ci.yml job",
            lane.id
        );
    }
}

#[test]
fn crate_sets_partition_the_workspace_exactly() {
    let root = workspace_root();
    let (sets, _, _) = manifest_and_jobs();
    // Member paths are directory names; the lane sets carry cargo package
    // names. Two directories name their packages differently
    // (intent-engine -> intent-compiler, proof-engine -> proof-orchestrator),
    // so resolve each member's package name from its own manifest.
    let members: BTreeSet<String> = read_workspace_file(&root, "Cargo.toml")
        .lines()
        .filter_map(|l| l.trim().strip_prefix("\"crates/"))
        .map(|l| l.trim_end_matches("\",").to_string())
        .map(|dir| {
            let manifest = read_workspace_file(&root, &format!("crates/{dir}/Cargo.toml"));
            manifest
                .lines()
                .find_map(|line| line.trim().strip_prefix("name = \""))
                .map(|rest| rest.trim_end_matches('\"').to_string())
                .unwrap_or_else(|| {
                    std::panic::panic_any(format!("crates/{dir}/Cargo.toml has no name"))
                })
        })
        .collect();
    let mut union = BTreeSet::new();
    for set in [
        &sets.cargo_allow_release_set,
        &sets.intent_set,
        &sets.proof_set,
        &sets.shared_protocol_set,
    ] {
        for name in set {
            assert!(
                union.insert(name.clone()),
                "crate {name} listed in two lane scopes"
            );
        }
    }
    assert_eq!(
        union, members,
        "lane scope sets must partition the workspace membership exactly"
    );
}

#[test]
fn required_lanes_never_reference_experimental_products_without_declaration() {
    let (sets, manifest, jobs) = manifest_and_jobs();
    let required: BTreeSet<&str> = manifest
        .lane
        .iter()
        .filter(|l| l.posture == "required")
        .map(|l| l.id.as_str())
        .collect();
    let mut experimental_tokens: Vec<&str> = sets
        .intent_set
        .iter()
        .chain(&sets.proof_set)
        .chain(&sets.shared_protocol_set)
        .map(String::as_str)
        .collect();
    experimental_tokens.extend(["cargo-intent", "cargo-proof"]);
    let declared: BTreeSet<(&str, &str)> = manifest
        .declared_cross_product_reference
        .iter()
        .map(|d| (d.lane.as_str(), d.token.as_str()))
        .collect();

    for (id, body) in &jobs {
        if !required.contains(id.as_str()) {
            continue;
        }
        for token in &experimental_tokens {
            if body.contains(token) && !declared.contains(&(id.as_str(), *token)) {
                std::panic::panic_any(format!(
                    "required lane {id} references experimental product token '{token}' \
                     without a declared_cross_product_reference entry"
                ));
            }
        }
    }
}

#[test]
fn seeded_experimental_failure_cannot_block_required_lanes() {
    let (sets, manifest, jobs) = manifest_and_jobs();
    let required: BTreeSet<&str> = manifest
        .lane
        .iter()
        .filter(|l| l.posture == "required")
        .map(|l| l.id.as_str())
        .collect();
    let non_required: BTreeSet<&str> = manifest
        .lane
        .iter()
        .filter(|l| l.posture != "required")
        .map(|l| l.id.as_str())
        .collect();

    for (id, body) in &jobs {
        if !required.contains(id.as_str()) {
            continue;
        }
        // No required lane may depend on a non-required lane.
        for dep in non_required.iter() {
            assert!(
                !body.contains(&format!("needs: {dep}"))
                    && !body.contains(&format!("needs: [{dep}]")),
                "required lane {id} declares needs on non-required lane {dep}"
            );
        }
        // No required lane with release-set scope may build the whole
        // workspace; experimental products prove themselves elsewhere.
        let lane = manifest
            .lane
            .iter()
            .find(|l| l.id == *id)
            .unwrap_or_else(|| std::panic::panic_any(format!("lane {id} missing")));
        if lane.scope == "cargo-allow-release-set" {
            assert!(
                !body.contains(" --workspace"),
                "required lane {id} still builds the whole workspace"
            );
            // Every `-p <crate>` on a cargo invocation line must belong to
            // the release set (mkdir -p and friends are not cargo scoping).
            for line in body.lines() {
                let words: Vec<&str> = line.split_whitespace().collect();
                if words.first() != Some(&"cargo") {
                    continue;
                }
                for (index, word) in words.iter().enumerate() {
                    if *word == "-p" {
                        let target = words
                            .get(index + 1)
                            .unwrap_or_else(|| {
                                std::panic::panic_any(format!("lane {id}: dangling -p flag"))
                            })
                            .trim_end_matches('\\');
                        let in_release_set =
                            sets.cargo_allow_release_set.iter().any(|c| c == target);
                        let declared = manifest
                            .declared_cross_product_reference
                            .iter()
                            .any(|d| d.lane == *id && d.token == *target);
                        assert!(
                            in_release_set || declared,
                            "required lane {id} scopes cargo onto non-release-set crate {target} without a declared_cross_product_reference entry"
                        );
                    }
                }
            }
        }
    }
}

/// The four cargo-scoped required lanes must each carry the complete
/// release set, so narrowing can never silently drop a crate's coverage.
#[test]
fn cargo_scoped_required_lanes_carry_the_complete_release_set() {
    let (sets, _, jobs) = manifest_and_jobs();
    for lane_id in ["msrv", "test", "test-windows", "coverage"] {
        let body = jobs
            .iter()
            .find(|(id, _)| id == lane_id)
            .map(|(_, body)| body.as_str())
            .unwrap_or_else(|| std::panic::panic_any(format!("job {lane_id} missing")));
        for name in &sets.cargo_allow_release_set {
            assert!(
                body.contains(&format!("-p {name}")),
                "required lane {lane_id} omits release-set crate {name}"
            );
        }
    }
}

#[test]
fn candidate_smokes_and_dogfood_live_in_product_scoped_lanes() {
    let (_, _, jobs) = manifest_and_jobs();
    let body = |id: &str| {
        jobs.iter()
            .find(|(job, _)| job == id)
            .map(|(_, body)| body.as_str())
            .unwrap_or_else(|| std::panic::panic_any(format!("job {id} missing")))
    };
    let package_smoke = body("package-smoke");
    for script in [
        "intent-candidate-smoke.sh",
        "proof-candidate-smoke.sh",
        "exact-candidate-interop-smoke.sh",
    ] {
        assert!(
            !package_smoke.contains(script),
            "package-smoke still runs {script}; it belongs to product-candidates-interop"
        );
    }
    let interop = body("product-candidates-interop");
    for script in [
        "intent-candidate-smoke.sh",
        "proof-candidate-smoke.sh",
        "exact-candidate-interop-smoke.sh",
    ] {
        assert!(
            interop.contains(script),
            "product-candidates-interop must run {script}"
        );
    }
    let test_lane = body("test");
    for marker in [
        "three-product-dogfood",
        "governance",
        "simplification-audit",
    ] {
        assert!(
            !test_lane.contains(marker),
            "test lane still carries integrated step '{marker}'"
        );
    }
    let dogfood = body("integrated-dogfood");
    for marker in [
        "three-product-dogfood",
        "governance",
        "simplification-audit",
    ] {
        assert!(
            dogfood.contains(marker),
            "integrated-dogfood must carry '{marker}'"
        );
    }
}
