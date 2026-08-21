//! Fragment JSON Schema projection tests (#3613): the required fixture
//! list, the negative controls, determinism, and independent-schema
//! validation via python jsonschema in the CI-side smoke (the pure-Rust
//! fixtures here cover structure, identity, and parity).

use super::{fragment_json_schema, fragment_schema_association};
use crate::changie::parse_config;
use crate::changie::{ChangieRepoPath, ChangieSourceDocument};
use crate::changie_lint::compiled_contract::compile_contract;

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

fn compiled(
    text: &str,
) -> crate::changie_lint::compiled_contract::ChangieCompiledFragmentContractV1 {
    compile_contract(&config_doc(text))
        .unwrap_or_else(|err| std::panic::panic_any(format!("compile: {err:?}")))
}

const CARGO_ALLOW_CONFIG: &str = concat!(
    "changesDir: .changes\n",
    "unreleasedDir: .\n",
    "kinds:\n",
    "  - label: Fixed\n",
    "body:\n",
    "  minLength: 1\n",
    "changeFormat: \"- {{.Body}}\"\n",
);
const PERL_CONFIG: &str = "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\n  - label: Internal\n    skipBody: true\n    skipGlobalChoices: true\n    additionalChoices:\n      - key: Ticket\n        type: int\n        minInt: 1\nprojects:\n  - key: backend\n    label: Backend\ncomponents: [ui, core]\ncustom:\n  - key: PR\n    type: int\n    minInt: 1\n  - key: Slug\n    type: string\n    optional: true\n  - key: Breaking\n    type: enum\n    enum: [no, yes]\n";

#[test]
fn cargo_allow_flat_config_projects_kind_and_body_shape() {
    // Fixture 1: the flat single-changelog configuration.
    let schema = fragment_json_schema(&compiled(CARGO_ALLOW_CONFIG));
    let text = schema.schema_text;
    assert!(text.contains("\"enum\": [\"Fixed\"]"));
    assert!(text.contains("\"minLength\": 1"));
    assert!(text.contains("\"required\": [\"kind\", \"body\"]"));
    // Flat config has no projects/components: those properties must be
    // absent so the schema does not invent vocabulary.
    assert!(!text.contains("\"project\""));
    assert!(!text.contains("\"component\""));
}

#[test]
fn perl_config_projects_components_and_custom_choices() {
    // Fixture 2/3: projects, components, optional and required choices.
    let schema = fragment_json_schema(&compiled(PERL_CONFIG));
    let text = schema.schema_text;
    // Canonical project keys, never labels (negative control 4).
    assert!(text.contains("\"enum\": [\"backend\"]"));
    assert!(!text.contains("\"enum\": [\"Backend\"]"));
    assert!(
        text.contains("\"enum\": [\"ui, core\"]")
            || text.contains("\"ui\"") && text.contains("\"core\"")
    );
    // Optional Slug is not in the global custom required list; PR and
    // Breaking are.
    assert!(text.contains("\"required\": [\"PR\", \"Breaking\"]"));
    assert!(!text.contains("\"required\": [\"PR\", \"Slug\""));
}

#[test]
fn kind_specific_branches_are_deterministic_and_ordered() {
    // Fixture 4: skipBody, skipGlobalChoices, additionalChoices.
    let schema = fragment_json_schema(&compiled(PERL_CONFIG));
    let text = schema.schema_text;
    let internal_at = text
        .find("\"const\": \"Internal\"")
        .unwrap_or_else(|| std::panic::panic_any("Internal kind branch missing"));
    let fixed_at = text
        .find("\"const\": \"Fixed\"")
        .unwrap_or_else(|| std::panic::panic_any("Fixed kind branch missing"));
    assert!(
        fixed_at < internal_at,
        "branch order follows compiled kind order"
    );
    // The Internal branch runs to the end of the allOf array; assert
    // containment in order without slicing a byte window.
    let internal_branch = text.get(internal_at..).unwrap_or("");
    let body_at = internal_branch
        .find("x-changie-body-required\": false")
        .unwrap_or_else(|| std::panic::panic_any("skipBody branch missing"));
    let ticket_at = internal_branch
        .find("\"required\": [\"Ticket\"]")
        .unwrap_or_else(|| std::panic::panic_any("Ticket required missing"));
    assert!(ticket_at > body_at);
}

