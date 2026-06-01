use allow_core::FindingKind;

use super::test_support::{test_entry, test_finding};
use super::{WORK_ITEM_KINDS, proof_commands};

const FORBIDDEN_PROOF_COMMAND_TOOL_TOKENS: &[&str] = &[
    "cargo",
    "rustc",
    "clippy",
    "clippy-driver",
    "cargo-clippy",
    "cargo-deny",
    "cargo-vet",
    "cargo-geiger",
    "ripr",
    "unsafe-review",
    "cargo-llvm-cov",
    "llvm-cov",
    "grcov",
    "tarpaulin",
    "cargo-tarpaulin",
];

#[test]
fn proof_commands_cover_known_worklist_kinds() {
    for kind in WORK_ITEM_KINDS {
        let commands = proof_commands(kind, None, None);
        let item_kind_command = format!("cargo-allow worklist --item-kind {kind} --format json");

        assert!(
            commands.iter().any(|command| command == &item_kind_command),
            "{kind} proof commands should include the item-kind worklist shortcut"
        );
        assert!(
            commands
                .iter()
                .all(|command| command.starts_with("cargo-allow ")),
            "{kind} proof commands should stay cargo-allow first"
        );
    }
}

#[test]
fn proof_commands_do_not_route_agents_to_external_tools() {
    let finding_cases = [
        test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "call_expr",
        ),
        test_finding(
            FindingKind::Unsafe,
            Some("unsafe_block"),
            "src/lib.rs",
            "unsafe_block",
        ),
        test_finding(
            FindingKind::LintException,
            Some("clippy"),
            "src/lib.rs",
            "attribute_item",
        ),
        test_finding(
            FindingKind::NonRustFile,
            Some("script"),
            "scripts/release.sh",
            "tracked_file",
        ),
        test_finding(
            FindingKind::GeneratedCode,
            Some("generated"),
            "src/generated.rs",
            "generated_file",
        ),
    ];
    let entry_cases = [
        test_entry("allow-panic", FindingKind::Panic),
        test_entry("allow-unsafe", FindingKind::Unsafe),
        test_entry("allow-lint", FindingKind::LintException),
        test_entry("allow-non-rust", FindingKind::NonRustFile),
        test_entry("allow-generated", FindingKind::GeneratedCode),
        {
            let mut entry = test_entry("allow-workflow", FindingKind::PolicyException);
            entry.family = Some("workflow_external_action".to_string());
            entry
        },
        {
            let mut entry = test_entry("allow-policy", FindingKind::PolicyException);
            entry.family = Some("unknown_policy_family".to_string());
            entry
        },
    ];

    for kind in WORK_ITEM_KINDS {
        assert_source_tree_proof_command_boundary(kind, &proof_commands(kind, None, None));

        for finding in &finding_cases {
            assert_source_tree_proof_command_boundary(
                kind,
                &proof_commands(kind, Some(finding), None),
            );
        }

        for entry in &entry_cases {
            assert_source_tree_proof_command_boundary(
                kind,
                &proof_commands(kind, None, Some(entry)),
            );
        }
    }
}

#[test]
fn proof_commands_use_finding_kind_when_present() {
    let finding = test_finding(
        FindingKind::LintException,
        Some("clippy"),
        "src/lib.rs",
        "attribute_item",
    );
    let mut entry = test_entry("allow-unsafe", FindingKind::Unsafe);
    entry.family = Some("unsafe_fn".to_string());

    assert_eq!(
        proof_commands("new_unreceipted_finding", Some(&finding), Some(&entry)),
        vec![
            "cargo-allow explain allow-unsafe",
            "cargo-allow list --allow-id allow-unsafe --format json",
            "cargo-allow worklist --allow-id allow-unsafe --format json",
            "cargo-allow check --kind lint-exception --mode no-new",
            "cargo-allow worklist --item-kind new_unreceipted_finding --format json",
            "cargo-allow worklist --kind lint-exception --format json",
        ]
    );
}

fn assert_source_tree_proof_command_boundary(kind: &str, commands: &[String]) {
    for command in commands {
        assert!(
            command.starts_with("cargo-allow "),
            "{kind} proof command should stay within cargo-allow: {command}"
        );

        for token in command.split_ascii_whitespace() {
            assert!(
                !FORBIDDEN_PROOF_COMMAND_TOOL_TOKENS
                    .iter()
                    .any(|forbidden| forbidden_tool_token_matches(token, forbidden)),
                "{kind} proof command should not invoke adjacent build/evidence tooling: {command}"
            );
        }
    }
}

fn forbidden_tool_token_matches(token: &str, forbidden: &str) -> bool {
    token == forbidden || token.strip_suffix(".exe") == Some(forbidden)
}

