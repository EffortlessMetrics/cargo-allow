use allow_policy::ResolvedCargoAllowConfigV1;

/// Render the reusable resolved-configuration component with deterministic
/// field and candidate ordering supplied by the v1 policy adapter.
pub fn render_resolved_cargo_allow_config_json(
    resolved: &ResolvedCargoAllowConfigV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(resolved).map(|mut rendered| {
        rendered.push('\n');
        rendered
    })
}

#[cfg(test)]
mod tests {
    use allow_policy::{
        ConfigCandidateDispositionV1, ConfigCandidateSourceV1, ConfigCandidateV1,
        ConfigCompletenessV1, ConfigFallbackV1, ConfigFederationParticipationV1,
        ConfigFederationPostureV1, ConfigPathAnchorV1, ConfigPrecedenceTierV1,
        ConfigProfileParticipationV1, ConfigResolutionStatusV1, PortableConfigPathV1,
        RESOLVED_CARGO_ALLOW_CONFIG_CLAIM_BOUNDARY, RESOLVED_CARGO_ALLOW_CONFIG_SCHEMA_ID,
        RESOLVED_CARGO_ALLOW_CONFIG_SCHEMA_VERSION, ResolvedCargoAllowConfigV1, ResolvedPolicyV1,
    };

    use super::render_resolved_cargo_allow_config_json;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn resolved_config_renderer_is_deterministic_and_portable() -> TestResult {
        let resolved = sample();
        let first = render_resolved_cargo_allow_config_json(&resolved)?;
        let second = render_resolved_cargo_allow_config_json(&resolved)?;

        ensure(first == second, "renderer should be deterministic")?;
        ensure(
            first.ends_with('\n'),
            "renderer should end with one newline",
        )?;
        ensure(
            !first.contains("C:\\\\private") && !first.contains("/home/private"),
            "renderer should not invent private roots",
        )?;
        Ok(())
    }

    #[test]
    fn resolved_config_schema_matches_the_typed_contract() -> TestResult {
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/schemas/resolved-cargo-allow-config-v1.schema.json"
        ))?;
        ensure(
            schema
                .pointer("/properties/schema_id/const")
                .and_then(serde_json::Value::as_str)
                == Some(RESOLVED_CARGO_ALLOW_CONFIG_SCHEMA_ID),
            "schema id should match the producer",
        )?;
        ensure(
            schema
                .pointer("/properties/schema_version/const")
                .and_then(serde_json::Value::as_u64)
                == Some(u64::from(RESOLVED_CARGO_ALLOW_CONFIG_SCHEMA_VERSION)),
            "schema version should match the producer",
        )?;
        ensure_enum(
            &schema,
            "/properties/status/enum",
            &[
                "complete",
                "no_policy",
                "invalid",
                "partial",
                "ambiguous",
                "unsupported",
                "instrument_failure",
            ],
        )?;
        ensure_enum(
            &schema,
            "/$defs/candidate/properties/source/enum",
            &[
                "cli_override",
                "federation_registry",
                "package_metadata",
                "workspace_metadata",
                "cargo_metadata",
                "conventional_path",
                "legacy_discovery",
            ],
        )?;
        let rendered: serde_json::Value =
            serde_json::from_str(&render_resolved_cargo_allow_config_json(&sample())?)?;
        for required in schema
            .get("required")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
        {
            ensure(
                rendered.get(required).is_some(),
                &format!("rendered contract should include required field {required}"),
            )?;
        }
        Ok(())
    }

    fn sample() -> ResolvedCargoAllowConfigV1 {
        ResolvedCargoAllowConfigV1 {
            schema_id: RESOLVED_CARGO_ALLOW_CONFIG_SCHEMA_ID.to_string(),
            schema_version: RESOLVED_CARGO_ALLOW_CONFIG_SCHEMA_VERSION,
            producer_generation: 1,
            source_subject: "sha256:v1:subject".to_string(),
            requested_root: ".".to_string(),
            resolved_repository_root: ".".to_string(),
            status: ConfigResolutionStatusV1::Complete,
            completeness: ConfigCompletenessV1::Partial,
            selected_policy: Some(ResolvedPolicyV1 {
                path: root_path("policy/allow.toml"),
                digest: Some("sha256:v1:policy".to_string()),
                schema_version: Some("0.1".to_string()),
                policy: Some("cargo-allow".to_string()),
                status: Some("active".to_string()),
            }),
            selection_source: Some(ConfigCandidateSourceV1::ConventionalPath),
            precedence_tier: Some(ConfigPrecedenceTierV1::DiscoveryFallback),
            explicit_cli_values: Vec::new(),
            candidates: vec![ConfigCandidateV1 {
                source: ConfigCandidateSourceV1::ConventionalPath,
                path: Some(root_path("policy/allow.toml")),
                disposition: ConfigCandidateDispositionV1::Selected,
                reason: None,
            }],
            fallback: ConfigFallbackV1 {
                considered: false,
                selected: false,
                reason: None,
            },
            federation: ConfigFederationParticipationV1 {
                config_path: root_path(".allow/config.toml"),
                posture: ConfigFederationPostureV1::Missing,
                selected_for_source_exception: false,
                configured_ledgers: Vec::new(),
                diagnostics: Vec::new(),
            },
            profile: ConfigProfileParticipationV1 {
                observed: false,
                selected_profile: None,
                reason: "separate current consumer".to_string(),
            },
            inventory_mode: None,
            ignored_scopes: vec![".git/**".to_string()],
            generated_scopes: vec!["target/**".to_string()],
            selected_sensor_families: Vec::new(),
            diagnostics: Vec::new(),
            limitations: vec![
                "current_multi_pass_adapter_does_not_prove_atomic_resolution".to_string(),
            ],
            claim_boundary: RESOLVED_CARGO_ALLOW_CONFIG_CLAIM_BOUNDARY.to_string(),
        }
    }

    fn root_path(path: &str) -> PortableConfigPathV1 {
        PortableConfigPathV1 {
            anchor: ConfigPathAnchorV1::ResolvedRepositoryRoot,
            ancestor_depth: 0,
            path: path.to_string(),
        }
    }

    fn ensure_enum(schema: &serde_json::Value, pointer: &str, expected: &[&str]) -> TestResult {
        let Some(values) = schema
            .pointer(pointer)
            .and_then(serde_json::Value::as_array)
        else {
            return Err(format!("schema should define enum {pointer}").into());
        };
        let actual = values
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        let expected = expected
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        ensure(actual == expected, &format!("enum mismatch at {pointer}"))
    }

    fn ensure(condition: bool, message: &str) -> TestResult {
        if condition {
            Ok(())
        } else {
            Err(message.to_string().into())
        }
    }
}