#[test]
fn string_valued_integers_annotate_not_enforce() {
    // Fixture 5 / negative control 5: lexical pattern plus namespaced
    // numeric annotations; never JSON-Schema numeric min/max on strings.
    let schema = fragment_json_schema(&compiled(PERL_CONFIG));
    let text = schema.schema_text;
    assert!(text.contains("\"pattern\": \"^-?[0-9]+$\""));
    assert!(text.contains("x-changie-min-int\": 1"));
    // The honest note is present.
    assert!(text.contains("annotated, not JSON-Schema-enforced"));
    // And there is no numeric minimum claim on the string property.
    let pr_at = text
        .find("\"PR\"")
        .unwrap_or_else(|| std::panic::panic_any("PR property missing"));
    // The PR property ends where the next property or the custom-object
    // close begins; assert absence over that structural window.
    let pr_close = text
        .get(pr_at..)
        .and_then(|remainder| remainder.find("\n      }"))
        .map(|offset| pr_at + offset)
        .unwrap_or(text.len());
    let pr_block = text.get(pr_at..pr_close).unwrap_or("");
    assert!(!pr_block.contains("\"minimum\""));
    assert!(!pr_block.contains("\"maximum\""));
}

#[test]
fn claim_boundary_and_limitations_are_machine_readable() {
    // Negative control 7: runtime/template semantics must not disappear
    // into a complete-schema claim.
    let schema = fragment_json_schema(&compiled(CARGO_ALLOW_CONFIG));
    assert!(schema.schema_text.contains("x-changie-claim-boundary"));
    assert!(
        schema
            .schema_text
            .contains("x-changie-limitation\": \"template_render_semantics\"")
    );
}

#[test]
fn malformed_config_refuses_a_schema_at_compile() {
    // Fixture 7: malformed/partial config cannot reach the projection
    // with a falsely complete schema — compile_contract fails closed
    // before the schema sees it.
    let malformed = parse_config(
        ChangieSourceDocument::from_bytes(
            ChangieRepoPath::from_repo_relative(".changie.yaml")
                .unwrap_or_else(|err| std::panic::panic_any(err)),
            b"kinds:\n\t- tabbed\n".to_vec(),
            None,
        )
        .unwrap_or_else(|err| std::panic::panic_any(err)),
    );
    assert!(compile_contract(&malformed).is_err());
}

#[test]
fn schema_projection_is_deterministic_and_order_semantic() {
    // Fixture 8: the projection is a pure function of the compiled
    // contract (equal contracts in, identical schema bytes out), and a
    // reordered config document — same effective semantics — compiles to
    // the same semantic schema. The contract retains exact byte-level
    // config identity (#3620), so the digest-bearing identity lines
    // honestly differ for reordered documents; every semantic line is
    // byte-equal. This mirrors the compiled contract's own test.
    let contract = compiled(
        "changesDir: .changes
unreleasedDir: .
kinds:
  - label: Fixed
custom:
  - key: PR
    type: int
    minInt: 1
",
    );
    let first = fragment_json_schema(&contract);
    let second = fragment_json_schema(&contract);
    assert_eq!(first.schema_text, second.schema_text);
    assert_eq!(first.schema_digest, second.schema_digest);

    let reordered = fragment_json_schema(&compiled(
        "unreleasedDir: .
changesDir: .changes
custom:
  - key: PR
    type: int
    minInt: 1
kinds:
  - label: Fixed
",
    ));
    let strip_identity_lines = |text: &str| -> Vec<String> {
        text.lines()
            .filter(|line| {
                !line.contains("contract=")
                    && !line.contains("contract digest")
                    && !line.contains("(contract ")
                    && !line.contains("x-changie-contract-digest")
            })
            .map(str::to_string)
            .collect()
    };
    assert_eq!(
        strip_identity_lines(&first.schema_text),
        strip_identity_lines(&reordered.schema_text),
        "reordered config compiles to the same semantic schema"
    );
}

