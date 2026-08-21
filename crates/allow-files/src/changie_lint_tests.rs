//! Changie sensor lint tests (#3589 PR B1).

use crate::changie::{ChangieRepoPath, ChangieSourceDocument, parse_config, parse_fragment};
use crate::changie_lint::*;

fn config_doc(text: &str) -> ChangieConfigDocument {
    parse_config(source(".changie.yaml", text))
}

fn source(path: &str, text: &str) -> ChangieSourceDocument {
    ChangieSourceDocument::from_bytes(
        ChangieRepoPath::from_repo_relative(path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("repo path: {err}"))),
        text.as_bytes().to_vec(),
        None,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("source: {err}")))
}

fn entry(path: &str, state: ChangieEntryState) -> ChangieCandidateEntry {
    ChangieCandidateEntry {
        repo_path: path.to_string(),
        state,
        fragment: None,
    }
}

fn rule_strings(report: &ChangieLintReport) -> Vec<String> {
    report
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.rule.as_str().to_string())
        .collect()
}

#[test]
fn real_repository_config_lints_clean() {
    let text = include_str!("../../../.changie.yaml");
    let report = lint(ChangieLintCandidate {
        config: config_doc(text),
        entries: vec![entry(
            ".changes/Fixed-20260816-example.yaml",
            ChangieEntryState::File,
        )],
    });
    assert!(
        report.diagnostics.is_empty(),
        "real config diagnostics: {:#?}",
        report.diagnostics
    );
    assert_eq!(report.generation, "1.25");
    assert_eq!(
        report.discovered,
        vec![".changes/Fixed-20260816-example.yaml".to_string()]
    );
    assert!(report.not_discovered.is_empty());
    assert_eq!(report.completeness, ChangieCompleteness::NotProven);
    assert!(
        report
            .not_proven_dimensions
            .contains(&"template_render_semantics"),
        "the real config uses format templates: {:#?}",
        report.not_proven_dimensions
    );
}

#[test]
fn duplicate_config_keys_are_findings_with_ranges() {
    let report = lint(ChangieLintCandidate {
        config: config_doc("changesDir: .changes\nchangesDir: .other\n"),
        entries: Vec::new(),
    });
    assert!(rule_strings(&report).contains(&"changie.config.duplicate_key".to_string()));
    let duplicate = report
        .diagnostics
        .iter()
        .find(|d| d.rule.as_str() == "changie.config.duplicate_key")
        .unwrap_or_else(|| std::panic::panic_any("duplicate finding missing"));
    assert!(duplicate.range.is_some());
    assert_eq!(duplicate.result_class, ChangieResultClass::Finding);
}

#[test]
fn unknown_fields_and_paths_get_stable_rule_ids() {
    let report = lint(ChangieLintCandidate {
        config: config_doc("changesDir: /absolute\nunknownField: 1\n"),
        entries: Vec::new(),
    });
    let rules = rule_strings(&report);
    assert!(rules.contains(&"changie.config.unknown_field".to_string()));
    assert!(rules.contains(&"changie.config.path_invalid".to_string()));
    let path = report
        .diagnostics
        .iter()
        .find(|d| d.rule.as_str() == "changie.config.path_invalid")
        .unwrap_or_else(|| std::panic::panic_any("path finding missing"));
    assert_eq!(
        path.expected_actual.as_ref().map(|ea| ea.expected.clone()),
        Some("repository-relative path".to_string())
    );
}

#[test]
fn normalization_collisions_relate_both_declarations() {
    let report = lint(ChangieLintCandidate {
        config: config_doc(
            "changesDir: .changes\nunreleasedDir: .\nheaderPath: .changes/./header.md\nheaderPath2: 1\nchangelogPath: .changes//header.md\n",
        ),
        entries: Vec::new(),
    });
    let collision = report
        .diagnostics
        .iter()
        .find(|d| d.message.contains("normalizes onto the same path"))
        .unwrap_or_else(|| {
            std::panic::panic_any(format!("collision not reported: {:#?}", report.diagnostics))
        });
    assert!(
        !collision.related_config_ranges.is_empty(),
        "the prior declaration's range must be related"
    );
}

