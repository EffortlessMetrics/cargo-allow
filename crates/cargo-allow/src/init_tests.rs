use super::*;
use crate::{CargoAllowCli, CargoAllowCommand, RootArgs};
use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn clap_parses_init_root_config_and_force() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "init",
        "--root",
        "fixtures/source-tree",
        "--strict",
        "--force",
        "--config",
        "target/allow.toml",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse init: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Init(InitArgs {
            root: RootArgs { root: Some(root) },
            strict: true,
            force: true,
            config,
        })) if root == Path::new("fixtures/source-tree")
            && config == Path::new("target/allow.toml")
    ));
}

#[test]
fn cmd_init_writes_relative_config_under_explicit_root() {
    let root = init_fixture_dir();
    let policy = root.join("policy/allow.toml");

    cmd_init(&InitArgs {
        root: RootArgs {
            root: Some(root.clone()),
        },
        strict: false,
        force: false,
        config: PathBuf::from("policy/allow.toml"),
    })
    .unwrap_or_else(|err| std::panic::panic_any(format!("init should write policy: {err}")));

    assert!(
        policy.exists(),
        "init should resolve relative config paths under the source-tree root"
    );

    remove_init_fixture_dir(root);
}

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}

fn init_fixture_dir() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("cargo-allow-init-{}-{stamp}", std::process::id()));
    remove_init_fixture_dir(dir.clone());
    fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create init fixture: {err}")));
    dir
}

fn remove_init_fixture_dir(path: PathBuf) {
    match fs::remove_dir_all(&path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => std::panic::panic_any(format!("remove init fixture {}: {err}", path.display())),
    }
}
