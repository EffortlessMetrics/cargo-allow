use super::extraction_shim_registry_fails_check;
use allow_match::CheckMode;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn repository_extraction_shim_registry_passes_no_new_guard() -> Result<(), String> {
    let root = repo_root();
    if extraction_shim_registry_fails_check(&root, CheckMode::NoNew)
        .map_err(|error| format!("evaluate extraction shim guard: {error}"))?
    {
        return Err("repository extraction shim registry should pass no-new guard".to_string());
    }
    Ok(())
}

#[test]
fn extraction_shim_registry_is_advisory_in_audit_mode() -> Result<(), String> {
    let root = repo_root();
    if extraction_shim_registry_fails_check(&root, CheckMode::Audit)
        .map_err(|error| format!("evaluate extraction shim guard: {error}"))?
    {
        return Err("audit mode should not enforce extraction shim registry".to_string());
    }
    Ok(())
}

#[test]
fn duplicate_extraction_shim_fails_no_new_guard() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-extraction-shim-no-new-{}",
        std::process::id()
    ));
    if root.exists() {
        std::fs::remove_dir_all(&root).map_err(|error| format!("clean temp root: {error}"))?;
    }
    std::fs::create_dir_all(root.join("policy"))
        .map_err(|error| format!("create temp policy root: {error}"))?;

    let registry_path = repo_root().join("policy/extraction-shims.toml");
    let registry = std::fs::read_to_string(&registry_path)
        .map_err(|error| format!("read repository shim registry: {error}"))?;
    let first_shim = registry
        .split("\n[[shim]]")
        .nth(1)
        .and_then(|block| block.split("\n[[shim]]").next())
        .ok_or_else(|| "repository shim registry has no shim block".to_string())?;
    let duplicate_registry = format!("{registry}\n[[shim]]{first_shim}\n");
    std::fs::write(
        root.join("policy/extraction-shims.toml"),
        duplicate_registry,
    )
    .map_err(|error| format!("write duplicate shim registry: {error}"))?;
    std::fs::copy(
        repo_root().join("policy/product-move-ledger.toml"),
        root.join("policy/product-move-ledger.toml"),
    )
    .map_err(|error| format!("copy move ledger: {error}"))?;

    if !extraction_shim_registry_fails_check(&root, CheckMode::NoNew)
        .map_err(|error| format!("evaluate duplicate shim guard: {error}"))?
    {
        return Err("duplicate extraction shim should fail no-new guard".to_string());
    }
    if extraction_shim_registry_fails_check(&root, CheckMode::Audit)
        .map_err(|error| format!("evaluate duplicate shim audit guard: {error}"))?
    {
        return Err("audit mode should not enforce duplicate extraction shim".to_string());
    }

    std::fs::remove_dir_all(&root).map_err(|error| format!("remove temp root: {error}"))?;
    Ok(())
}