#[test]
fn config_change_invalidates_schema_and_association_identity() {
    // Fixture 9: a semantic config change changes the schema digest and
    // the association identity — stale editor bindings are detectable.
    let before = compiled(PERL_CONFIG);
    let after = compiled(
        "changesDir: .changes\nunreleasedDir: .\nkinds:\n  - label: Fixed\nprojects:\n  - key: backend\n    label: Backend\ncomponents: [ui, core]\ncustom:\n  - key: PR\n    type: int\n    minInt: 2\n",
    );
    let schema_before = fragment_json_schema(&before);
    let schema_after = fragment_json_schema(&after);
    assert_ne!(schema_before.schema_digest, schema_after.schema_digest);
    let association_before =
        fragment_schema_association(&before, &schema_before, ".changie.yaml", ".changes");
    let association_after =
        fragment_schema_association(&after, &schema_after, ".changie.yaml", ".changes");
    assert_ne!(association_before.schema_id, association_after.schema_id);
    assert_ne!(
        association_before.config_content_identity,
        association_after.config_content_identity
    );
}

#[test]
fn association_pattern_follows_config_discovery_semantics() {
    // The pattern is `<changesDir>/<unreleasedDir>/*.yaml`: direct-child
    // .yaml only, derived from the config's own fields.
    let contract =
        compiled("changesDir: fragments\nunreleasedDir: current\nkinds:\n  - label: Fixed\n");
    let schema = fragment_json_schema(&contract);
    let association =
        fragment_schema_association(&contract, &schema, ".changie.yaml", "fragments/current");
    assert_eq!(
        association.fragment_path_patterns,
        vec!["fragments/current/*.yaml".to_string()]
    );
    // No absolute paths in portable identity (negative control 8).
    assert!(!association.schema_id.contains('/'));
    assert!(!association.schema_id.contains('\\'));
    assert!(!association.schema_id.contains("F:"));
    assert!(!association.schema_id.contains('\\'));
    let schema_text = schema.schema_text;
    assert!(!schema_text.contains("F:\\\\") && !schema_text.contains("/home/"));
}

#[test]
fn expressible_rules_map_to_canonical_sensor_identities() {
    // Diagnostic parity: every rule id the schema claims is a real
    // sensor rule; Rust-only constraints do not disappear (they stay in
    // the sensor, absent from this list).
    let schema = fragment_json_schema(&compiled(PERL_CONFIG));
    let rule_ids = schema.expressible_rule_ids.clone();
    for rule in &rule_ids {
        assert!(
            rule.starts_with("changie.fragment."),
            "{rule} is not a canonical sensor rule id"
        );
    }
    // kind_missing is Rust-authoritative here (requiredness is expressed
    // via the schema's own required list, not a rule annotation).
    assert!(!rule_ids.contains(&"changie.fragment.kind_missing"));
}

#[test]
fn schema_is_valid_json() {
    // Independent-parse fixture (fixture 10's Rust half; the CI smoke
    // runs the independent validator).
    let schema = fragment_json_schema(&compiled(PERL_CONFIG));
    // Parse without a JSON dependency: structural checks over the text.
    assert!(schema.schema_text.starts_with('{'));
    assert!(schema.schema_text.ends_with(
        "}
"
    ));
    assert!(
        schema
            .schema_text
            .contains("\"$schema\": \"https://json-schema.org/draft/2020-12/schema\"")
    );
    assert!(schema.schema_text.contains("\"properties\": {"));
}
