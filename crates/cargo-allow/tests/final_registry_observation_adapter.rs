use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> Result<PathBuf, Box<dyn Error>> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?)
}

#[test]
fn final_registry_preflight_provider_python_adapter_contract() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let output = Command::new("python")
        .arg(root.join("scripts/test-final-registry-observation.py"))
        .current_dir(&root)
        .output()?;

    if !output.status.success() {
        return Err(io::Error::other(format!(
            "registry observation adapter contract failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        ))
        .into());
    }

    Ok(())
}
