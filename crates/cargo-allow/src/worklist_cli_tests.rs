use super::*;
use crate::{CargoAllowCli, CargoAllowCommand, HumanJsonFormat, ProfileArg};
use clap::Parser;
use std::path::Path;

fn argv(items: Vec<&str>) -> Vec<String> {
    items.into_iter().map(String::from).collect()
}

#[test]
fn clap_parses_worklist_location_drift_status() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "worklist",
        "--status",
        "location_drift",
    ]))
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("CLI should accept location_drift status: {err}"))
    });

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Worklist(WorklistArgs {
            status: Some(status),
            ..
        })) if status == "location_drift"
    ));
}

#[test]
fn clap_parses_worklist_json_output() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "worklist",
        "--kind",
        "unsafe",
        "--family",
        "unsafe_fn",
        "--item-kind",
        "baseline_debt",
        "--status",
        "baseline_debt",
        "--allow-id",
        "allow-0001",
        "--path",
        "crates/allow-core",
        "--source-package",
        "allow-core",
        "--owner",
        "runtime",
        "--classification",
        "baseline_debt",
        "--baseline-debt",
        "--broad-scope",
        "--risk",
        "medium",
        "--difficulty",
        "small",
        "--missing-evidence",
        "--broken-evidence",
        "--weak-evidence",
        "--format",
        "json",
        "--output",
        "target/worklist.json",
    ]))
    .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse worklist args: {err}")));

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Worklist(WorklistArgs {
            kind: Some(kind),
            family: Some(family),
            item_kind: Some(item_kind),
            status: Some(status),
            allow_id: Some(allow_id),
            path: Some(path_filter),
            source_package: Some(source_package),
            owner: Some(owner),
            classification: Some(classification),
            baseline_debt: true,
            broad_scope: true,
            risk: Some(risk),
            difficulty: Some(difficulty),
            missing_evidence: true,
            broken_evidence: true,
            weak_evidence: true,
            format: HumanJsonFormat::Json,
            output: Some(path),
            ..
        })) if kind == "unsafe"
            && family == "unsafe_fn"
            && item_kind == "baseline_debt"
            && status == "baseline_debt"
            && allow_id == "allow-0001"
            && path_filter == "crates/allow-core"
            && source_package == "allow-core"
            && owner == "runtime"
            && classification == "baseline_debt"
            && risk == "medium"
            && difficulty == "small"
            && path == Path::new("target/worklist.json")
    ));
}

#[test]
fn clap_rejects_unknown_worklist_item_kind() {
    let err = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "worklist",
        "--item-kind",
        "stale_allow_typo",
    ]))
    .expect_err("unknown worklist item-kind should fail closed");

    assert!(
        err.to_string().contains("unknown work item kind"),
        "unexpected parse error: {err}"
    );
}

#[test]
fn clap_rejects_unknown_worklist_kind() {
    let err =
        CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "worklist", "--kind", "unsfae"]))
            .expect_err("unknown worklist kind should fail closed");

    assert!(
        err.to_string()
            .contains("unknown kind `unsfae`; supported kinds:"),
        "unexpected parse error: {err}"
    );
}

#[test]
fn clap_parses_spec_system_profile_for_worklist() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "worklist",
        "--profile",
        "spec-system",
        "--format",
        "json",
    ]))
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("CLI should parse spec-system worklist args: {err}"))
    });

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Worklist(WorklistArgs {
            profile: Some(ProfileArg::SpecSystem),
            format: HumanJsonFormat::Json,
            ..
        }))
    ));
}

#[test]
fn clap_accepts_hyphenated_worklist_item_kind_alias() {
    let parsed = CargoAllowCli::try_parse_from(argv(vec![
        "cargo-allow",
        "worklist",
        "--item-kind",
        "stale-allow",
    ]))
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("hyphenated item-kind alias should parse: {err}"))
    });

    assert!(matches!(
        parsed.command,
        Some(CargoAllowCommand::Worklist(WorklistArgs {
            item_kind: Some(item_kind),
            ..
        })) if item_kind == "stale_allow"
    ));
}

#[test]
fn clap_parses_migration_closeout_worklist_presets() {
    let cases = [
        (None, "broken_evidence_link"),
        (None, "weak_evidence_reference"),
        (None, "baseline_debt"),
        (Some("unsafe"), "broken_evidence_link"),
        (Some("unsafe"), "weak_evidence_reference"),
        (Some("unsafe"), "baseline_debt"),
    ];

    for (kind, item_kind) in cases {
        let mut args = vec!["cargo-allow", "worklist"];
        if let Some(kind) = kind {
            args.extend(["--kind", kind]);
        }
        args.extend(["--item-kind", item_kind, "--format", "json"]);

        let parsed = CargoAllowCli::try_parse_from(argv(args)).unwrap_or_else(|err| {
            std::panic::panic_any(format!(
                "migration closeout preset {kind:?}/{item_kind} should parse: {err}"
            ))
        });

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Worklist(WorklistArgs {
                kind: parsed_kind,
                item_kind: Some(parsed_item_kind),
                format: HumanJsonFormat::Json,
                ..
            })) if parsed_kind.as_deref() == kind && parsed_item_kind == item_kind
        ));
    }
}