#[test]
fn proof_commands_map_entry_only_kinds_and_worklist_shortcuts() {
    let mut entry = test_entry("allow-workflow", FindingKind::PolicyException);
    entry.family = Some("workflow_external_action".to_string());

    assert_eq!(
        proof_commands("broad_scope", None, Some(&entry)),
        vec![
            "cargo-allow explain allow-workflow",
            "cargo-allow list --allow-id allow-workflow --format json",
            "cargo-allow worklist --allow-id allow-workflow --format json",
            "cargo-allow worklist --broad-scope --format json",
            "cargo-allow check --kind workflow --mode no-new",
            "cargo-allow worklist --item-kind broad_scope --format json",
            "cargo-allow worklist --kind workflow --format json",
        ]
    );

    entry.id = "allow-dependency".to_string();
    entry.family = Some("dependency_surface".to_string());
    assert_eq!(
        proof_commands("baseline_debt", None, Some(&entry)),
        vec![
            "cargo-allow explain allow-dependency",
            "cargo-allow list --allow-id allow-dependency --format json",
            "cargo-allow worklist --allow-id allow-dependency --format json",
            "cargo-allow worklist --baseline-debt --format json",
            "cargo-allow check --kind dependency-surface --mode no-new",
            "cargo-allow worklist --item-kind baseline_debt --format json",
            "cargo-allow worklist --kind dependency-surface --format json",
        ]
    );

    entry.id = "allow-non-rust".to_string();
    entry.kind = FindingKind::NonRustFile;
    entry.family = Some("script".to_string());
    assert_eq!(
        proof_commands("missing_evidence", None, Some(&entry)),
        vec![
            "cargo-allow explain allow-non-rust",
            "cargo-allow list --allow-id allow-non-rust --format json",
            "cargo-allow worklist --allow-id allow-non-rust --format json",
            "cargo-allow worklist --missing-evidence --format json",
            "cargo-allow check --kind non-rust --mode no-new",
            "cargo-allow worklist --item-kind missing_evidence --format json",
            "cargo-allow worklist --kind non-rust --format json",
        ]
    );
}

#[test]
fn proof_commands_cover_policy_family_aliases_and_unknown_policy_fallback() {
    let cases = [
        ("executable_file", "executable"),
        ("github_workflow", "workflow"),
        ("workflow_external_action", "workflow"),
        ("dependency_surface", "dependency-surface"),
        ("process_spawn", "process"),
        ("network_destination", "network"),
    ];

    for (family, kind_arg) in cases {
        let mut entry = test_entry(&format!("allow-{family}"), FindingKind::PolicyException);
        entry.family = Some(family.to_string());

        assert_eq!(
            proof_commands("review_due", None, Some(&entry)),
            vec![
                format!("cargo-allow explain allow-{family}"),
                format!("cargo-allow list --allow-id allow-{family} --format json"),
                format!("cargo-allow worklist --allow-id allow-{family} --format json"),
                format!("cargo-allow check --kind {kind_arg} --mode no-new"),
                "cargo-allow worklist --item-kind review_due --format json".to_string(),
                format!("cargo-allow worklist --kind {kind_arg} --format json"),
            ],
            "{family} should map to --kind {kind_arg}"
        );
    }

    let mut unknown = test_entry("allow-policy", FindingKind::PolicyException);
    unknown.family = Some("unknown_policy_family".to_string());
    assert_eq!(
        proof_commands("baseline_debt", None, Some(&unknown)),
        vec![
            "cargo-allow explain allow-policy",
            "cargo-allow list --allow-id allow-policy --format json",
            "cargo-allow worklist --allow-id allow-policy --format json",
            "cargo-allow worklist --baseline-debt --format json",
            "cargo-allow check --mode no-new",
            "cargo-allow worklist --item-kind baseline_debt --format json",
            "cargo-allow worklist --format json",
        ]
    );

    assert_eq!(
        proof_commands("review_due", None, Some(&unknown)),
        vec![
            "cargo-allow explain allow-policy",
            "cargo-allow list --allow-id allow-policy --format json",
            "cargo-allow worklist --allow-id allow-policy --format json",
            "cargo-allow check --mode no-new",
            "cargo-allow worklist --item-kind review_due --format json",
            "cargo-allow worklist --format json",
        ]
    );
}

#[test]
fn missing_evidence_keeps_shortcut_when_kind_is_unknown() {
    let mut entry = test_entry("allow-policy", FindingKind::PolicyException);
    entry.family = Some("unknown_policy_family".to_string());

    assert_eq!(
        proof_commands("missing_evidence", None, Some(&entry)),
        vec![
            "cargo-allow explain allow-policy",
            "cargo-allow list --allow-id allow-policy --format json",
            "cargo-allow worklist --allow-id allow-policy --format json",
            "cargo-allow worklist --missing-evidence --format json",
            "cargo-allow check --mode no-new",
            "cargo-allow worklist --item-kind missing_evidence --format json",
            "cargo-allow worklist --format json",
        ]
    );
}

#[test]
fn unsafe_missing_evidence_adds_unsafe_check_when_kind_is_unknown() {
    let mut entry = test_entry("allow-policy", FindingKind::PolicyException);
    entry.family = Some("unknown_policy_family".to_string());

    assert_eq!(
        proof_commands("unsafe_missing_evidence", None, Some(&entry)),
        vec![
            "cargo-allow explain allow-policy",
            "cargo-allow list --allow-id allow-policy --format json",
            "cargo-allow worklist --allow-id allow-policy --format json",
            "cargo-allow worklist --missing-evidence --format json",
            "cargo-allow check --mode no-new",
            "cargo-allow worklist --item-kind unsafe_missing_evidence --format json",
            "cargo-allow worklist --format json",
            "cargo-allow check --kind unsafe --mode no-new",
        ]
    );
}
