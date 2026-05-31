use std::{
    fs,
    ops::Deref,
    path::{Path, PathBuf},
};

mod diagnostics;
mod local_rejections;
mod local_validation;

struct TestRoot {
    path: PathBuf,
}

impl AsRef<Path> for TestRoot {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl Deref for TestRoot {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        remove_test_path(&self.path);
    }
}

fn unique_test_dir(slug: &str) -> TestRoot {
    let mut path = std::env::temp_dir();
    path.push(format!("cargo-allow-policy-{slug}-{}", std::process::id()));
    remove_test_path(&path);
    TestRoot { path }
}

fn remove_test_dir(root: TestRoot) {
    drop(root);
}

fn remove_test_path(path: &Path) {
    match fs::remove_dir_all(path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => std::panic::panic_any(format!(
            "failed to remove test dir {}: {err}",
            path.display()
        )),
    }
}

fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
