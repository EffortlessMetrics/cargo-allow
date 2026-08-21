//! Compiled effective contract tests (#3620): determinism, digest,
//! canonical-identity law, string-valued integers, fail-honest
//! blockers, and the falsifier list.

use crate::changie::parse_config;
use crate::changie::{ChangieRepoPath, ChangieSourceDocument};
use crate::changie_lint::compiled_contract::{
    ContractCompileBlocker, canonical_contract_text, compile_contract,
};
use crate::changie_lint::*;

fn config_doc(text: &str) -> crate::changie::ChangieConfigDocument {
    parse_config(
        ChangieSourceDocument::from_bytes(
            ChangieRepoPath::from_repo_relative(".changie.yaml")
                .unwrap_or_else(|err| std::panic::panic_any(err)),
            text.as_bytes().to_vec(),
            None,
        )
        .unwrap_or_else(|err| std::panic::panic_any(err)),
    )
}

const PERL_CONFIG: &str = "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\ncustom:\n  - key: PR\n    type: int\n    minInt: 1\n  - key: Slug\n    type: string\n    optional: true\n  - key: Breaking\n    type: enum\n    enum: [no, yes]\n";

#[test]
fn perl_fixture_compiles_to_the_ruling_shape() {
    let contract = compile_contract(&config_doc(PERL_CONFIG))
        .unwrap_or_else(|err| std::panic::panic_any(format!("compile: {err:?}")));
    assert_eq!(contract.generation, "1.25");
    let pr = contract
        .choices
        .iter()
        .find(|choice| choice.key == "PR")
        .unwrap_or_else(|| std::panic::panic_any("PR choice missing"));
    assert_eq!(pr.choice_type.as_str(), "int");
    assert!(!pr.optional);
    assert_eq!(pr.min_int, Some(1));
    let slug = contract
        .choices
        .iter()
        .find(|choice| choice.key == "Slug")
        .unwrap_or_else(|| std::panic::panic_any("Slug choice missing"));
    assert_eq!(slug.choice_type.as_str(), "string");
    assert!(slug.optional);
    let breaking = contract
        .choices
        .iter()
        .find(|choice| choice.key == "Breaking")
        .unwrap_or_else(|| std::panic::panic_any("Breaking choice missing"));
    assert_eq!(breaking.choice_type.as_str(), "enum");
    assert_eq!(
        breaking.enum_options,
        vec!["no".to_string(), "yes".to_string()]
    );
    assert!(!breaking.optional);
    // No repository names or house keys leak into the contract.
    assert!(
        contract
            .choices
            .iter()
            .all(|choice| !choice.key.contains("github"))
    );
}

#[test]
fn contract_is_deterministic_and_digest_discriminating() {
    let first = compile_contract(&config_doc(PERL_CONFIG))
        .unwrap_or_else(|err| std::panic::panic_any(format!("compile: {err:?}")));
    let second = compile_contract(&config_doc(PERL_CONFIG))
        .unwrap_or_else(|err| std::panic::panic_any(format!("compile: {err:?}")));
    assert_eq!(first, second, "equal configurations compile equally");
    assert_eq!(
        canonical_contract_text(&first),
        canonical_contract_text(&second)
    );

    let changed = compile_contract(&config_doc(
        "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\ncustom:\n  - key: PR\n    type: int\n    minInt: 2\n",
    ))
    .unwrap_or_else(|err| std::panic::panic_any(format!("compile: {err:?}")));
    assert_ne!(
        first.digest, changed.digest,
        "semantic change changes the digest"
    );

    // The contract retains the exact config identity (byte-level), so a
    // reordered document is a different contract by identity — but the
    // effective semantic surface (kinds, choices) is identical.
    let reordered = compile_contract(&config_doc(concat!(
        "unreleasedDir: .\n",
        "changesDir: .changes\n",
        "kinds:\n",
        "  - label: Fixed\n",
        "custom:\n",
        "  - key: PR\n",
        "    type: int\n",
        "    minInt: 1\n",
        "  - key: Slug\n",
        "    type: string\n",
        "    optional: true\n",
        "  - key: Breaking\n",
        "    type: enum\n",
        "    enum: [no, yes]\n",
    )))
    .unwrap_or_else(|err| std::panic::panic_any(format!("compile: {err:?}")));
    assert_eq!(first.kinds, reordered.kinds);
    assert_eq!(first.choices, reordered.choices);
    assert_eq!(first.body, reordered.body);
    assert_ne!(first.config_identity, reordered.config_identity);
}

