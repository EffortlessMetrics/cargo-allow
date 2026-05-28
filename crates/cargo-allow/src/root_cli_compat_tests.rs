use crate::*;
use clap::Parser;

#[test]
fn clap_parses_lint_exception_compat_check() {
    assert_parses_compat_kind("lint-exception", "lint exception");
}

#[test]
fn clap_parses_no_panic_allowlist_compat_check() {
    assert_parses_compat_kind("no-panic-allowlist", "no-panic allowlist");
}

#[test]
fn clap_parses_unsafe_compat_check() {
    assert_parses_compat_kind("unsafe", "unsafe");
}

#[test]
fn clap_parses_non_rust_compat_check() {
    assert_parses_compat_kind("non-rust", "non-rust");
}

#[test]
fn clap_parses_generated_compat_check() {
    assert_parses_compat_kind("generated", "generated");
}

#[test]
fn clap_parses_panic_compat_check() {
    assert_parses_compat_kind("panic", "panic");
}

#[test]
fn clap_parses_executable_compat_check() {
    assert_parses_compat_kind("executable", "executable");
}

#[test]
fn clap_parses_workflow_compat_check() {
    assert_parses_compat_kind("workflow", "workflow");
}

#[test]
fn clap_parses_dependency_surface_compat_check() {
    assert_parses_compat_kind("dependency-surface", "dependency-surface");
}

#[test]
fn clap_parses_process_compat_check() {
    assert_parses_compat_kind("process", "process");
}

#[test]
fn clap_parses_network_compat_check() {
    assert_parses_compat_kind("network", "network");
}

fn assert_parses_compat_kind(kind: &str, label: &str) {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "check",
        "--compat",
        "--kind",
        kind,
    ]))
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("CLI should parse {label} compat check: {err}"))
    });

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Check(check::CheckArgs {
            compat: true,
            kind: Some(parsed_kind),
            ..
        })) if parsed_kind == kind
    ));
}

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}
