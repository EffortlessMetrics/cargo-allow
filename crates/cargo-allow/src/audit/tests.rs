use super::*;
use crate::{CargoAllowCli, CargoAllowCommand};
use clap::Parser;

#[test]
fn clap_parses_include_untracked_audit_flag() {
    let parsed =
        CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "audit", "--include-untracked"]))
            .unwrap_or_else(|err| {
                std::panic::panic_any(format!("CLI should parse include-untracked: {err}"))
            });

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Audit(ReportArgs {
            include_untracked: true,
            ..
        }))
    ));
}

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}
