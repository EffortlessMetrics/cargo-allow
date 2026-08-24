use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[test]
fn topology_v2_retains_expected_checksums_for_shared_prerequisites() -> Result<(), Box<dyn Error>> {
    let root = repository_root()?;
    if !root.join(".git").exists() {
        return Ok(());
    }

    let topology_content =
        fs::read_to_string(root.join("policy/product-package-topology-v2.toml"))?;
    let topology: toml::Value = toml::from_str(&topology_content)?;
    let topology_json = serde_json::to_value(&topology)?;

    let rows = topology_json
        .pointer("/package")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| io::Error::other("missing package table in topology v2"))?;

    let mut cargo_allow_count = 0;
    let mut shared_count = 0;

    for row in rows {
        let is_candidate = row
            .get("candidate_inclusion")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !is_candidate {
            continue;
        }

        let family = row
            .get("product_family")
            .and_then(serde_json::Value::as_str);
        match family {
            Some("cargo-allow") => cargo_allow_count += 1,
            Some("shared") => {
                shared_count += 1;
                let expected_checksum = row
                    .get("expected_registry_checksum")
                    .and_then(serde_json::Value::as_str);
                assert!(
                    expected_checksum.is_some(),
                    "shared prerequisite row {} must have expected_registry_checksum",
                    row.get("cargo_package_name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("unknown")
                );
                let cs = expected_checksum.unwrap();
                assert!(cs.starts_with("sha256:") && cs.len() == 71);
            }
            _ => {}
        }
    }

    assert_eq!(
        cargo_allow_count, 10,
        "must have exactly 10 cargo-allow candidate rows"
    );
    assert_eq!(
        shared_count, 3,
        "must have exactly 3 shared prerequisite rows"
    );

    Ok(())
}

fn repository_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = manifest_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow manifest has no crates parent"))?;
    let root = crates_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow crates directory has no repository parent"))?;
    Ok(root.to_path_buf())
}
