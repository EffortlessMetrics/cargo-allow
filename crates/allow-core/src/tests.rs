use super::*;
use std::path::PathBuf;
use std::str::FromStr;

mod date;
mod fingerprint;
mod identity;
mod source_tree_path;

#[test]
fn glob_question_mark_matches_one_unicode_character() {
    assert!(glob_matches_str("docs/?.md", "docs/é.md"));
    assert!(glob_matches_str("docs/??.md", "docs/éx.md"));
    assert!(!glob_matches_str("docs/?.md", "docs/ee.md"));
}

#[test]
fn finding_kind_accepts_hyphenated_cli_aliases() {
    assert!(matches!(
        FindingKind::from_str(" NON-RUST "),
        Ok(FindingKind::NonRustFile)
    ));
    assert!(matches!(
        FindingKind::from_str(" LINT-EXCEPTION "),
        Ok(FindingKind::LintException)
    ));
    assert!(matches!(
        FindingKind::from_str(" GENERATED-CODE "),
        Ok(FindingKind::GeneratedCode)
    ));
}

#[test]
fn source_code_kinds_require_selector_identity() {
    assert!(FindingKind::Panic.requires_source_selector_identity());
    assert!(FindingKind::Unsafe.requires_source_selector_identity());
    assert!(FindingKind::LintException.requires_source_selector_identity());
    assert!(!FindingKind::NonRustFile.requires_source_selector_identity());
    assert!(!FindingKind::GeneratedCode.requires_source_selector_identity());
    assert!(!FindingKind::PolicyException.requires_source_selector_identity());
}

#[test]
fn json_escape_covers_quotes_backslashes_whitespace_and_control_chars() {
    assert_eq!(
        json_escape("quote: \" slash: \\ newline:\n tab:\t return:\r bell:\u{0007}"),
        "quote: \\\" slash: \\\\ newline:\\n tab:\\t return:\\r bell:\\u0007"
    );
}

#[test]
fn allow_config_empty_sets_document_defaults() {
    let config = AllowConfig::empty();

    assert_eq!(config.schema_version, "0.1");
    assert_eq!(config.policy, "cargo-allow");
    assert_eq!(config.owner, None);
    assert_eq!(config.status.as_deref(), Some("active"));
    assert_eq!(config.workspace, WorkspaceConfig::default());
    assert_eq!(config.requirements, Requirements::default());
    assert!(config.allow.is_empty());
}

#[test]
fn allow_config_empty_validate_passes() {
    // The documented-empty config must validate: programmatic core consumers
    // building `AllowConfig::empty()` get a valid baseline.
    assert!(AllowConfig::empty().validate().is_ok());
}

#[test]
fn validate_rejects_empty_schema_version() {
    let mut config = AllowConfig::empty();
    config.schema_version = "  ".to_string();
    let err = config
        .validate()
        .expect_err("empty schema_version should fail");
    assert!(err.to_string().contains("schema_version must not be empty"));
    assert_eq!(err.kind(), CargoAllowErrorKind::InvalidPolicy);
}

#[test]
fn validate_rejects_unsupported_schema_version() {
    let mut config = AllowConfig::empty();
    config.schema_version = "0.2".to_string();
    let err = config
        .validate()
        .expect_err("unsupported schema_version should fail");
    assert!(
        err.to_string()
            .contains("unsupported policy schema_version `0.2`")
    );
}

#[test]
fn validate_rejects_empty_policy_name() {
    let mut config = AllowConfig::empty();
    config.policy = String::new();
    let err = config.validate().expect_err("empty policy should fail");
    assert!(err.to_string().contains("policy name must not be empty"));
}

#[test]
fn validate_rejects_unsupported_policy_name() {
    let mut config = AllowConfig::empty();
    config.policy = "not-cargo-allow".to_string();
    let err = config
        .validate()
        .expect_err("unsupported policy name should fail");
    assert!(
        err.to_string()
            .contains("unsupported policy `not-cargo-allow`")
    );
}

#[test]
fn validate_rejects_typoed_default_mode() {
    // `no_new` (underscore) is a common typo for `no-new` and must be rejected
    // at the core data-model level, not silently treated as a string.
    let mut config = AllowConfig::empty();
    config.workspace.default_mode = "no_new".to_string();
    let err = config
        .validate()
        .expect_err("typoed default_mode should fail");
    assert!(
        err.to_string()
            .contains("unsupported workspace default_mode `no_new`")
    );
}

#[test]
fn validate_rejects_empty_status() {
    let mut config = AllowConfig::empty();
    config.status = Some("  ".to_string());
    let err = config.validate().expect_err("empty status should fail");
    assert!(err.to_string().contains("policy status must not be empty"));
}

