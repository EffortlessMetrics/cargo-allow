//! Changie fragment semantic validation tests (#3589 PR B2).

use crate::changie::{ChangieRepoPath, ChangieSourceDocument, parse_config, parse_fragment};
use crate::changie_lint::*;

const ROOTS: &str = "changesDir: .changes\nunreleasedDir: .\n";

// Keep this representative fragment independent from `.changes`: Changie removes live
// fragments when it batches a release.
const REPOSITORY_FRAGMENT_FIXTURE: &str = r##"kind: Changed
body: 'Change-status reporting now resolves typed subjects through a new,
  standalone subject-resolution layer before building the view: graph-validation
  failures and setup failures are classified separately, identical subject IDs must
  resolve to byte-identical content, graph content is projected from stored
  diagnostics rather than recompiled from mutable files, CLI JSON reports the resolved
  subject ID and source-state identity, and staged pre-commit parity, relocation,
  missing-required-subject, dirty-worktree, nested-scope, idempotence, and malformed-root
  fixtures now close the first consumer boundary.'
time: 2026-08-11T15:45:00.000Z
custom:
  Issue: '#2982'
  PR: '#3114'
  Components: cargo-allow
"##;

fn source(path: &str, text: &str) -> ChangieSourceDocument {
    ChangieSourceDocument::from_bytes(
        ChangieRepoPath::from_repo_relative(path).unwrap_or_else(|err| std::panic::panic_any(err)),
        text.as_bytes().to_vec(),
        None,
    )
    .unwrap_or_else(|err| std::panic::panic_any(err))
}

fn lint_with(config_text: &str, fragment_text: &str) -> ChangieLintReport {
    lint(ChangieLintCandidate {
        config: parse_config(source(".changie.yaml", config_text)),
        entries: vec![ChangieCandidateEntry {
            repo_path: ".changes/Fixture.yaml".into(),
            state: ChangieEntryState::File,
            fragment: Some(parse_fragment(source(
                ".changes/Fixture.yaml",
                fragment_text,
            ))),
        }],
    })
}

fn rules(report: &ChangieLintReport) -> Vec<String> {
    report
        .diagnostics
        .iter()
        .map(|d| d.rule.as_str().to_string())
        .collect()
}

const PERL_CONFIG: &str = "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\ncustom:\n  - key: PR\n    type: int\n    minInt: 1\n  - key: Slug\n    type: string\n    optional: true\n  - key: Breaking\n    type: enum\n    enum: [no, yes]\n";

#[test]
fn perl_fixture_semantics_hold() {
    let report = lint_with(
        PERL_CONFIG,
        "kind: Fixed\nbody: text\ncustom:\n  PR: 12\n  Breaking: yes\n",
    );
    assert!(
        report.diagnostics.is_empty(),
        "required int in range, optional string absent, required enum valid: {:#?}",
        report.diagnostics
    );

    let missing_pr = lint_with(
        PERL_CONFIG,
        "kind: Fixed\nbody: text\ncustom:\n  Breaking: no\n",
    );
    assert!(rules(&missing_pr).contains(&"changie.fragment.custom_missing".to_string()));

    let low_pr = lint_with(
        PERL_CONFIG,
        "kind: Fixed\nbody: text\ncustom:\n  PR: 0\n  Breaking: no\n",
    );
    assert!(rules(&low_pr).contains(&"changie.fragment.custom_out_of_range".to_string()));
    let low = low_pr
        .diagnostics
        .iter()
        .find(|d| d.rule.as_str() == "changie.fragment.custom_out_of_range")
        .unwrap_or_else(|| std::panic::panic_any("range finding missing"));
    assert_eq!(
        low.expected_actual.as_ref().map(|ea| ea.expected.clone()),
        Some("minimum 1".into())
    );

    let bad_breaking = lint_with(
        PERL_CONFIG,
        "kind: Fixed\nbody: text\ncustom:\n  PR: 3\n  Breaking: maybe\n",
    );
    assert!(rules(&bad_breaking).contains(&"changie.fragment.custom_unknown_value".to_string()));

    let pr_string = lint_with(
        PERL_CONFIG,
        "kind: Fixed\nbody: text\ncustom:\n  PR: \"twelve\"\n  Breaking: no\n",
    );
    assert!(rules(&pr_string).contains(&"changie.fragment.custom_wrong_type".to_string()));
}

#[test]
fn body_length_uses_upstream_byte_semantics() {
    // "é" is 1 rune but 2 UTF-8 bytes; upstream measures bytes.
    let config =
        "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\nbody:\n  minLength: 2\n";
    let report = lint_with(config, "kind: Fixed\nbody: é\n");
    assert!(
        report.diagnostics.is_empty(),
        "2 bytes satisfies minLength 2 under byte semantics: {:#?}",
        report.diagnostics
    );

    let stricter = lint_with(
        "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\nbody:\n  minLength: 3\n",
        "kind: Fixed\nbody: é\n",
    );
    assert!(rules(&stricter).contains(&"changie.fragment.body_too_short".to_string()));
    let finding = stricter
        .diagnostics
        .iter()
        .find(|d| d.rule.as_str() == "changie.fragment.body_too_short")
        .unwrap_or_else(|| std::panic::panic_any("too_short missing"));
    assert!(finding.message.contains("utf8-bytes"));
    assert!(
        finding
            .message
            .contains("2 utf8-bytes; the configured minimum is 3")
    );
}