#[test]
fn ambiguous_configurations_fail_closed() {
    let duplicate_key = compile_contract(&config_doc(
        "changesDir: .changes\nchangesDir: .other\nunreleasedDir: .\n",
    ));
    assert!(matches!(
        duplicate_key,
        Err(ContractCompileBlocker::AmbiguousConfiguration(_))
    ));

    let duplicate_kind = compile_contract(&config_doc(
        "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\n  - label: Fixed\n",
    ));
    assert!(matches!(
        duplicate_kind,
        Err(ContractCompileBlocker::AmbiguousConfiguration(_))
    ));

    let duplicate_project = compile_contract(&config_doc(
        "changesDir: .changes\nunreleasedDir: .\nprojects:\n  - key: a\n  - key: a\n",
    ));
    assert!(matches!(
        duplicate_project,
        Err(ContractCompileBlocker::AmbiguousConfiguration(_))
    ));

    let malformed = compile_contract(&config_doc("kinds:\n\t- tabbed\n"));
    assert!(matches!(
        malformed,
        Err(ContractCompileBlocker::MalformedConfiguration)
    ));
}

#[test]
fn lint_reports_the_blocker_and_skips_fragment_semantics() {
    let report = lint(ChangieLintCandidate {
        config: config_doc("changesDir: .changes\nchangesDir: .other\nunreleasedDir: .\n"),
        entries: Vec::new(),
    });
    assert_eq!(report.completeness, ChangieCompleteness::Partial);
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.rule.as_str() == "changie.config.unsupported_semantics"
            && diagnostic
                .message
                .contains("fragment contract not compiled")
    }));
}

#[test]
fn canonical_text_is_stable_and_consumable() {
    let contract = compile_contract(&config_doc(PERL_CONFIG))
        .unwrap_or_else(|err| std::panic::panic_any(format!("compile: {err:?}")));
    let text = canonical_contract_text(&contract);
    assert!(text.starts_with("changie.compiled-fragment-contract.v1\n"));
    assert!(text.contains("choice key=PR type=int optional=false scope=global"));
    assert!(text.contains("choice key=Breaking type=enum optional=false scope=global"));
    assert!(text.contains("enum=no,yes"));
    // Line order is fixed: the canonical text of an equal contract is
    // byte-equal (verified by the determinism test).
}

#[test]
fn string_valued_integer_is_accepted_not_required() {
    // Falsifier 2: a configured int choice must not require a YAML
    // integer — Changie persists custom values as strings, so the
    // string form parses and validates.
    let report = lint(ChangieLintCandidate {
        config: config_doc(PERL_CONFIG),
        entries: vec![fragment(
            "kind: Fixed\nbody: text\ncustom:\n  PR: \"12\"\n  Breaking: no\n",
        )],
    });
    assert!(
        report.diagnostics.is_empty(),
        "string-valued int in range must be clean: {:#?}",
        report.diagnostics
    );
    let out_of_range = lint(ChangieLintCandidate {
        config: config_doc(PERL_CONFIG),
        entries: vec![fragment(
            "kind: Fixed\nbody: text\ncustom:\n  PR: \"0\"\n  Breaking: no\n",
        )],
    });
    assert!(
        out_of_range
            .diagnostics
            .iter()
            .any(|d| d.rule.as_str() == "changie.fragment.custom_out_of_range")
    );
}

#[test]
fn every_diagnostic_carries_a_provenance_class() {
    let report = lint(ChangieLintCandidate {
        config: config_doc("changesDir: /absolute\nunknownField: 1\n"),
        entries: vec![fragment("kind: Added\nbody: x\n")],
    });
    assert!(!report.diagnostics.is_empty());
    for diagnostic in &report.diagnostics {
        let provenance = diagnostic.provenance();
        assert!(
            !provenance.is_empty(),
            "rule {} lost its provenance",
            diagnostic.rule.as_str()
        );
    }
    // Source-safety rules keep their class.
    let path_finding = report
        .diagnostics
        .iter()
        .find(|d| d.rule.as_str() == "changie.config.path_invalid")
        .unwrap_or_else(|| std::panic::panic_any("path finding missing"));
    assert_eq!(path_finding.provenance(), "source_safety");
}

#[test]
fn post_generated_key_is_never_required_input() {
    // Falsifier 6: only configured choices are required; an unknown key
    // present in the fragment stays visible and is never demanded.
    let report = lint(ChangieLintCandidate {
        config: config_doc(PERL_CONFIG),
        entries: vec![fragment(
            "kind: Fixed\nbody: text\ncustom:\n  PR: 5\n  Breaking: yes\n  TicketLink: generated\n",
        )],
    });
    assert!(report.diagnostics.iter().any(|d| {
        d.rule.as_str() == "changie.fragment.custom_unconfigured"
            && d.message.contains("TicketLink")
    }));
    assert!(!report.diagnostics.iter().any(|d| {
        d.rule.as_str() == "changie.fragment.custom_missing" && d.message.contains("TicketLink")
    }));
}

fn fragment(text: &str) -> ChangieCandidateEntry {
    ChangieCandidateEntry {
        repo_path: ".changes/Fixture.yaml".into(),
        state: ChangieEntryState::File,
        fragment: Some(crate::changie::parse_fragment(
            ChangieSourceDocument::from_bytes(
                ChangieRepoPath::from_repo_relative(".changes/Fixture.yaml")
                    .unwrap_or_else(|err| std::panic::panic_any(err)),
                text.as_bytes().to_vec(),
                None,
            )
            .unwrap_or_else(|err| std::panic::panic_any(err)),
        )),
    }
}