#[test]
fn missing_discovery_roots_are_findings() {
    let report = lint(ChangieLintCandidate {
        config: config_doc("changelogPath: CHANGELOG.md\n"),
        entries: Vec::new(),
    });
    let rules = rule_strings(&report);
    assert!(rules.contains(&"changie.config.path_invalid".to_string()));
    let missing: Vec<String> = report
        .diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "changie.config.path_invalid")
        .filter_map(|d| d.field_path.as_ref().map(|p| p.to_string()))
        .collect();
    let joined = missing.join(",");
    assert!(joined.contains("changesDir"), "got {joined}");
    assert!(joined.contains("unreleasedDir"), "got {joined}");
}

#[test]
fn body_constraints_validate_types_order_and_sign() {
    let report = lint(ChangieLintCandidate {
        config: config_doc("body:\n  minLength: 5\n  maxLength: 2\n"),
        entries: Vec::new(),
    });
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.message.contains("exceeds"))
    );

    let typed = lint(ChangieLintCandidate {
        config: config_doc("body:\n  minLength: \"5\"\n"),
        entries: Vec::new(),
    });
    assert!(
        typed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("must be an integer"))
    );

    let negative = lint(ChangieLintCandidate {
        config: config_doc("body:\n  minLength: -1\n"),
        entries: Vec::new(),
    });
    assert!(
        negative
            .diagnostics
            .iter()
            .any(|d| d.message.contains("non-negative"))
    );
}

#[test]
fn custom_choices_enforce_the_type_contract() {
    let base = "changesDir: .changes\nunreleasedDir: .\ncustom:\n  - key: PR\n    type: int\n";
    let ok = lint(ChangieLintCandidate {
        config: config_doc(base),
        entries: Vec::new(),
    });
    assert!(ok.diagnostics.is_empty(), "{:#?}", ok.diagnostics);

    let unsupported = lint(ChangieLintCandidate {
        config: config_doc(
            "changesDir: .changes\nunreleasedDir: .\ncustom:\n  - key: PR\n    type: float\n",
        ),
        entries: Vec::new(),
    });
    assert!(
        rule_strings(&unsupported).contains(&"changie.config.unsupported_semantics".to_string())
    );

    let enum_empty = lint(ChangieLintCandidate {
        config: config_doc("custom:\n  - key: Breaking\n    type: enum\n    enum: []\n"),
        entries: Vec::new(),
    });
    assert!(
        enum_empty
            .diagnostics
            .iter()
            .any(|d| d.message.contains("no options"))
    );

    let enum_dup = lint(ChangieLintCandidate {
        config: config_doc("custom:\n  - key: Breaking\n    type: enum\n    enum: [no, no, yes]\n"),
        entries: Vec::new(),
    });
    assert!(
        enum_dup
            .diagnostics
            .iter()
            .any(|d| d.message.contains("duplicate enum option"))
    );

    let key_dup = lint(ChangieLintCandidate {
        config: config_doc("custom:\n  - key: PR\n    type: int\n  - key: PR\n    type: string\n"),
        entries: Vec::new(),
    });
    assert!(
        key_dup
            .diagnostics
            .iter()
            .any(|d| d.message.contains("duplicate custom choice key"))
    );
}

#[test]
fn components_and_kinds_validate_uniqueness_and_shape() {
    let components = lint(ChangieLintCandidate {
        config: config_doc("changesDir: .changes\nunreleasedDir: .\ncomponents: [ui, ui, \"\"]\n"),
        entries: Vec::new(),
    });
    assert!(rule_strings(&components).contains(&"changie.config.duplicate_component".to_string()));
    assert!(
        components
            .diagnostics
            .iter()
            .any(|d| d.message.contains("empty"))
    );

    let kinds = lint(ChangieLintCandidate {
        config: config_doc("kinds:\n  - label: Fixed\n  - label: Fixed\n  - body: 1\n"),
        entries: Vec::new(),
    });
    assert!(rule_strings(&kinds).contains(&"changie.config.duplicate_kind".to_string()));
    assert!(
        kinds
            .diagnostics
            .iter()
            .any(|d| d.message.contains("missing its `label`"))
    );
}

#[test]
fn operation_affecting_fields_degrade_completeness_not_cleanliness() {
    let report = lint(ChangieLintCandidate {
        config: config_doc("changesDir: .changes\nunreleasedDir: .\npost: []\n"),
        entries: Vec::new(),
    });
    assert_eq!(report.completeness, ChangieCompleteness::NotProven);
    assert!(report.not_proven_dimensions.contains(&"post_execution"));
    assert!(
        report.diagnostics.is_empty(),
        "operation-specific fields are limitations, not findings: {:#?}",
        report.diagnostics
    );
}