#[test]
fn body_required_unless_kind_skips_it() {
    let with_kinds = "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\n  - label: Meta\n    skipBody: true\n";
    let missing = lint_with(with_kinds, "kind: Fixed\ncustom: {}\n");
    assert!(rules(&missing).contains(&"changie.fragment.body_missing".to_string()));

    let skipped = lint_with(with_kinds, "kind: Meta\ncustom: {}\n");
    assert!(
        !rules(&skipped).contains(&"changie.fragment.body_missing".to_string()),
        "skipBody removes the requirement: {:#?}",
        skipped.diagnostics
    );

    let empty = lint_with(with_kinds, "kind: Fixed\nbody: \"\"\n");
    let empty_finding = empty
        .diagnostics
        .iter()
        .find(|d| d.rule.as_str() == "changie.fragment.body_missing")
        .unwrap_or_else(|| std::panic::panic_any("empty body not flagged"));
    assert!(empty_finding.message.contains("authored empty"));

    let wrong_type = lint_with(with_kinds, "kind: Fixed\nbody: 42\n");
    assert!(rules(&wrong_type).contains(&"changie.fragment.body_wrong_type".to_string()));
}

#[test]
fn kind_rules_do_not_fall_back_for_unknown_kinds() {
    let config = "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\n";
    let unknown = lint_with(config, "kind: Added\nbody: text\n");
    assert!(rules(&unknown).contains(&"changie.fragment.kind_unknown".to_string()));

    let missing = lint_with(config, "body: text\n");
    assert!(rules(&missing).contains(&"changie.fragment.kind_missing".to_string()));

    let no_kinds = lint_with(ROOTS, "body: text\n");
    assert!(
        !rules(&no_kinds).contains(&"changie.fragment.kind_missing".to_string()),
        "no configured kinds means no kind gate"
    );
}

#[test]
fn component_rules_follow_the_configured_set() {
    let config = "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\ncomponents: [ui, core]\n";
    let ok = lint_with(config, "kind: Fixed\nbody: b\ncomponent: ui\n");
    assert!(ok.diagnostics.is_empty(), "{:#?}", ok.diagnostics);

    let missing = lint_with(config, "kind: Fixed\nbody: b\n");
    assert!(rules(&missing).contains(&"changie.fragment.component_missing".to_string()));

    let unknown = lint_with(config, "kind: Fixed\nbody: b\ncomponent: api\n");
    assert!(rules(&unknown).contains(&"changie.fragment.component_unknown".to_string()));

    let unconfigured = lint_with(ROOTS, "body: b\n");
    assert!(
        !rules(&unconfigured).contains(&"changie.fragment.component_missing".to_string()),
        "no configured components means no hidden policy"
    );
}

#[test]
fn project_rules_match_keys_only() {
    let config = "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\nprojects:\n  - key: backend\n    label: Backend\n";
    let ok = lint_with(config, "kind: Fixed\nbody: b\nproject: backend\n");
    assert!(ok.diagnostics.is_empty(), "{:#?}", ok.diagnostics);

    let label_only = lint_with(config, "kind: Fixed\nbody: b\nproject: Backend\n");
    // Canonical identity law: a label is a source-located finding with a
    // canonicalization action, not a silent normalization and not a bare
    // unknown.
    let canonical = label_only
        .diagnostics
        .iter()
        .find(|d| d.rule.as_str() == "changie.fragment.project_not_canonical")
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "label should be not_canonical, got {:#?}",
                label_only.diagnostics
            ))
        });
    assert_eq!(
        canonical
            .expected_actual
            .as_ref()
            .map(|ea| ea.expected.clone()),
        Some("backend".to_string())
    );
    assert!(
        canonical
            .actions
            .contains(&ChangieAction::CanonicalizeConfiguredValue)
    );

    let missing = lint_with(config, "kind: Fixed\nbody: b\n");
    assert!(rules(&missing).contains(&"changie.fragment.project_missing".to_string()));
}

