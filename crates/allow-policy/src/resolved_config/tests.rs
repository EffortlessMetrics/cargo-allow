use std::fs;
use std::path::{Path, PathBuf};

use super::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn resolved_cargo_allow_config_preserves_conventional_selection() -> TestResult {
    let fixture = Fixture::new("conventional")?;
    fixture.write("policy/allow.toml", valid_policy())?;

    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:conventional")?;

    ensure_eq(
        resolved.status,
        ConfigResolutionStatusV1::Complete,
        "status",
    )?;
    ensure_eq(
        resolved.selection_source,
        Some(ConfigCandidateSourceV1::ConventionalPath),
        "selection source",
    )?;
    ensure_eq(
        resolved.precedence_tier,
        Some(ConfigPrecedenceTierV1::DiscoveryFallback),
        "precedence",
    )?;
    ensure_eq(
        resolved
            .selected_policy
            .as_ref()
            .map(|policy| policy.path.path.as_str()),
        Some("policy/allow.toml"),
        "selected path",
    )?;
    ensure(
        resolved
            .selected_policy
            .as_ref()
            .and_then(|policy| policy.digest.as_deref())
            .is_some_and(|digest| digest.starts_with("sha256:v1:")),
        "selected policy digest should be content-addressed",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_retains_malformed_federation_fallback() -> TestResult {
    let fixture = Fixture::new("malformed-federation")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    fixture.write(".allow/config.toml", "[[ledgers]\ninvalid = true\n")?;

    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:malformed")?;

    ensure_eq(resolved.status, ConfigResolutionStatusV1::Partial, "status")?;
    ensure(resolved.fallback.considered, "fallback should be explicit")?;
    ensure(
        resolved.fallback.selected,
        "fallback winner should be explicit",
    )?;
    ensure_eq(
        resolved.federation.posture,
        ConfigFederationPostureV1::Unreadable,
        "federation posture",
    )?;
    ensure(
        resolved
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == "invalid_config"),
        "higher-order error should remain typed",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_keeps_cli_winner_when_federation_is_malformed() -> TestResult {
    let fixture = Fixture::new("cli-malformed-federation")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    fixture.write("policy/explicit.toml", valid_policy())?;
    fixture.write(".allow/config.toml", "[[ledgers]\ninvalid = true\n")?;

    let resolved = resolve_cargo_allow_config_v1(
        fixture.path(),
        Some(Path::new("policy/explicit.toml")),
        "subject:cli-malformed-federation",
    )?;

    ensure_eq(resolved.status, ConfigResolutionStatusV1::Partial, "status")?;
    ensure_eq(
        resolved.selection_source,
        Some(ConfigCandidateSourceV1::CliOverride),
        "selection source",
    )?;
    ensure_eq(
        resolved.precedence_tier,
        Some(ConfigPrecedenceTierV1::CliOverride),
        "precedence",
    )?;
    ensure_eq(
        resolved
            .selected_policy
            .as_ref()
            .map(|policy| policy.path.path.as_str()),
        Some("policy/explicit.toml"),
        "selected path",
    )?;
    ensure(
        !resolved.fallback.considered && !resolved.fallback.selected,
        "an explicit CLI selection must not be rewritten as discovery fallback",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_marks_invalid_discovery_fallback_invalid() -> TestResult {
    let fixture = Fixture::new("invalid-discovery-fallback")?;
    fixture.write(
        "policy/allow.toml",
        &valid_policy().replace("status = \"active\"", "status = \"bogus\""),
    )?;
    fixture.write(".allow/config.toml", "[[ledgers]\ninvalid = true\n")?;

    let resolved =
        resolve_cargo_allow_config_v1(fixture.path(), None, "subject:invalid-discovery-fallback")?;

    ensure_eq(resolved.status, ConfigResolutionStatusV1::Invalid, "status")?;
    ensure(
        resolved.fallback.selected,
        "discovery fallback selection should remain visible",
    )?;
    ensure(
        resolved
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == "invalid_policy"),
        "invalid fallback policy diagnostic should remain typed",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_preserves_invalid_policy_ahead_of_ambiguity() -> TestResult {
    let fixture = Fixture::new("invalid-policy-and-ambiguous-federation")?;
    fixture.write(
        "policy/allow.toml",
        &valid_policy().replace("status = \"active\"", "status = \"bogus\""),
    )?;
    fixture.write("policy/other.toml", valid_policy())?;
    fixture.write(
        ".allow/config.toml",
        r#"schema_version = "1.0"

[[ledgers]]
id = "first"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
mode = "blocking"
priority = 10

[[ledgers]]
id = "second"
path = "policy/other.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
mode = "blocking"
priority = 10
"#,
    )?;

    let resolved = resolve_cargo_allow_config_v1(
        fixture.path(),
        None,
        "subject:invalid-policy-and-ambiguous-federation",
    )?;

    ensure_eq(
        resolved.status,
        ConfigResolutionStatusV1::Invalid,
        "invalid selected policy must take precedence over registry ambiguity",
    )?;
    ensure(
        resolved
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == "invalid_policy"),
        "invalid selected policy diagnostic should remain typed",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_keeps_federation_and_conventional_candidates_distinct() -> TestResult
{
    let fixture = Fixture::new("federation-winner")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    fixture.write("policy/other.toml", valid_policy())?;
    fixture.write(
        ".allow/config.toml",
        r#"schema_version = "1.0"

[[ledgers]]
id = "source-policy"
path = "policy/other.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
mode = "blocking"
priority = 10
"#,
    )?;

    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:federation")?;

    ensure_eq(
        resolved.status,
        ConfigResolutionStatusV1::Complete,
        "status",
    )?;
    ensure_eq(
        resolved.selection_source,
        Some(ConfigCandidateSourceV1::FederationRegistry),
        "selection source",
    )?;
    ensure_eq(
        resolved
            .selected_policy
            .as_ref()
            .map(|policy| policy.path.path.as_str()),
        Some("policy/other.toml"),
        "selected path",
    )?;
    ensure(
        resolved.candidates.iter().any(|candidate| {
            candidate.source == ConfigCandidateSourceV1::ConventionalPath
                && candidate.path.as_ref().map(|path| path.path.as_str())
                    == Some("policy/allow.toml")
                && candidate.disposition == ConfigCandidateDispositionV1::Available
        }),
        "conventional candidate should remain visible",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_is_relocation_stable_and_omits_private_roots() -> TestResult {
    let first = Fixture::new("relocation-first")?;
    let second = Fixture::new("relocation-second")?;
    first.write("policy/allow.toml", valid_policy())?;
    second.write("policy/allow.toml", valid_policy())?;

    let first_resolved = resolve_cargo_allow_config_v1(first.path(), None, "subject:same")?;
    let second_resolved = resolve_cargo_allow_config_v1(second.path(), None, "subject:same")?;
    let first_json = serde_json::to_string(&first_resolved)?;
    let second_json = serde_json::to_string(&second_resolved)?;

    ensure_eq(first_json, second_json, "portable resolution")?;
    ensure(
        !serde_json::to_string(&first_resolved)?.contains(&first.path().display().to_string()),
        "portable result should not contain the first private root",
    )?;
    ensure(
        !serde_json::to_string(&second_resolved)?.contains(&second.path().display().to_string()),
        "portable result should not contain the second private root",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_distinguishes_no_policy() -> TestResult {
    let fixture = Fixture::new("no-policy")?;

    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:none")?;

    ensure_eq(
        resolved.status,
        ConfigResolutionStatusV1::NoPolicy,
        "status",
    )?;
    ensure(
        resolved.selected_policy.is_none(),
        "policy should be absent",
    )?;
    ensure(
        resolved.fallback.considered,
        "legacy fallback should be recorded",
    )?;
    ensure(
        !resolved.fallback.selected,
        "fallback should not invent a policy",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_honors_explicit_cli_over_federation() -> TestResult {
    let fixture = Fixture::new("explicit")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    fixture.write("policy/explicit.toml", valid_policy())?;
    fixture.write(
        ".allow/config.toml",
        r#"schema_version = "1.0"

[[ledgers]]
id = "source-policy"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
mode = "blocking"
priority = 10
"#,
    )?;

    let resolved = resolve_cargo_allow_config_v1(
        fixture.path(),
        Some(Path::new("policy/explicit.toml")),
        "subject:explicit",
    )?;

    ensure_eq(
        resolved.status,
        ConfigResolutionStatusV1::Complete,
        "status",
    )?;
    ensure_eq(
        resolved.selection_source,
        Some(ConfigCandidateSourceV1::CliOverride),
        "selection source",
    )?;
    ensure_eq(
        resolved.precedence_tier,
        Some(ConfigPrecedenceTierV1::CliOverride),
        "precedence",
    )?;
    ensure_eq(
        resolved
            .selected_policy
            .as_ref()
            .map(|policy| policy.path.path.as_str()),
        Some("policy/explicit.toml"),
        "selected path",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_preserves_package_over_workspace_metadata() -> TestResult {
    let fixture = Fixture::new("metadata")?;
    fixture.write("policy/package.toml", valid_policy())?;
    fixture.write("policy/workspace.toml", valid_policy())?;
    fixture.write(
        "Cargo.toml",
        r#"[package.metadata.cargo-allow]
config = "policy/package.toml"

[workspace.metadata.cargo-allow]
config = "policy/workspace.toml"
"#,
    )?;

    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:metadata")?;

    ensure_eq(
        resolved.selection_source,
        Some(ConfigCandidateSourceV1::PackageMetadata),
        "selection source",
    )?;
    ensure_eq(
        resolved
            .selected_policy
            .as_ref()
            .map(|policy| policy.path.path.as_str()),
        Some("policy/package.toml"),
        "selected path",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_preserves_skipped_package_metadata_provenance() -> TestResult {
    let fixture = Fixture::new("skipped-metadata")?;
    fixture.write(
        "Cargo.toml",
        r#"[package.metadata.cargo-allow]
config = "policy/missing.toml"
"#,
    )?;

    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:skipped-metadata")?;

    ensure_eq(resolved.status, ConfigResolutionStatusV1::Invalid, "status")?;
    ensure(
        resolved.candidates.iter().any(|candidate| {
            candidate.source == ConfigCandidateSourceV1::PackageMetadata
                && candidate.path.as_ref().map(|path| path.path.as_str())
                    == Some("policy/missing.toml")
                && candidate.disposition == ConfigCandidateDispositionV1::Skipped
        }),
        "skipped metadata should retain its typed package source",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_preserves_malformed_manifest_provenance() -> TestResult {
    let fixture = Fixture::new("malformed-metadata")?;
    let private_path = "/home/alice/private/customer-policy.toml";
    fixture.write(
        "Cargo.toml",
        &format!("[workspace\nprivate = \"{private_path}\"\n"),
    )?;
    fixture.write("policy/allow.toml", valid_policy())?;

    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:malformed")?;
    let rendered = serde_json::to_string(&resolved)?;

    ensure(
        resolved.candidates.iter().any(|candidate| {
            candidate.source == ConfigCandidateSourceV1::CargoMetadata
                && candidate.path.as_ref().map(|path| path.path.as_str()) == Some("Cargo.toml")
                && candidate.disposition == ConfigCandidateDispositionV1::Skipped
        }),
        "malformed manifest should retain generic Cargo metadata provenance",
    )?;
    ensure(
        resolved.candidates.iter().any(|candidate| {
            candidate.source == ConfigCandidateSourceV1::CargoMetadata
                && candidate.reason.as_deref() == Some("cargo-allow metadata could not be parsed")
        }),
        "malformed manifest should use a bounded generic parse reason",
    )?;
    ensure(
        !rendered.contains(private_path),
        "malformed manifest source text must not leak through the resolved artifact",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_redacts_malformed_selected_policy_source() -> TestResult {
    let fixture = Fixture::new("malformed-selected-policy")?;
    let private_path = "/home/alice/private/selected-policy.toml";
    fixture.write(
        "policy/explicit.toml",
        &format!("schema_version = [\"0.1\"\nprivate = \"{private_path}\"\n"),
    )?;

    let resolved = resolve_cargo_allow_config_v1(
        fixture.path(),
        Some(Path::new("policy/explicit.toml")),
        "subject:malformed-selected-policy",
    )?;
    let rendered = serde_json::to_string(&resolved)?;

    ensure_eq(resolved.status, ConfigResolutionStatusV1::Invalid, "status")?;
    ensure(
        !rendered.contains(private_path),
        "selected policy parser source text must not leak through diagnostics",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_redacts_malformed_federation_source() -> TestResult {
    let fixture = Fixture::new("malformed-federation-private-source")?;
    let private_path = "/home/alice/private/federation-policy.toml";
    fixture.write("policy/allow.toml", valid_policy())?;
    fixture.write(
        ".allow/config.toml",
        &format!("[[ledgers]\nprivate = \"{private_path}\"\n"),
    )?;

    let resolved = resolve_cargo_allow_config_v1(
        fixture.path(),
        None,
        "subject:malformed-federation-private-source",
    )?;
    let rendered = serde_json::to_string(&resolved)?;

    ensure_eq(resolved.status, ConfigResolutionStatusV1::Partial, "status")?;
    ensure(
        !rendered.contains(private_path),
        "federation parser source text must not leak through diagnostics",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_rejects_path_shaped_federation_identity() -> TestResult {
    let fixture = Fixture::new("path-shaped-federation-id")?;
    let private_id = "/home/alice/private/customer";
    fixture.write("policy/allow.toml", valid_policy())?;
    fixture.write("policy/federated.toml", valid_policy())?;
    fixture.write(
        ".allow/config.toml",
        &format!(
            r#"schema_version = "1.0"

[[ledgers]]
id = "{private_id}"
path = "policy/federated.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
priority = 10
"#
        ),
    )?;

    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:path-shaped-id")?;
    let rendered = serde_json::to_string(&resolved)?;

    ensure_eq(resolved.status, ConfigResolutionStatusV1::Partial, "status")?;
    ensure(
        resolved.federation.configured_ledgers.is_empty()
            && resolved.federation.selected_for_source_exception,
        "invalid federation identity must be omitted without changing current selection",
    )?;
    ensure(
        resolved.selected_policy.as_ref().is_some_and(|policy| {
            policy.path.path == "policy/federated.toml" && policy.digest.is_some()
        }),
        "projection hardening must preserve the policy selected by current federation behavior",
    )?;
    ensure(
        resolved
            .federation
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic == "invalid_ledger_id"),
        "invalid federation identity should retain a typed diagnostic",
    )?;
    ensure(
        !rendered.contains(private_id),
        "path-shaped federation identity must not leak through the artifact",
    )?;
    Ok(())
}

#[test]
fn portable_ledger_identity_grammar_covers_contract_boundaries() -> TestResult {
    let max = "a".repeat(1_024);
    let oversized = "a".repeat(1_025);
    for allowed in ["a", "Team_1.release@owner+next", max.as_str()] {
        ensure(
            is_portable_ledger_identity(allowed),
            &format!("portable ledger identity should be accepted: {allowed}"),
        )?;
    }
    for rejected in [
        "",
        " ",
        "team/source",
        r"team\source",
        "C:private",
        "équipe",
        oversized.as_str(),
    ] {
        ensure(
            !is_portable_ledger_identity(rejected),
            &format!("non-portable ledger identity should be rejected: {rejected}"),
        )?;
    }
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_preserves_unsafe_metadata_source_provenance() -> TestResult {
    let fixture = Fixture::new("unsafe-metadata")?;
    fixture.write(
        "Cargo.toml",
        r#"[package.metadata.cargo-allow]
config = "../outside.toml"
"#,
    )?;

    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:unsafe-metadata")?;

    ensure(
        resolved.candidates.iter().any(|candidate| {
            candidate.source == ConfigCandidateSourceV1::PackageMetadata
                && candidate.path.as_ref().map(|path| path.path.as_str()) == Some("Cargo.toml")
                && candidate.disposition == ConfigCandidateDispositionV1::Skipped
        }),
        "unsafe metadata should retain its typed package source",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_retains_foreign_candidate_rejection() -> TestResult {
    let fixture = Fixture::new("foreign")?;
    fixture.write(
        "policy/allow.toml",
        "schema_version = \"1\"\npolicy = \"another-tool\"\n",
    )?;

    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:foreign")?;

    ensure_eq(resolved.status, ConfigResolutionStatusV1::Invalid, "status")?;
    ensure(
        resolved.candidates.iter().any(|candidate| {
            candidate.path.as_ref().map(|path| path.path.as_str()) == Some("policy/allow.toml")
                && candidate.disposition == ConfigCandidateDispositionV1::Skipped
        }),
        "foreign candidate should remain visible",
    )?;
    ensure(
        resolved.selected_policy.is_none(),
        "foreign policy must not be selected",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_marks_ambiguous_federation_non_clean() -> TestResult {
    let fixture = Fixture::new("ambiguous-federation")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    fixture.write("policy/other.toml", valid_policy())?;
    fixture.write(
        ".allow/config.toml",
        r#"schema_version = "1.0"

[[ledgers]]
id = "first"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
mode = "blocking"
priority = 10

[[ledgers]]
id = "second"
path = "policy/other.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
mode = "blocking"
priority = 10
"#,
    )?;

    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:ambiguous")?;

    ensure_eq(
        resolved.status,
        ConfigResolutionStatusV1::Ambiguous,
        "status",
    )?;
    ensure_eq(
        resolved.federation.posture,
        ConfigFederationPostureV1::Invalid,
        "federation posture",
    )?;
    ensure(
        resolved
            .federation
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic == "duplicate_canonical_lane"),
        "ambiguity diagnostic should remain typed",
    )?;
    ensure(resolved.fallback.considered, "fallback should be explicit")?;
    ensure(
        resolved.fallback.selected,
        "conventional fallback winner should be explicit",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_keeps_cli_winner_over_ambiguous_federation() -> TestResult {
    let fixture = Fixture::new("cli-over-ambiguous-federation")?;
    fixture.write("policy/explicit.toml", valid_policy())?;
    fixture.write("policy/first.toml", valid_policy())?;
    fixture.write("policy/second.toml", valid_policy())?;
    fixture.write(
        ".allow/config.toml",
        r#"schema_version = "1.0"

[[ledgers]]
id = "first"
path = "policy/first.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
mode = "blocking"
priority = 10

[[ledgers]]
id = "second"
path = "policy/second.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
mode = "blocking"
priority = 10
"#,
    )?;

    let resolved = resolve_cargo_allow_config_v1(
        fixture.path(),
        Some(Path::new("policy/explicit.toml")),
        "subject:cli-ambiguous",
    )?;

    ensure_eq(resolved.status, ConfigResolutionStatusV1::Partial, "status")?;
    ensure_eq(
        resolved.selection_source,
        Some(ConfigCandidateSourceV1::CliOverride),
        "selection source",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_does_not_launder_spec_lane_conflict_into_core_ambiguity()
-> TestResult {
    let fixture = Fixture::new("spec-only-ambiguity")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    fixture.write("docs/first.toml", "schema_version = \"1\"\n")?;
    fixture.write("docs/second.toml", "schema_version = \"1\"\n")?;
    fixture.write(
        ".allow/config.toml",
        r#"schema_version = "1.0"

[[ledgers]]
id = "first"
path = "docs/first.toml"
dialect = "cargo-allow-doc-artifacts"
role = "canonical"
lanes = ["spec-system"]
mode = "blocking"
priority = 10

[[ledgers]]
id = "second"
path = "docs/second.toml"
dialect = "cargo-allow-doc-artifacts"
role = "canonical"
lanes = ["spec-system"]
mode = "blocking"
priority = 10
"#,
    )?;

    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:spec-conflict")?;

    ensure_eq(resolved.status, ConfigResolutionStatusV1::Partial, "status")?;
    ensure_eq(
        resolved.selection_source,
        Some(ConfigCandidateSourceV1::ConventionalPath),
        "selection source",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_represents_ancestor_winner_without_private_path() -> TestResult {
    let fixture = Fixture::new("ancestor-winner")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    fs::create_dir_all(fixture.path().join("nested/source"))?;
    let requested = fixture.path().join("nested/source");

    let resolved = resolve_cargo_allow_config_v1(&requested, None, "subject:ancestor")?;
    let rendered = serde_json::to_string(&resolved)?;

    ensure_eq(
        resolved.status,
        ConfigResolutionStatusV1::Complete,
        "status",
    )?;
    ensure_eq(
        resolved.selection_source,
        Some(ConfigCandidateSourceV1::ConventionalPath),
        "selection source",
    )?;
    ensure(
        resolved.candidates.iter().any(|candidate| {
            candidate.source == ConfigCandidateSourceV1::ConventionalPath
                && candidate.path.as_ref().is_some_and(|path| {
                    path.anchor == ConfigPathAnchorV1::DiscoveryAncestor
                        && path.ancestor_depth == 2
                        && path.path == "policy/allow.toml"
                })
                && candidate.disposition == ConfigCandidateDispositionV1::Selected
        }),
        "ancestor winner should retain portable anchor provenance",
    )?;
    ensure(
        resolved.selected_policy.as_ref().is_some_and(|policy| {
            policy.path.anchor == ConfigPathAnchorV1::DiscoveryAncestor
                && policy.path.ancestor_depth == 2
                && policy.path.path == "policy/allow.toml"
                && policy.digest.is_some()
        }),
        "ancestor winner should retain policy identity and digest",
    )?;
    ensure(
        !rendered.contains(&fixture.path().display().to_string()),
        "ancestor winner should not leak the private fixture root",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_retains_ancestor_metadata_provenance() -> TestResult {
    let fixture = Fixture::new("ancestor-metadata")?;
    fixture.write("policy/package.toml", valid_policy())?;
    fixture.write(
        "Cargo.toml",
        r#"[package.metadata.cargo-allow]
config = "policy/package.toml"
"#,
    )?;
    fs::create_dir_all(fixture.path().join("nested/source"))?;
    let requested = fixture.path().join("nested/source");

    let resolved = resolve_cargo_allow_config_v1(&requested, None, "subject:ancestor-metadata")?;

    ensure_eq(
        resolved.selection_source,
        Some(ConfigCandidateSourceV1::PackageMetadata),
        "selection source",
    )?;
    ensure(
        resolved.selected_policy.as_ref().is_some_and(|policy| {
            policy.path.anchor == ConfigPathAnchorV1::DiscoveryAncestor
                && policy.path.ancestor_depth == 2
                && policy.path.path == "policy/package.toml"
                && policy.digest.is_some()
        }),
        "ancestor metadata winner should retain anchored path and digest",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_redacts_missing_ancestor_metadata_path() -> TestResult {
    let fixture = Fixture::new("ancestor-metadata-missing")?;
    fixture.write(
        "Cargo.toml",
        r#"[package.metadata.cargo-allow]
config = "policy/missing.toml"
"#,
    )?;
    fs::create_dir_all(fixture.path().join("nested/source"))?;
    let requested = fixture.path().join("nested/source");

    let resolved =
        resolve_cargo_allow_config_v1(&requested, None, "subject:ancestor-metadata-missing")?;
    let rendered = serde_json::to_string(&resolved)?;

    ensure(
        !rendered.contains(&fixture.path().display().to_string()),
        "missing ancestor metadata diagnostics should not leak the private fixture root",
    )?;
    ensure(
        resolved.candidates.iter().any(|candidate| {
            candidate.source == ConfigCandidateSourceV1::PackageMetadata
                && candidate.path.as_ref().is_some_and(|path| {
                    path.anchor == ConfigPathAnchorV1::DiscoveryAncestor
                        && path.ancestor_depth == 2
                        && path.path == "policy/missing.toml"
                })
        }),
        "missing ancestor metadata should retain structured portable provenance",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_keeps_child_policy_ahead_of_parent() -> TestResult {
    let fixture = Fixture::new("child-before-parent")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    fixture.write("nested/source/policy/allow.toml", valid_policy())?;
    let requested = fixture.path().join("nested/source");

    let resolved = resolve_cargo_allow_config_v1(&requested, None, "subject:child")?;

    ensure(
        resolved.selected_policy.as_ref().is_some_and(|policy| {
            policy.path.anchor == ConfigPathAnchorV1::ResolvedRepositoryRoot
                && policy.path.ancestor_depth == 0
                && policy.path.path == "policy/allow.toml"
        }),
        "nearest child policy should preserve current discovery precedence",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_keeps_root_valued_cli_path_schema_safe() -> TestResult {
    let fixture = Fixture::new("root-valued-cli")?;

    let resolved =
        resolve_cargo_allow_config_v1(fixture.path(), Some(Path::new(".")), "subject:root")?;

    ensure_eq(
        resolved
            .explicit_cli_values
            .iter()
            .map(|path| path.path.as_str())
            .collect::<Vec<_>>(),
        vec!["."],
        "root-valued explicit path",
    )?;
    ensure_eq(
        resolved
            .selected_policy
            .as_ref()
            .map(|policy| policy.path.path.as_str()),
        Some("."),
        "root-valued selected policy path",
    )?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn resolved_cargo_allow_config_rejects_in_root_symlink_to_external_policy() -> TestResult {
    let fixture = Fixture::new("symlink-root")?;
    let external = Fixture::new("symlink-external")?;
    external.write("outside.toml", valid_policy())?;
    fs::create_dir_all(fixture.path().join("policy"))?;
    std::os::unix::fs::symlink(
        external.path().join("outside.toml"),
        fixture.path().join("policy/allow.toml"),
    )?;
    fixture.write(".cargo/allow.toml", valid_policy())?;

    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:symlink")?;
    let rendered = serde_json::to_string(&resolved)?;

    ensure_eq(
        resolved.status,
        ConfigResolutionStatusV1::Complete,
        "status",
    )?;
    ensure(
        resolved
            .selected_policy
            .as_ref()
            .is_some_and(|policy| policy.path.path == ".cargo/allow.toml"),
        "safe lower-precedence policy should win without probing external bytes",
    )?;
    ensure(
        !rendered.contains(&external.path().display().to_string()),
        "external symlink target should not enter the portable projection",
    )?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn resolved_cargo_allow_config_rejects_dangling_discovery_parent_component() -> TestResult {
    let fixture = Fixture::new("dangling-discovery-parent")?;
    std::os::unix::fs::symlink("missing-policy", fixture.path().join("policy"))?;
    fixture.write(".cargo/allow.toml", valid_policy())?;

    let resolved =
        resolve_cargo_allow_config_v1(fixture.path(), None, "subject:dangling-discovery-parent")?;

    ensure(
        resolved
            .selected_policy
            .as_ref()
            .is_some_and(|policy| policy.path.path == ".cargo/allow.toml"),
        "safe fallback should win after a dangling discovery parent is rejected",
    )?;
    ensure(
        resolved.candidates.iter().any(|candidate| {
            candidate.source == ConfigCandidateSourceV1::ConventionalPath
                && candidate.disposition == ConfigCandidateDispositionV1::Skipped
                && candidate
                    .path
                    .as_ref()
                    .is_some_and(|path| path.path == "policy/allow.toml")
                && candidate
                    .reason
                    .as_deref()
                    .is_some_and(|reason| reason.contains("unresolved symlink component"))
        }),
        "dangling discovery parent must remain visible as a rejected candidate",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_rejects_non_regular_fallback_before_reading() -> TestResult {
    let fixture = Fixture::new("non-regular-fallback")?;
    fixture.write("policy/cli.toml", valid_policy())?;
    fs::create_dir_all(fixture.path().join("policy/allow.toml"))?;

    let resolved = resolve_cargo_allow_config_v1(
        fixture.path(),
        Some(Path::new("policy/cli.toml")),
        "subject:non-regular-fallback",
    )?;

    ensure(
        resolved
            .selected_policy
            .as_ref()
            .is_some_and(|policy| policy.path.path == "policy/cli.toml"),
        "explicit regular-file policy should remain selected",
    )?;
    ensure(
        resolved.candidates.iter().any(|candidate| {
            candidate.source == ConfigCandidateSourceV1::ConventionalPath
                && candidate
                    .path
                    .as_ref()
                    .is_some_and(|path| path.path == "policy/allow.toml")
                && candidate
                    .reason
                    .as_deref()
                    .is_some_and(|reason| reason.contains("not a regular file"))
        }),
        "non-regular fallback must be rejected before the read boundary",
    )?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn resolved_cargo_allow_config_accepts_internal_symlink_with_lexical_identity() -> TestResult {
    let fixture = Fixture::new("internal-symlink")?;
    fixture.write("policy/actual.toml", valid_policy())?;
    std::os::unix::fs::symlink("actual.toml", fixture.path().join("policy/allow.toml"))?;

    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:internal-link")?;

    ensure_eq(
        resolved.status,
        ConfigResolutionStatusV1::Complete,
        "status",
    )?;
    ensure(
        resolved.selected_policy.as_ref().is_some_and(|policy| {
            policy.path.anchor == ConfigPathAnchorV1::ResolvedRepositoryRoot
                && policy.path.path == "policy/allow.toml"
                && policy.digest.is_some()
        }),
        "internal symlink should preserve its lexical configured identity",
    )?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn resolved_cargo_allow_config_rejects_external_federation_registry_symlink() -> TestResult {
    let fixture = Fixture::new("external-federation-link")?;
    let external = Fixture::new("external-federation-target")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    external.write(
        "config.toml",
        r#"schema_version = "1.0"

[[ledgers]]
id = "external-policy"
path = "policy/outside.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
mode = "blocking"
priority = 10
"#,
    )?;
    fs::create_dir_all(fixture.path().join(".allow"))?;
    std::os::unix::fs::symlink(
        external.path().join("config.toml"),
        fixture.path().join(".allow/config.toml"),
    )?;

    let resolved =
        resolve_cargo_allow_config_v1(fixture.path(), None, "subject:external-federation-link")?;
    let rendered = serde_json::to_string(&resolved)?;

    ensure_eq(
        resolved.federation.posture,
        ConfigFederationPostureV1::Unreadable,
        "federation posture",
    )?;
    ensure(
        resolved.federation.configured_ledgers.is_empty(),
        "external registry contents must not be observed",
    )?;
    ensure(
        resolved
            .selected_policy
            .as_ref()
            .is_some_and(|policy| policy.path.path == "policy/allow.toml"),
        "safe conventional policy should remain the bounded fallback",
    )?;
    ensure(
        !rendered.contains("external-policy")
            && !rendered.contains(&external.path().display().to_string()),
        "external registry identity and path must not enter the projection",
    )?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn resolved_cargo_allow_config_accepts_internal_federation_registry_symlink() -> TestResult {
    let fixture = Fixture::new("internal-federation-link")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    fixture.write(
        ".allow/actual.toml",
        r#"schema_version = "1.0"

[[ledgers]]
id = "internal-policy"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
mode = "blocking"
priority = 10
"#,
    )?;
    std::os::unix::fs::symlink("actual.toml", fixture.path().join(".allow/config.toml"))?;

    let resolved =
        resolve_cargo_allow_config_v1(fixture.path(), None, "subject:internal-federation-link")?;

    ensure_eq(
        resolved.federation.posture,
        ConfigFederationPostureV1::Valid,
        "federation posture",
    )?;
    ensure_eq(
        resolved.federation.configured_ledgers,
        vec!["internal-policy".to_string()],
        "configured ledgers",
    )?;
    ensure(
        resolved.federation.selected_for_source_exception,
        "internal registry link should remain selectable",
    )?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn resolved_cargo_allow_config_rejects_dangling_federation_registry_symlink() -> TestResult {
    let fixture = Fixture::new("dangling-federation-link")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    fs::create_dir_all(fixture.path().join(".allow"))?;
    std::os::unix::fs::symlink("missing.toml", fixture.path().join(".allow/config.toml"))?;

    let resolved =
        resolve_cargo_allow_config_v1(fixture.path(), None, "subject:dangling-federation-link")?;

    ensure_eq(
        resolved.federation.posture,
        ConfigFederationPostureV1::Unreadable,
        "federation posture",
    )?;
    ensure(
        resolved.federation.configured_ledgers.is_empty(),
        "dangling registry link must not produce configured ledgers",
    )?;
    ensure(
        resolved
            .selected_policy
            .as_ref()
            .is_some_and(|policy| policy.path.path == "policy/allow.toml"),
        "safe conventional policy should remain the bounded fallback",
    )?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn resolved_cargo_allow_config_rejects_dangling_federation_parent_component() -> TestResult {
    let fixture = Fixture::new("dangling-federation-parent")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    std::os::unix::fs::symlink("missing-allow", fixture.path().join(".allow"))?;

    let resolved =
        resolve_cargo_allow_config_v1(fixture.path(), None, "subject:dangling-federation-parent")?;

    ensure_eq(
        resolved.federation.posture,
        ConfigFederationPostureV1::Unreadable,
        "federation posture",
    )?;
    ensure(
        resolved.federation.configured_ledgers.is_empty(),
        "dangling federation parent must not produce configured ledgers",
    )?;
    ensure(
        resolved
            .selected_policy
            .as_ref()
            .is_some_and(|policy| policy.path.path == "policy/allow.toml"),
        "safe conventional policy should remain the bounded fallback",
    )?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn resolved_cargo_allow_config_accepts_absolute_cli_through_directory_alias() -> TestResult {
    let fixture = Fixture::new("aliased-root")?;
    let alias_holder = Fixture::new("alias-holder")?;
    fixture.write("policy/explicit.toml", valid_policy())?;
    let alias = alias_holder.path().join("repo-alias");
    std::os::unix::fs::symlink(fixture.path(), &alias)?;
    let cli_path = alias.join("policy/explicit.toml");

    let resolved =
        resolve_cargo_allow_config_v1(fixture.path(), Some(&cli_path), "subject:directory-alias")?;

    ensure_eq(
        resolved
            .selected_policy
            .as_ref()
            .map(|policy| policy.path.path.as_str()),
        Some("policy/explicit.toml"),
        "canonical fallback should recover the portable in-root path",
    )?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn resolved_cargo_allow_config_rejects_dangling_explicit_symlink() -> TestResult {
    let fixture = Fixture::new("dangling-symlink")?;
    fs::create_dir_all(fixture.path().join("policy"))?;
    std::os::unix::fs::symlink("missing.toml", fixture.path().join("policy/dangling.toml"))?;

    let resolved = resolve_cargo_allow_config_v1(
        fixture.path(),
        Some(Path::new("policy/dangling.toml")),
        "subject:dangling-link",
    )?;

    ensure_eq(
        resolved.status,
        ConfigResolutionStatusV1::Unsupported,
        "status",
    )?;
    ensure(
        resolved.selected_policy.is_none(),
        "dangling link must not become a selected policy",
    )?;
    ensure(
        resolved.candidates.iter().any(|candidate| {
            candidate.source == ConfigCandidateSourceV1::CliOverride
                && candidate.disposition == ConfigCandidateDispositionV1::Selected
        }),
        "dangling leaf candidate should retain its existing selected disposition",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_preserves_missing_explicit_policy_selection() -> TestResult {
    let fixture = Fixture::new("missing-cli-policy")?;

    let resolved = resolve_cargo_allow_config_v1(
        fixture.path(),
        Some(Path::new("missing/policy.toml")),
        "subject:missing-cli-policy",
    )?;

    ensure_eq(resolved.status, ConfigResolutionStatusV1::Invalid, "status")?;
    ensure(
        resolved.selected_policy.as_ref().is_some_and(|policy| {
            policy.path.path == "missing/policy.toml" && policy.digest.is_none()
        }),
        "ordinary missing CLI policy should retain its selected portable identity",
    )?;
    ensure(
        resolved.candidates.iter().any(|candidate| {
            candidate.source == ConfigCandidateSourceV1::CliOverride
                && candidate.disposition == ConfigCandidateDispositionV1::Selected
        }),
        "ordinary missing CLI policy should retain selected disposition",
    )?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn resolved_cargo_allow_config_rejects_explicit_policy_beneath_dangling_parent() -> TestResult {
    let fixture = Fixture::new("dangling-cli-parent")?;
    std::os::unix::fs::symlink("missing-policy", fixture.path().join("policy"))?;

    let resolved = resolve_cargo_allow_config_v1(
        fixture.path(),
        Some(Path::new("policy/dangling.toml")),
        "subject:dangling-cli-parent",
    )?;

    ensure_eq(
        resolved.status,
        ConfigResolutionStatusV1::Unsupported,
        "status",
    )?;
    ensure(
        resolved.selected_policy.is_none(),
        "a policy beneath a dangling parent must not become selected",
    )?;
    ensure(
        !resolved.candidates.iter().any(|candidate| {
            candidate.source == ConfigCandidateSourceV1::CliOverride
                && candidate.disposition == ConfigCandidateDispositionV1::Selected
        }),
        "a CLI policy beneath a dangling parent must not have selected disposition",
    )?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn resolved_cargo_allow_config_rejects_non_regular_explicit_policy() -> TestResult {
    if std::env::var_os("CARGO_ALLOW_FIFO_CHILD").is_some() {
        let fixture = Fixture::new("fifo-cli-policy-child")?;
        std::process::Command::new("mkfifo")
            .arg(fixture.path().join("policy.fifo"))
            .status()?
            .success()
            .then_some(())
            .ok_or("mkfifo failed")?;
        let resolved = resolve_cargo_allow_config_v1(
            fixture.path(),
            Some(Path::new("policy.fifo")),
            "subject:fifo-cli-policy",
        )?;
        ensure_eq(resolved.status, ConfigResolutionStatusV1::Invalid, "status")?;
        return Ok(());
    }
    let mut child = std::process::Command::new(std::env::current_exe()?)
        .args(["--exact", "resolved_config::tests::resolved_cargo_allow_config_rejects_non_regular_explicit_policy", "--nocapture"])
        .env("CARGO_ALLOW_FIFO_CHILD", "1")
        .spawn()?;
    for _ in 0..40 {
        if let Some(status) = child.try_wait()? {
            return status
                .success()
                .then_some(())
                .ok_or("FIFO child failed".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    child.kill()?;
    let _ = child.wait();
    Err("FIFO probe exceeded deadline".into())
}

#[test]
fn resolved_cargo_allow_config_selects_only_the_winning_source_for_a_shared_path() -> TestResult {
    let fixture = Fixture::new("shared-candidate-path")?;
    fixture.write("policy/allow.toml", valid_policy())?;
    fixture.write(
        ".allow/config.toml",
        r#"schema_version = "1.0"

[[ledgers]]
id = "source-policy"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
mode = "blocking"
priority = 10
"#,
    )?;

    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:shared")?;
    let selected = resolved
        .candidates
        .iter()
        .filter(|candidate| candidate.disposition == ConfigCandidateDispositionV1::Selected)
        .collect::<Vec<_>>();

    ensure_eq(selected.len(), 1, "selected candidate count")?;
    ensure_eq(
        selected.first().map(|candidate| candidate.source),
        Some(ConfigCandidateSourceV1::FederationRegistry),
        "selected candidate source",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_accepts_absolute_cli_paths_inside_the_root() -> TestResult {
    let fixture = Fixture::new("absolute-in-root")?;
    fixture.write("policy/explicit.toml", valid_policy())?;
    let absolute = fixture.path().join("policy/explicit.toml");

    let resolved =
        resolve_cargo_allow_config_v1(fixture.path(), Some(&absolute), "subject:absolute")?;

    ensure_eq(
        resolved.selection_source,
        Some(ConfigCandidateSourceV1::CliOverride),
        "selection source",
    )?;
    ensure_eq(
        resolved
            .explicit_cli_values
            .iter()
            .map(|path| path.path.as_str())
            .collect::<Vec<_>>(),
        vec!["policy/explicit.toml"],
        "portable explicit CLI values",
    )?;
    ensure(
        resolved.candidates.iter().any(|candidate| {
            candidate.source == ConfigCandidateSourceV1::CliOverride
                && candidate.path.as_ref().map(|path| path.path.as_str())
                    == Some("policy/explicit.toml")
                && candidate.disposition == ConfigCandidateDispositionV1::Selected
        }),
        "absolute in-root CLI path should remain selected and portable",
    )?;
    Ok(())
}

#[cfg(windows)]
#[test]
fn lexical_relative_path_preserves_unicode_case_insensitive_windows_matching() -> TestResult {
    ensure_eq(
        lexical_relative_path(Path::new(r"C:\Répo"), Path::new(r"c:\répo\missing.toml")),
        Some("missing.toml".to_string()),
        "Unicode case-insensitive Windows relative path",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_rejects_non_portable_source_subjects() -> TestResult {
    let fixture = Fixture::new("source-subject-validation")?;
    fixture.write("policy/allow.toml", valid_policy())?;

    ensure(
        resolve_cargo_allow_config_v1(fixture.path(), None, "").is_err(),
        "empty source subject should be rejected",
    )?;
    ensure(
        resolve_cargo_allow_config_v1(
            fixture.path(),
            None,
            &format!("subject:{}", fixture.path().display()),
        )
        .is_err(),
        "source subject containing the private root should be rejected",
    )?;
    ensure(
        resolve_cargo_allow_config_v1(fixture.path(), None, &"x".repeat(1_025)).is_err(),
        "overlong source subject should be rejected",
    )?;
    for private_path_subject in ["subject:/home/alice/private", "artifact:C:\\private\\x"] {
        ensure(
            resolve_cargo_allow_config_v1(fixture.path(), None, private_path_subject).is_err(),
            "source subjects carrying unrelated private paths should be rejected",
        )?;
    }
    if cfg!(windows) {
        ensure(
            resolve_cargo_allow_config_v1(
                fixture.path(),
                None,
                &format!("subject:{}", fixture.path().display()).to_lowercase(),
            )
            .is_err(),
            "case-folded private Windows root should be rejected",
        )?;
    }
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_does_not_report_unreadable_federation_as_no_policy() -> TestResult {
    let fixture = Fixture::new("unreadable-federation")?;
    fs::create_dir_all(fixture.path().join(".allow/config.toml"))?;

    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:unreadable")?;

    ensure_eq(
        resolved.status,
        ConfigResolutionStatusV1::InstrumentFailure,
        "status",
    )?;
    ensure_eq(
        resolved.federation.posture,
        ConfigFederationPostureV1::Unreadable,
        "federation posture",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_rejects_parent_traversing_cli_paths_portably() -> TestResult {
    let fixture = Fixture::new("parent-traversal")?;

    let resolved = resolve_cargo_allow_config_v1(
        fixture.path(),
        Some(Path::new("../outside.toml")),
        "subject:parent",
    )?;

    ensure_eq(resolved.status, ConfigResolutionStatusV1::Invalid, "status")?;
    ensure(
        resolved.explicit_cli_values.is_empty(),
        "unsafe CLI path must not enter portable explicit values",
    )?;
    ensure(
        resolved.candidates.iter().any(|candidate| {
            candidate.source == ConfigCandidateSourceV1::CliOverride
                && candidate.path.is_none()
                && candidate.disposition == ConfigCandidateDispositionV1::Invalid
        }),
        "unsafe CLI path should remain as a bounded invalid candidate",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_reports_unavailable_roots_as_instrument_failure() -> TestResult {
    let fixture = Fixture::new("missing-root-parent")?;
    let missing = fixture.path().join("does-not-exist");

    let resolved = resolve_cargo_allow_config_v1(&missing, None, "subject:missing-root")?;

    ensure_eq(
        resolved.status,
        ConfigResolutionStatusV1::InstrumentFailure,
        "status",
    )?;
    ensure_eq(
        resolved.completeness,
        ConfigCompletenessV1::Unavailable,
        "completeness",
    )?;
    ensure(
        resolved.selected_policy.is_none(),
        "policy should be unavailable",
    )?;
    ensure_eq(
        resolved.requested_root,
        "unknown".to_string(),
        "requested root",
    )?;
    ensure(
        resolved.limitations.iter().any(|limitation| {
            limitation == "requested_root_relationship_could_not_be_represented_portably"
        }),
        "unavailable root should report unknown relationship",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_preserves_valid_long_subject_when_root_is_unavailable() -> TestResult
{
    let fixture = Fixture::new("missing-root-long-subject")?;
    let missing = fixture.path().join("does-not-exist");
    let source_subject = "s".repeat(1_024);

    let resolved = resolve_cargo_allow_config_v1(&missing, None, &source_subject)?;

    ensure_eq(
        resolved.source_subject,
        source_subject,
        "exact unavailable-root source subject",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_redacts_unrelated_absolute_metadata_value() -> TestResult {
    let fixture = Fixture::new("absolute-metadata-redaction")?;
    let private_path = std::env::temp_dir()
        .join("cargo-allow-private")
        .join("policy.toml");
    let private_text = private_path.display().to_string().replace('\\', "/");
    fixture.write(
        "Cargo.toml",
        &format!(
            "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\n\n[package.metadata.cargo-allow]\nconfig = \"{private_text}\"\n"
        ),
    )?;

    let resolved =
        resolve_cargo_allow_config_v1(fixture.path(), None, "subject:absolute-metadata")?;
    let rendered = serde_json::to_string(&resolved)?;

    ensure(
        !rendered.contains(&private_text),
        "unrelated absolute metadata value must not enter the portable artifact",
    )?;
    ensure(
        resolved.candidates.iter().any(|candidate| {
            candidate.source == ConfigCandidateSourceV1::PackageMetadata
                && candidate
                    .reason
                    .as_deref()
                    .is_some_and(|reason| reason.contains("source-tree-relative"))
        }),
        "redacted metadata rejection should retain bounded provenance",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_keeps_no_policy_with_unrelated_federation_lane() -> TestResult {
    let fixture = Fixture::new("spec-only-federation")?;
    fixture.write(
        ".allow/config.toml",
        r#"schema_version = "1.0"

[[ledgers]]
id = "doc-artifacts"
path = ".allow/artifacts/doc-artifacts.toml"
dialect = "cargo-allow-doc-artifacts"
role = "canonical"
lanes = ["spec-system"]
priority = 20
"#,
    )?;

    let resolved = resolve_cargo_allow_config_v1(fixture.path(), None, "subject:spec-only")?;

    ensure_eq(
        resolved.status,
        ConfigResolutionStatusV1::NoPolicy,
        "source-exception status",
    )?;
    ensure_eq(
        resolved.federation.posture,
        ConfigFederationPostureV1::Valid,
        "federation participation posture",
    )?;
    ensure(
        !resolved
            .candidates
            .iter()
            .any(|candidate| candidate.source == ConfigCandidateSourceV1::FederationRegistry),
        "an unrelated valid lane is participation, not a source-policy candidate",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_keeps_federation_ledger_candidate_under_cli() -> TestResult {
    let fixture = Fixture::new("cli-over-federation-candidate")?;
    fixture.write("policy/cli.toml", valid_policy())?;
    fixture.write("policy/federated.toml", valid_policy())?;
    fixture.write(
        ".allow/config.toml",
        r#"schema_version = "1.0"

[[ledgers]]
id = "source-policy"
path = "policy/federated.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
priority = 10
"#,
    )?;

    let resolved = resolve_cargo_allow_config_v1(
        fixture.path(),
        Some(Path::new("policy/cli.toml")),
        "subject:cli-over-federation",
    )?;

    ensure(
        resolved.candidates.iter().any(|candidate| {
            candidate.source == ConfigCandidateSourceV1::FederationRegistry
                && candidate.disposition == ConfigCandidateDispositionV1::Available
                && candidate
                    .path
                    .as_ref()
                    .is_some_and(|path| path.path == "policy/federated.toml")
        }),
        "available federation candidate should retain the source-policy ledger identity",
    )?;
    Ok(())
}

#[test]
fn resolved_cargo_allow_config_reports_external_cli_as_unsupported_without_identity() -> TestResult
{
    let fixture = Fixture::new("external-cli-root")?;
    let external = Fixture::new("external-cli-policy")?;
    external.write("policy.toml", valid_policy())?;
    let external_path = external.path().join("policy.toml").canonicalize()?;

    let resolved = resolve_cargo_allow_config_v1(
        fixture.path(),
        Some(&external_path),
        "subject:external-cli",
    )?;
    let rendered = serde_json::to_string(&resolved)?;

    ensure_eq(
        resolved.status,
        ConfigResolutionStatusV1::Unsupported,
        "external CLI posture",
    )?;
    ensure(
        resolved.selected_policy.is_none(),
        "unsupported external identity must not be projected as a repository policy",
    )?;
    ensure(
        !rendered.contains(&external_path.display().to_string()),
        "external CLI identity must remain redacted",
    )?;
    ensure(
        resolved
            .limitations
            .iter()
            .any(|limitation| limitation == EXTERNAL_CLI_LIMITATION),
        "external CLI limitation should be explicit",
    )?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn resolved_cargo_allow_config_rejects_literal_backslash_filename_identity() -> TestResult {
    let fixture = Fixture::new("literal-backslash")?;
    let literal = r"policy\..\outside.toml";
    fixture.write(literal, valid_policy())?;

    let resolved = resolve_cargo_allow_config_v1(
        fixture.path(),
        Some(Path::new(literal)),
        "subject:literal-backslash",
    )?;

    ensure_eq(
        resolved.status,
        ConfigResolutionStatusV1::Unsupported,
        "literal backslash identity posture",
    )?;
    ensure(
        resolved.selected_policy.is_none(),
        "literal Unix filename must not be normalized into another selected identity",
    )?;
    ensure(
        !resolved.candidates.iter().any(|candidate| {
            candidate
                .path
                .as_ref()
                .is_some_and(|path| path.path == "outside.toml")
        }),
        "literal backslash filename must not collide with outside.toml",
    )?;
    Ok(())
}

// APFS rejects this invalid byte sequence before cargo-allow can observe it;
// Linux provides the concrete filesystem witness for the portable-path rule.
#[cfg(target_os = "linux")]
#[test]
fn resolved_cargo_allow_config_rejects_non_utf8_filename_identity() -> TestResult {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let fixture = Fixture::new("non-utf8-identity")?;
    let invalid_name = OsString::from_vec(b"policy-\xff.toml".to_vec());
    let invalid_path = PathBuf::from(&invalid_name);
    fs::write(fixture.path().join(&invalid_path), valid_policy())?;
    fixture.write("policy-�.toml", valid_policy())?;

    let resolved = resolve_cargo_allow_config_v1(
        fixture.path(),
        Some(&invalid_path),
        "subject:non-utf8-identity",
    )?;
    let rendered = serde_json::to_string(&resolved)?;

    ensure_eq(
        resolved.status,
        ConfigResolutionStatusV1::Unsupported,
        "non-UTF-8 identity posture",
    )?;
    ensure(
        resolved.selected_policy.is_none(),
        "non-UTF-8 identity must not produce a selected policy or digest",
    )?;
    let cli_candidates = resolved
        .candidates
        .iter()
        .filter(|candidate| candidate.source == ConfigCandidateSourceV1::CliOverride)
        .collect::<Vec<_>>();
    ensure_eq(cli_candidates.len(), 1, "CLI candidate count")?;
    ensure(
        cli_candidates.first().is_some_and(|candidate| {
            candidate.path.is_none()
                && candidate.disposition == ConfigCandidateDispositionV1::Invalid
                && candidate.reason.as_deref()
                    == Some("CLI path cannot be represented as a portable identity")
        }),
        "non-UTF-8 CLI candidate must have a truthful invalid portable posture",
    )?;
    ensure(
        !resolved
            .candidates
            .iter()
            .any(|candidate| candidate.disposition == ConfigCandidateDispositionV1::Selected),
        "non-UTF-8 identity must not select any candidate",
    )?;
    ensure(
        !rendered.contains("policy-�.toml"),
        "non-UTF-8 identity must not collide with a replacement-character filename",
    )?;
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn resolved_cargo_allow_config_preserves_distinct_unicode_filename_spellings() -> TestResult {
    let fixture = Fixture::new("unicode-spelling")?;
    let decomposed = "policy/cafe\u{301}.toml";
    let composed = "policy/caf\u{e9}.toml";
    fixture.write(decomposed, &format!("{}\n# decomposed", valid_policy()))?;
    fixture.write(composed, &format!("{}\n# composed", valid_policy()))?;

    let decomposed_resolution = resolve_cargo_allow_config_v1(
        fixture.path(),
        Some(Path::new(decomposed)),
        "subject:unicode-decomposed",
    )?;
    let composed_resolution = resolve_cargo_allow_config_v1(
        fixture.path(),
        Some(Path::new(composed)),
        "subject:unicode-composed",
    )?;
    let decomposed_policy = decomposed_resolution
        .selected_policy
        .as_ref()
        .ok_or("decomposed policy should be selected")?;
    let composed_policy = composed_resolution
        .selected_policy
        .as_ref()
        .ok_or("composed policy should be selected")?;

    ensure_eq(
        decomposed_resolution.status,
        ConfigResolutionStatusV1::Complete,
        "decomposed status",
    )?;
    ensure_eq(
        composed_resolution.status,
        ConfigResolutionStatusV1::Complete,
        "composed status",
    )?;
    ensure_eq(
        decomposed_policy.path.path.as_str(),
        decomposed,
        "decomposed path spelling",
    )?;
    ensure_eq(
        composed_policy.path.path.as_str(),
        composed,
        "composed path spelling",
    )?;
    ensure(
        decomposed_policy.path.path != composed_policy.path.path,
        "distinct filesystem spellings must retain distinct portable identities",
    )?;
    ensure(
        decomposed_policy.digest != composed_policy.digest,
        "distinct policy bytes must retain distinct digests",
    )?;
    Ok(())
}

#[test]
fn resolved_config_candidates_retain_projection_order() -> TestResult {
    let fixture = Fixture::new("candidate-order")?;
    let root = fixture.path();
    fixture.write(
        "policy/allow.toml",
        "schema_version = \"1\"\npolicy = \"other\"\n",
    )?;
    let resolved = resolve_cargo_allow_config_v1(root, None, "subject")?;
    let positions: Vec<u32> = resolved
        .candidates
        .iter()
        .map(|candidate| candidate.observation_position.ok_or("missing position"))
        .collect::<Result<_, _>>()?;
    ensure_eq(
        positions,
        (0..resolved.candidates.len() as u32).collect::<Vec<_>>(),
        "candidate projection order",
    )?;
    Ok(())
}

fn valid_policy() -> &'static str {
    r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "repo-infra"
status = "active"

[workspace]
ignored = [".git/**", "target/**"]
generated = ["target/**"]
default_mode = "no-new"
"#
}

fn ensure(condition: bool, message: &str) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.to_string().into())
    }
}

fn ensure_eq<T>(actual: T, expected: T, label: &str) -> TestResult
where
    T: std::fmt::Debug + PartialEq,
{
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{label}: expected {expected:?}, got {actual:?}").into())
    }
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> std::io::Result<Self> {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(std::io::Error::other)?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cargo-allow-resolved-config-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, relative: &str, contents: &str) -> std::io::Result<()> {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, contents)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
