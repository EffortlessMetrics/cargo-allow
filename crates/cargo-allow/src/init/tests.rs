use super::*;
use crate::{CargoAllowCli, CargoAllowCommand};
use clap::Parser;
use std::path::Path;

#[test]
fn clap_parses_init_config_and_force() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "init",
        "--strict",
        "--force",
        "--config",
        "target/allow.toml",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse init: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Init(InitArgs {
            strict: true,
            force: true,
            config,
        })) if config == Path::new("target/allow.toml")
    ));
}

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}
