use allow_core::{CargoAllowError, CargoAllowResult};
use std::path::Path;

pub fn workspace_members_from_manifest(root: &Path) -> CargoAllowResult<Vec<String>> {
    let cargo_toml = root.join("Cargo.toml");
    let text = std::fs::read_to_string(&cargo_toml).map_err(|err| {
        CargoAllowError::new(format!(
            "workspace Cargo.toml unreadable at {}: {err}",
            cargo_toml.display()
        ))
    })?;
    let parsed: toml::Value = toml::from_str(&text)
        .map_err(|err| CargoAllowError::new(format!("workspace Cargo.toml parse error: {err}")))?;
    let members = parsed
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(|members| members.as_array())
        .ok_or_else(|| CargoAllowError::new("workspace members missing from Cargo.toml"))?;
    let mut paths = Vec::with_capacity(members.len());
    for member in members {
        let Some(path) = member.as_str() else {
            return Err(CargoAllowError::new(
                "workspace member entry was not a string",
            ));
        };
        paths.push(path.to_string());
    }
    Ok(paths)
}
