//! Facade tests (#3621): the API surface an external consumer uses,
//! end to end, with caller-supplied inputs only.

use super::ChangieSensor;
use crate::changie::{ChangieRepoPath, ChangieSourceDocument};
use crate::changie_lint::{ChangieCandidateEntry, ChangieEntryState, ChangieLintCandidate};

fn source(path: &str, text: &str) -> ChangieSourceDocument {
    ChangieSourceDocument::from_bytes(
        ChangieRepoPath::from_repo_relative(path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("repo path: {err}"))),
        text.as_bytes().to_vec(),
        Some("external-consumer-subject".to_string()),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("source: {err}")))
}

const PERL_CONFIG: &str = "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\ncustom:\n  - key: PR\n    type: int\n    minInt: 1\n  - key: Slug\n    type: string\n    optional: true\n  - key: Breaking\n    type: enum\n    enum: [no, yes]\n";

#[test]
fn facade_round_trip_from_parse_to_serialized_report() {
    let sensor = ChangieSensor;
    assert_eq!(sensor.generation(), "1.25");
    assert_eq!(sensor.diagnostic_schema_generation(), 1);
    assert_eq!(sensor.effective_rule_schema_generation(), 1);

    let config = sensor.parse_config(source(".changie.yaml", PERL_CONFIG));
    let contract = sensor
        .compile_contract(&config)
        .unwrap_or_else(|err| std::panic::panic_any(format!("compile: {err:?}")));
    let text = sensor.contract_text(&contract);
    assert!(text.contains("choice key=PR type=int optional=false scope=global"));

    let fragment = sensor.parse_fragment(source(
        ".changes/Fixture.yaml",
        "kind: Fixed\nbody: text\ncustom:\n  PR: 3\n  Breaking: no\n",
    ));
    let report = sensor.lint(ChangieLintCandidate {
        config,
        entries: vec![ChangieCandidateEntry {
            repo_path: ".changes/Fixture.yaml".into(),
            state: ChangieEntryState::File,
            fragment: Some(fragment),
        }],
    });
    assert!(report.diagnostics.is_empty(), "{:#?}", report.diagnostics);

    let invalid = sensor.parse_fragment(source(
        ".changes/Bad.yaml",
        "kind: Fixed\nbody: text\ncustom:\n  PR: 0\n  Breaking: maybe\n",
    ));
    let report = sensor.lint(ChangieLintCandidate {
        config: sensor.parse_config(source(".changie.yaml", PERL_CONFIG)),
        entries: vec![ChangieCandidateEntry {
            repo_path: ".changes/Bad.yaml".into(),
            state: ChangieEntryState::File,
            fragment: Some(invalid),
        }],
    });
    let serialized = ChangieSensor.serialize(&report);
    assert!(serialized.starts_with("changie.lint-report.v1\n"));
    assert!(serialized.contains("diagnostic rule=changie.fragment.custom_out_of_range"));
    assert!(serialized.contains("provenance=rust_static_companion"));
    assert!(serialized.contains("expected=minimum 1 actual=0"));
    assert!(serialized.contains("diagnostic rule=changie.fragment.custom_unknown_value"));
    assert!(serialized.contains("field=custom.Breaking"));
    assert!(serialized.contains("discovered=.changes/Bad.yaml"));
}

#[test]
fn equal_inputs_produce_equal_serialized_reports() {
    let sensor = ChangieSensor;
    let run = || {
        let report = sensor.lint(ChangieLintCandidate {
            config: sensor.parse_config(source(".changie.yaml", PERL_CONFIG)),
            entries: vec![ChangieCandidateEntry {
                repo_path: ".changes/Fixture.yaml".into(),
                state: ChangieEntryState::File,
                fragment: Some(sensor.parse_fragment(source(
                    ".changes/Fixture.yaml",
                    "kind: Fixed\nbody: text\ncustom:\n  PR: 0\n  Breaking: no\n",
                ))),
            }],
        });
        ChangieSensor.serialize(&report)
    };
    assert_eq!(run(), run(), "repeated equal inputs serialize equally");
}

#[test]
fn ambiguous_configuration_is_not_clean_through_the_facade() {
    let sensor = ChangieSensor;
    let config = sensor.parse_config(source(
        ".changie.yaml",
        "changesDir: .changes\nchangesDir: .other\nunreleasedDir: .\n",
    ));
    let blocked = sensor.compile_contract(&config);
    assert!(blocked.is_err());
    let report = sensor.lint(ChangieLintCandidate {
        config,
        entries: Vec::new(),
    });
    assert_ne!(
        format!("{:?}", report.completeness),
        "Complete",
        "malformed/ambiguous state stays non-clean through the facade"
    );
    let serialized = ChangieSensor.serialize(&report);
    assert!(serialized.contains("completeness=partial"));
    assert!(serialized.contains("rule=changie.config.unsupported_semantics"));
}

#[test]
fn facade_documents_refuse_absolute_paths() {
    assert!(ChangieRepoPath::from_repo_relative("/etc/passwd").is_err());
    assert!(ChangieRepoPath::from_repo_relative("../escape.yaml").is_err());
    let doc = ChangieSourceDocument::from_bytes(
        ChangieRepoPath::from_repo_relative(".changie.yaml")
            .unwrap_or_else(|err| std::panic::panic_any(err)),
        b"changesDir: .changes\n".to_vec(),
        None,
    )
    .unwrap_or_else(|err| std::panic::panic_any(err));
    assert_eq!(doc.subject(), None);
    assert!(doc.content_identity().byte_len > 0);
}