#[test]
fn validate_aggregates_multiple_errors() {
    let mut config = AllowConfig::empty();
    config.schema_version = String::new();
    config.policy = String::new();
    config.workspace.default_mode = "bogus".to_string();
    let err = config
        .validate()
        .expect_err("multiple problems should fail");
    let message = err.to_string();
    assert!(
        message.starts_with("3 policy validation errors:"),
        "{message}"
    );
    assert!(message.contains("schema_version must not be empty"));
    assert!(message.contains("policy name must not be empty"));
    assert!(message.contains("unsupported workspace default_mode `bogus`"));
    assert_eq!(err.diagnostics().len(), 3);
    assert!(
        err.diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.category == "policy_validation")
    );
}

#[test]
fn workspace_mode_round_trips_and_rejects_unknown() {
    for mode in WorkspaceMode::ALL {
        let s = mode.as_str();
        assert!(
            matches!(WorkspaceMode::from_str(s), Ok(parsed) if parsed == *mode),
            "{s}"
        );
    }
    // The default is `no-new`.
    assert_eq!(WorkspaceMode::default(), WorkspaceMode::NoNew);
    assert_eq!(WorkspaceMode::default().as_str(), "no-new");
    // `no_new` (underscore) must not parse — it is a distinct value from
    // `no-new` and a common copy-paste typo.
    let err = WorkspaceMode::from_str("no_new").expect_err("underscore form must not parse");
    assert!(
        err.to_string()
            .contains("unsupported workspace default_mode `no_new`")
    );
}

#[test]
fn match_status_strings_and_failure_modes_cover_all_statuses() {
    for status in [
        MatchStatus::Matched,
        MatchStatus::New,
        MatchStatus::Stale,
        MatchStatus::Expired,
        MatchStatus::ReviewDue,
        MatchStatus::LocationDrift,
        MatchStatus::Ambiguous,
        MatchStatus::InvalidSelector,
        MatchStatus::MissingRequiredField,
        MatchStatus::EvidenceMissing,
        MatchStatus::BaselineDebt,
    ] {
        let (expected_name, strict_failure, no_new_failure) = match status {
            MatchStatus::Matched => ("matched", false, false),
            MatchStatus::New => ("new", true, true),
            MatchStatus::Stale => ("stale", true, false),
            MatchStatus::Expired => ("expired", true, true),
            MatchStatus::ReviewDue => ("review_due", true, false),
            MatchStatus::LocationDrift => ("location_drift", false, false),
            MatchStatus::Ambiguous => ("ambiguous", true, true),
            MatchStatus::InvalidSelector => ("invalid_selector", true, true),
            MatchStatus::MissingRequiredField => ("missing_required_field", true, true),
            MatchStatus::EvidenceMissing => ("evidence_missing", true, true),
            MatchStatus::BaselineDebt => ("baseline_debt", true, false),
        };

        assert_eq!(status.as_str(), expected_name);
        assert_eq!(status.is_failure_in_strict(), strict_failure, "{status:?}");
        assert_eq!(status.is_failure_in_no_new(), no_new_failure, "{status:?}");
    }
}

