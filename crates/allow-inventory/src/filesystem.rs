use allow_core::{CargoAllowError, CargoAllowResult};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn existing_regular_files(root: &Path, files: Vec<PathBuf>) -> Vec<PathBuf> {
    files
        .into_iter()
        .filter(|path| {
            // Use fs::metadata (follows symlinks) so a symlink pointing at a
            // regular file is included. Previously symlink_metadata was used,
            // which reports the link itself — is_file() is always false for a
            // symlink, silently dropping symlinked source files (#1842).
            fs::metadata(root.join(path))
                .map(|metadata| metadata.file_type().is_file())
                .unwrap_or(false)
        })
        .collect()
}

pub(crate) fn recursive_files(root: &Path) -> CargoAllowResult<Vec<PathBuf>> {
    let mut out = Vec::new();
    visit(root, root, &mut out)?;
    Ok(out)
}

fn visit(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> CargoAllowResult<()> {
    for entry in fs::read_dir(dir)
        .map_err(|e| CargoAllowError::new(format!("failed to read {}: {e}", dir.display())))?
    {
        let entry = entry
            .map_err(|e| CargoAllowError::new(format!("failed to read directory entry: {e}")))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|e| {
            CargoAllowError::new(format!("failed to inspect {}: {e}", path.display()))
        })?;
        if file_type.is_symlink() {
            // Resolve the symlink target. Include if it points to a regular
            // file (#1842). Do NOT recurse into symlinked directories (loop
            // safety).
            if fs::metadata(&path)
                .map(|m| m.file_type().is_file())
                .unwrap_or(false)
            {
                let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                out.push(rel);
            }
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == ".git" || name == "target" {
            continue;
        }
        if file_type.is_dir() {
            visit(root, &path, out)?;
        } else if file_type.is_file() {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            out.push(rel);
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn visit_for_test(
    root: &Path,
    dir: &Path,
    out: &mut Vec<PathBuf>,
) -> CargoAllowResult<()> {
    visit(root, dir, out)
}
