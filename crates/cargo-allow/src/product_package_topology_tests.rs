use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn current_v2_authorities_drive_package_candidate() -> Result<(), String> {
    // Re-based onto intent-model compat parsing (#3562): the receipt chain
    // is gone; the denominators and candidate rows derive from the same
    // authorities through the compat surface, with the closure validation
    // proven separately by the intent-engine live-authority tests.
    let root = repo_root();
    let read = |rel: &str| -> Result<String, String> {
        std::fs::read_to_string(root.join(rel)).map_err(|err| format!("read {rel}: {err}"))
    };

    let identities =
        intent_model::parse_crate_identities_v1(&read("policy/product-crates-v2.toml")?)?;
    let postures =
        intent_model::parse_package_postures_v1(&read("policy/product-package-topology-v2.toml")?)?;
    let workspace_members: Vec<String> = {
        let manifest: toml::Table = toml::from_str(&read("Cargo.toml")?)
            .map_err(|err| format!("parse workspace manifest: {err}"))?;
        manifest
            .get("workspace")
            .and_then(|w| w.get("members"))
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .ok_or_else(|| "workspace manifest missing members".to_string())?
    };

    if identities.len() != 22 || postures.len() != 22 || workspace_members.len() != 22 {
        return Err(format!(
            "expected three complete 22-package denominators, got workspace={} identities={} topology={}",
            workspace_members.len(),
            identities.len(),
            postures.len()
        ));
    }

    // The cargo-allow candidate set spans the cargo-allow-0.2 family rows
    // plus the three shared-0.1 substrate rows they depend on.
    let candidate: Vec<_> = postures
        .iter()
        .filter(|p| {
            p.membership.candidate_inclusion
                && (p.version_line == "cargo-allow-0.2" || p.version_line == "shared-0.1")
        })
        .map(|p| p.cargo_package_name.as_str())
        .collect();
    if candidate.len() != 13 {
        return Err(format!(
            "expected 13 cargo-allow-0.2 candidate rows, got {}",
            candidate.len()
        ));
    }
    for name in [
        "allow-core",
        "allow-policy",
        "allow-inventory",
        "allow-files",
        "allow-rust",
        "allow-match",
        "allow-report",
        "allow-policy-legacy",
        "allow-diff",
        "effortless-repo-protocol",
        "effortless-repo-snapshot",
        "effortless-repo-edit",
        "cargo-allow",
    ] {
        if !candidate.contains(&name) {
            return Err(format!("candidate set omitted `{name}`"));
        }
    }
    Ok(())
}

#[test]
fn candidate_package_names_exclude_engine_library_identities() -> Result<(), String> {
    // The cargo-allow candidate set never packages the engine crates:
    // intent-compiler and proof-orchestrator are library identities for
    // the engine family, not candidate rows (#3562 re-base).
    let root = repo_root();
    let postures = intent_model::parse_package_postures_v1(
        &std::fs::read_to_string(root.join("policy/product-package-topology-v2.toml"))
            .map_err(|err| format!("read topology authority: {err}"))?,
    )?;
    let candidate: Vec<_> = postures
        .iter()
        .filter(|p| p.membership.candidate_inclusion)
        .map(|p| p.cargo_package_name.as_str())
        .collect();
    for engine in ["intent-compiler", "proof-orchestrator"] {
        if candidate.contains(&engine) {
            return Err(format!(
                "candidate set includes engine library identity `{engine}`"
            ));
        }
    }
    Ok(())
}
