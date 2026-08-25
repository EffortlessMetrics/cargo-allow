use std::error::Error;
use std::io;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ReleaseSensitivity {
    ReleaseSensitive,
    Unrelated,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ImpactClassificationReceiptV1 {
    pub schema_version: String,
    pub classification: ReleaseSensitivity,
    pub matched_release_sensitive_paths: Vec<String>,
    pub rationale: String,
}

pub fn classify_pr_impact(changed_paths: &[&str]) -> ImpactClassificationReceiptV1 {
    let mut sensitive_matches = Vec::new();

    for path in changed_paths {
        if path.starts_with(".github/workflows/release")
            || path.starts_with("policy/product-package-topology")
            || path.starts_with("scripts/")
                && (path.contains("release")
                    || path.contains("packaged")
                    || path.contains("upgrade-rollback"))
            || path.starts_with(".changes/")
            || *path == "Cargo.toml"
            || *path == "Cargo.lock"
            || path.ends_with("/Cargo.toml")
            || *path == "docs/support-matrix.toml"
            || *path == "SUPPORT.md"
            || path.contains("release_authorization")
            || path.contains("release_rehearsal")
        {
            sensitive_matches.push(path.to_string());
        }
    }

    if !sensitive_matches.is_empty() {
        ImpactClassificationReceiptV1 {
            schema_version: "1.0".to_string(),
            classification: ReleaseSensitivity::ReleaseSensitive,
            matched_release_sensitive_paths: sensitive_matches,
            rationale:
                "PR changes load-bearing release surfaces, requiring full zero-upload rehearsal."
                    .to_string(),
        }
    } else {
        ImpactClassificationReceiptV1 {
            schema_version: "1.0".to_string(),
            classification: ReleaseSensitivity::Unrelated,
            matched_release_sensitive_paths: Vec::new(),
            rationale: "No release-sensitive paths detected; fast CI path eligible.".to_string(),
        }
    }
}

fn require(cond: bool, msg: &str) -> Result<(), io::Error> {
    if !cond {
        Err(io::Error::other(msg))
    } else {
        Ok(())
    }
}

#[test]
fn test_impact_classification_sensitivity() -> Result<(), Box<dyn Error>> {
    // 1. Cargo.lock change is release sensitive
    let r1 = classify_pr_impact(&["Cargo.lock"]);
    require(
        r1.classification == ReleaseSensitivity::ReleaseSensitive,
        "Cargo.lock must be release sensitive",
    )?;

    // 2. Topology file change is release sensitive
    let r2 = classify_pr_impact(&["policy/product-package-topology-v2.toml"]);
    require(
        r2.classification == ReleaseSensitivity::ReleaseSensitive,
        "product-package-topology must be release sensitive",
    )?;

    // 3. Release workflow change is release sensitive
    let r3 = classify_pr_impact(&[".github/workflows/release.yml"]);
    require(
        r3.classification == ReleaseSensitivity::ReleaseSensitive,
        "release.yml must be release sensitive",
    )?;

    // 4. Release script is release sensitive
    let r4 = classify_pr_impact(&["scripts/release-rehearsal.py"]);
    require(
        r4.classification == ReleaseSensitivity::ReleaseSensitive,
        "release-rehearsal.py must be release sensitive",
    )?;

    // 5. Unrelated documentation edit is not release sensitive
    let r5 = classify_pr_impact(&["docs/README.md"]);
    require(
        r5.classification == ReleaseSensitivity::Unrelated,
        "docs/README.md should be unrelated",
    )?;

    Ok(())
}

#[test]
fn test_impact_classifier_negative_controls() -> Result<(), Box<dyn Error>> {
    // Mixed set with one release-sensitive file must select ReleaseSensitive
    let r = classify_pr_impact(&["docs/README.md", "Cargo.toml"]);
    require(
        r.classification == ReleaseSensitivity::ReleaseSensitive,
        "mixed set with Cargo.toml must select ReleaseSensitive",
    )?;

    // Changie note change is release sensitive
    let r_changie = classify_pr_impact(&[".changes/Added-20260825-test.yaml"]);
    require(
        r_changie.classification == ReleaseSensitivity::ReleaseSensitive,
        "Changie note must select ReleaseSensitive",
    )?;

    Ok(())
}