#[test]
fn finding_kind_display_and_parser_cover_policy_aliases_and_errors() {
    assert_eq!(FindingKind::PolicyException.to_string(), "policy_exception");
    assert!(matches!(
        FindingKind::from_str(" policy-exception "),
        Ok(FindingKind::PolicyException)
    ));
    assert!(matches!(
        FindingKind::from_str("generated"),
        Ok(FindingKind::GeneratedCode)
    ));
    let error = FindingKind::from_str("unknown-kind")
        .unwrap_err()
        .to_string();
    assert!(error.contains("unsupported finding kind `unknown-kind`"));
    assert!(error.contains(
        "valid values: panic, unsafe, lint_exception, non_rust_file, generated_code, policy_exception"
    ));
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    fn path_segment() -> impl Strategy<Value = String> {
        prop_oneof![
            Just(".".to_string()),
            Just("..".to_string()),
            "[A-Za-z0-9_-]{1,8}".prop_map(|s| s),
        ]
    }

    fn relative_path_text() -> impl Strategy<Value = String> {
        prop::collection::vec(path_segment(), 0..12).prop_map(|segments| segments.join("/"))
    }

    fn stable_text() -> impl Strategy<Value = String> {
        "[A-Za-z0-9_:/|.-]{0,24}".prop_map(|s| s)
    }

    fn maybe_stable_text() -> impl Strategy<Value = Option<String>> {
        prop::option::of(stable_text())
    }

    prop_compose! {
        fn structural_identity_strategy()(
            language in stable_text(),
            ast_kind in stable_text(),
            crate_name in maybe_stable_text(),
            module in maybe_stable_text(),
            container in maybe_stable_text(),
            symbol in maybe_stable_text(),
            callee in maybe_stable_text(),
            macro_name in maybe_stable_text(),
            lint in maybe_stable_text(),
            receiver_fingerprint in maybe_stable_text(),
            target_fingerprint in maybe_stable_text(),
            normalized_snippet_hash in maybe_stable_text(),
            line_hint in prop::option::of(any::<u32>()),
            column_hint in prop::option::of(any::<u32>()),
        ) -> StructuralIdentity {
            StructuralIdentity {
                language,
                crate_name,
                module,
                container,
                ast_kind,
                symbol,
                callee,
                macro_name,
                lint,
                receiver_fingerprint,
                target_fingerprint,
                normalized_snippet_hash,
                line_hint,
                column_hint,
            }
        }
    }

    proptest! {
        #[test]
        fn normalize_path_is_idempotent(path in relative_path_text()) {
            let normalized = normalize_path(&path);
            prop_assert_eq!(normalize_path(&normalized), normalized);
        }

        #[test]
        fn normalize_path_removes_current_and_empty_segments(path in relative_path_text()) {
            let normalized = normalize_path(&path);
            prop_assert!(
                normalized.is_empty()
                    || !normalized
                        .split('/')
                        .any(|part| part.is_empty() || part == ".")
            );
        }

        #[test]
        fn double_star_glob_matches_any_number_of_whole_segments(
            prefix in "[A-Za-z0-9_-]{1,8}",
            middle in prop::collection::vec("[A-Za-z0-9_-]{1,8}", 0..6),
            file in r"[A-Za-z0-9_-]{1,8}\.rs",
        ) {
            let pattern = format!("{prefix}/**/*.rs");
            let path = std::iter::once(prefix)
                .chain(middle)
                .chain(std::iter::once(file))
                .collect::<Vec<_>>()
                .join("/");

            prop_assert!(glob_matches_str(&pattern, &path));
        }

        #[test]
        fn subtree_ignore_pattern_does_not_match_shared_prefix_sibling(
            prefix in "[A-Za-z][A-Za-z0-9_-]{0,8}",
            suffix in "[A-Za-z0-9_-]{1,8}",
            leaf in "[A-Za-z0-9_-]{1,8}",
        ) {
            let pattern = format!("{prefix}/**");
            let ignored = format!("{prefix}/{leaf}");
            let sibling = format!("{prefix}{suffix}/{leaf}");
            let patterns = vec![pattern];

            prop_assert!(source_tree_path_is_ignored(&ignored, &patterns));
            prop_assert!(!source_tree_path_is_ignored(&sibling, &patterns));
        }

        #[test]
        fn date_epoch_day_roundtrips(days in 0_i64..2_000_000_i64) {
            let date = SimpleDate::from_days_since_unix_epoch(days);

            prop_assert_eq!(date.days_since_unix_epoch(), days);
            prop_assert_eq!(SimpleDate::parse(&date.to_string()), Some(date));
        }

        #[test]
        fn date_add_days_matches_days_until(
            start_days in -2_000_000_i64..2_000_000_i64,
            delta in -20_000_i64..20_000_i64,
        ) {
            let start = SimpleDate::from_days_since_unix_epoch(start_days);
            let end = start.add_days(delta);

            prop_assert_eq!(start.days_until(end), delta);
            prop_assert_eq!(end.days_since_unix_epoch(), start_days + delta);
        }

        #[test]
        fn stable_hash_hex_has_expected_shape_and_is_deterministic(input in ".{0,256}") {
            let first = stable_hash_hex(&input);
            let second = stable_hash_hex(&input);

            prop_assert_eq!(&first, &second);
            prop_assert!(first.starts_with("fnv1a64:"));
            prop_assert_eq!(first.len(), "fnv1a64:".len() + 16);
            prop_assert!(first["fnv1a64:".len()..].chars().all(|ch| ch.is_ascii_hexdigit()));
        }

        #[test]
        fn stable_identity_keys_ignore_location_hints(
            mut identity in structural_identity_strategy(),
            line_hint in any::<u32>(),
            column_hint in any::<u32>(),
        ) {
            let base = identity.stable_key();
            identity.line_hint = Some(line_hint);
            identity.column_hint = Some(column_hint);

            prop_assert_eq!(identity.stable_key(), base);
        }

        #[test]
        fn finding_identity_keys_ignore_span(
            mut finding_path in relative_path_text(),
            mut identity in structural_identity_strategy(),
            family in maybe_stable_text(),
            line in any::<u32>(),
            column in any::<u32>(),
        ) {
            if finding_path.is_empty() {
                finding_path = "src/lib.rs".to_string();
            }
            identity.line_hint = Some(line);
            identity.column_hint = Some(column);
            let finding = Finding {
                kind: FindingKind::Unsafe,
                family,
                path: PathBuf::from(finding_path),
                span: Some(Span { line, column }),
                identity,
                message: "generated finding".to_string(),
                ledger: None,
            };
            let mut moved = finding.clone();
            moved.span = Some(Span {
                line: line.wrapping_add(1),
                column: column.wrapping_add(1),
            });

            prop_assert_eq!(finding_identity_key(&finding), finding_identity_key(&moved));
        }
    }
}