#[test]
fn time_is_required_only_when_timeformat_is_configured() {
    let with_time = "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\ntimeFormat: 2006-01-02 15:04:05\n";
    let ok = lint_with(
        with_time,
        "kind: Fixed\nbody: b\ntime: 2026-08-16T10:30:00Z\n",
    );
    assert!(ok.diagnostics.is_empty(), "{:#?}", ok.diagnostics);

    let missing = lint_with(with_time, "kind: Fixed\nbody: b\n");
    assert!(rules(&missing).contains(&"changie.fragment.time_missing".to_string()));

    let invalid = lint_with(with_time, "kind: Fixed\nbody: b\ntime: yesterday\n");
    assert!(rules(&invalid).contains(&"changie.fragment.time_invalid".to_string()));

    let out_of_range = lint_with(
        with_time,
        "kind: Fixed\nbody: b\ntime: 2026-13-40T10:30:00Z\n",
    );
    assert!(rules(&out_of_range).contains(&"changie.fragment.time_invalid".to_string()));

    let _no_format = lint_with(ROOTS, "kind: Fixed\nbody: b\n");
    let with_kinds = "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\n";
    let no_format = lint_with(with_kinds, "kind: Fixed\nbody: b\n");
    assert!(
        !rules(&no_format).contains(&"changie.fragment.time_missing".to_string()),
        "no timeFormat means no time expectation, matching this repository's fragments"
    );
}

#[test]
fn rfc3339_shape_accepts_offsets_fractions_and_rejects_ambiguity() {
    let with_time =
        "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\ntimeFormat: RFC3339\n";
    for good in [
        "2026-08-16T10:30:00Z",
        "2026-08-16t10:30:00z",
        "2026-08-16 10:30:00+02:00",
        "2026-08-16T10:30:00.123456-07:30",
        "2026-02-29T23:59:60Z",
    ] {
        let report = lint_with(with_time, &format!("kind: Fixed\nbody: b\ntime: {good}\n"));
        assert!(
            !rules(&report).contains(&"changie.fragment.time_invalid".to_string()),
            "{good} should parse: {:#?}",
            report.diagnostics
        );
    }
    for bad in [
        "2026-08-16",               // date only
        "2026-08-16T10:30Z",        // no seconds
        "2026-08-16T10:30:00",      // no offset: ambiguous local time
        "2026-08-16T10:30:00+2:00", // short offset hour
        "not-a-time",
    ] {
        let report = lint_with(with_time, &format!("kind: Fixed\nbody: b\ntime: {bad}\n"));
        assert!(
            rules(&report).contains(&"changie.fragment.time_invalid".to_string()),
            "{bad} should be invalid"
        );
    }
}

#[test]
fn custom_scoping_honors_skip_global_choices_and_additional() {
    let config = "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\n  - label: Internal\n    skipGlobalChoices: true\n    additionalChoices:\n      - key: Ticket\n        type: int\ncustom:\n  - key: PR\n    type: int\n";
    let global_required = lint_with(config, "kind: Fixed\nbody: b\ncustom:\n  Ticket: 1\n");
    assert!(
        rules(&global_required).contains(&"changie.fragment.custom_missing".to_string()),
        "global PR is required for Fixed"
    );

    let skipped = lint_with(config, "kind: Internal\nbody: b\ncustom:\n  Ticket: 9\n");
    assert!(
        !rules(&skipped).contains(&"changie.fragment.custom_missing".to_string()),
        "skipGlobalChoices removes PR for Internal: {:#?}",
        skipped.diagnostics
    );

    let additional_missing = lint_with(config, "kind: Internal\nbody: b\n");
    let found = rules(&additional_missing);
    assert!(
        found.contains(&"changie.fragment.custom_missing".to_string()),
        "kind additionalChoices are required: {found:#?}"
    );
}

#[test]
fn unconfigured_custom_keys_stay_visible() {
    let config = "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\ncustom:\n  - key: PR\n    type: int\n";
    let report = lint_with(
        config,
        "kind: Fixed\nbody: b\ncustom:\n  PR: 1\n  Mystery: yes\n",
    );
    assert!(rules(&report).contains(&"changie.fragment.custom_unconfigured".to_string()));
}

#[test]
fn repository_fragment_fixture_lints_clean() {
    let config_text = include_str!("../../../../.changie.yaml");
    let report = lint_with(config_text, REPOSITORY_FRAGMENT_FIXTURE);
    assert!(
        report.diagnostics.is_empty(),
        "real config + permanent representative fragment: {:#?}",
        report.diagnostics
    );
}

#[test]
fn string_choices_enforce_byte_bounds() {
    let config = "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\ncustom:\n  - key: Slug\n    type: string\n    minLength: 2\n    maxLength: 4\n";
    let ok = lint_with(config, "kind: Fixed\nbody: b\ncustom:\n  Slug: ab\n");
    assert!(ok.diagnostics.is_empty(), "{:#?}", ok.diagnostics);

    let too_short = lint_with(config, "kind: Fixed\nbody: b\ncustom:\n  Slug: a\n");
    assert!(rules(&too_short).contains(&"changie.fragment.custom_out_of_range".to_string()));

    let too_long = lint_with(config, "kind: Fixed\nbody: b\ncustom:\n  Slug: abcde\n");
    assert!(rules(&too_long).contains(&"changie.fragment.custom_out_of_range".to_string()));
}