#[test]
fn malformed_config_and_fragments_keep_rule_identity() {
    let report = lint(ChangieLintCandidate {
        config: config_doc("kinds:\n\t- tabbed\n"),
        entries: Vec::new(),
    });
    assert!(rule_strings(&report).contains(&"changie.config.malformed".to_string()));

    let fragment = parse_fragment(source(".changes/Broken.yaml", "kind: [unclosed\n"));
    let report = lint(ChangieLintCandidate {
        config: config_doc(include_str!("../../../.changie.yaml")),
        entries: vec![ChangieCandidateEntry {
            repo_path: ".changes/Broken.yaml".into(),
            state: ChangieEntryState::File,
            fragment: Some(fragment),
        }],
    });
    assert!(rule_strings(&report).contains(&"changie.fragment.malformed".to_string()));
    assert!(
        report
            .discovered
            .contains(&".changes/Broken.yaml".to_string()),
        "a malformed fragment stays in the population report"
    );
}

#[test]
fn discovery_contract_is_exact_about_extension_and_nesting() {
    let config = config_doc(include_str!("../../../.changie.yaml"));
    let report = lint(ChangieLintCandidate {
        config,
        entries: vec![
            entry(".changes/Fixed-a.yaml", ChangieEntryState::File),
            entry(".changes/Fixed-b.yml", ChangieEntryState::File),
            entry(".changes/Fixed-c.YAML", ChangieEntryState::File),
            entry(".changes/nested/Fixed-d.yaml", ChangieEntryState::File),
            entry("docs/other.yaml", ChangieEntryState::File),
            entry(".changes/dir.yaml", ChangieEntryState::Directory),
            entry(".changes/link.yaml", ChangieEntryState::Symlink),
            entry(".changes/gone.yaml", ChangieEntryState::DeletedTracked),
        ],
    });
    assert_eq!(
        report.discovered,
        vec![".changes/Fixed-a.yaml".to_string()],
        "only exact-extension direct children are ordinary fragments"
    );
    let rules = rule_strings(&report);
    assert!(rules.contains(&"changie.fragment.path_not_discovered".to_string()));
    assert!(rules.contains(&"changie.fragment.entry_unsupported".to_string()));
    let yml = report
        .diagnostics
        .iter()
        .find(|d| d.repo_path.ends_with("Fixed-b.yml"))
        .unwrap_or_else(|| std::panic::panic_any(".yml entry not diagnosed"));
    assert!(yml.message.contains("not the exact `.yaml`"));
    let nested = report
        .diagnostics
        .iter()
        .find(|d| d.repo_path.contains("nested/"))
        .unwrap_or_else(|| std::panic::panic_any("nested entry not diagnosed"));
    assert!(nested.message.contains("direct children only"));
    let directory = report
        .diagnostics
        .iter()
        .find(|d| d.repo_path.ends_with("dir.yaml"))
        .unwrap_or_else(|| std::panic::panic_any("directory entry not diagnosed"));
    assert_eq!(directory.result_class, ChangieResultClass::Partial);
    assert_eq!(
        directory
            .expected_actual
            .as_ref()
            .map(|ea| ea.actual.clone()),
        Some("directory".to_string())
    );
}

#[test]
fn empty_candidate_population_is_partial_not_clean() {
    let report = lint(ChangieLintCandidate {
        config: config_doc("changesDir: .changes\nunreleasedDir: .\n"),
        entries: Vec::new(),
    });
    assert_eq!(report.completeness, ChangieCompleteness::Partial);
}

#[test]
fn safe_actions_are_descriptors_never_edits() {
    let report = lint(ChangieLintCandidate {
        config: config_doc("changesDir: .changes\nchangesDir: .x\n"),
        entries: Vec::new(),
    });
    for diagnostic in &report.diagnostics {
        for action in &diagnostic.actions {
            let text = action.as_str();
            assert!(
                !text.contains("delete")
                    && !text.contains("replace with")
                    && !text.contains("set to"),
                "action must be a descriptor, not an edit: {text}"
            );
        }
    }
}
