use allow_core::{CargoAllowError, CargoAllowResult};
use allow_inventory::resolve_source_tree_root;
use allow_policy::starter_policy;
use std::env;
use std::fs;
use std::path::Path;

#[path = "init_args.rs"]
mod init_args;
pub(crate) use init_args::InitArgs;

use crate::root_relative_path;

pub(crate) fn cmd_init(args: &InitArgs) -> CargoAllowResult<()> {
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    let root = resolve_source_tree_root(args.root.root.as_deref(), cwd)?;
    let path = root_relative_path(&root, &args.config);
    if path.exists() && !args.force {
        return Err(CargoAllowError::new(format!(
            "{} already exists; use --force to overwrite",
            path.display()
        )));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            CargoAllowError::new(format!("failed to create {}: {e}", parent.display()))
        })?;
    }
    fs::write(&path, starter_policy(args.strict))
        .map_err(|e| CargoAllowError::new(format!("failed to write {}: {e}", path.display())))?;
    println!("created {}", created_path_display(&root, &path));
    Ok(())
}

fn created_path_display(root: &Path, path: &Path) -> String {
    let display_path = path
        .strip_prefix(root)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| path.to_path_buf());
    allow_report::source_tree_path_text(&display_path)
}

#[cfg(test)]
#[path = "init_tests.rs"]
mod tests;
