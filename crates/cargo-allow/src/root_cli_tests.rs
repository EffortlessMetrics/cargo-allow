use std::path::Path;

use crate::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::PersistentCacheMode;
    use crate::cli::ColorChoice;
    use clap::CommandFactory;
    use clap::Parser;

    #[test]
    fn normalized_args_accepts_cargo_subcommand_prefix() {
        let normalized = normalized_args(argv(vec!["cargo-allow", "allow", "audit"]));
        let expected = argv(vec!["cargo-allow", "audit"]);
        assert_eq!(normalized, expected);
    }

    #[test]
    fn normalized_args_accepts_cargo_completions_subcommand() {
        let normalized = normalized_args(argv(vec!["cargo-allow", "allow", "completions", "bash"]));
        let expected = argv(vec!["cargo-allow", "completions", "bash"]);
        assert_eq!(normalized, expected);
    }

    #[test]
    fn normalized_args_accepts_root_flag_before_cargo_subcommand_prefix() {
        let normalized = normalized_args(argv(vec![
            "cargo-allow",
            "--color=always",
            "allow",
            "audit",
        ]));
        let expected = argv(vec!["cargo-allow", "--color=always", "audit"]);
        assert_eq!(normalized, expected);
    }

    #[test]
    fn normalized_args_strips_allow_shim_when_flags_follow_before_subcommand() {
        let normalized =
            normalized_args(argv(vec!["cargo-allow", "allow", "--color=never", "check"]));
        let expected = argv(vec!["cargo-allow", "--color=never", "check"]);
        assert_eq!(normalized, expected);
    }

    #[test]
    fn normalized_args_does_not_strip_bare_allow_without_following_subcommand() {
        // Keeps `allow` available as a future subcommand name. Cargo plugin
        // invocations with only the shim token stay free for that evolution.
        let normalized = normalized_args(argv(vec!["cargo-allow", "allow"]));
        let expected = argv(vec!["cargo-allow", "allow"]);
        assert_eq!(normalized, expected);
    }

    #[test]
    fn normalized_args_strips_allow_shim_before_root_version_flag() {
        // `cargo allow --version` becomes `cargo-allow allow --version`.
        let normalized = normalized_args(argv(vec!["cargo-allow", "allow", "--version"]));
        let expected = argv(vec!["cargo-allow", "--version"]);
        assert_eq!(normalized, expected);
    }

    #[test]
    fn normalized_args_does_not_strip_allow_when_followed_by_unknown_command() {
        let normalized = normalized_args(argv(vec!["cargo-allow", "allow", "future-cmd"]));
        let expected = argv(vec!["cargo-allow", "allow", "future-cmd"]);
        assert_eq!(normalized, expected);
    }

    #[test]
    fn normalized_args_does_not_strip_allow_after_real_subcommand() {
        let normalized = normalized_args(argv(vec!["cargo-allow", "add", "allow"]));
        let expected = argv(vec!["cargo-allow", "add", "allow"]);
        assert_eq!(normalized, expected);
    }

    #[test]
    fn known_subcommands_do_not_reserve_allow_name() {
        assert!(!CargoAllowCommand::SUBCOMMANDS.contains(&"allow"));
    }

    #[test]
    fn shim_registry_covers_every_clap_subcommand() {
        let command = CargoAllowCli::command();
        for subcommand in command.get_subcommands() {
            assert!(
                CargoAllowCommand::SUBCOMMANDS.contains(&subcommand.get_name()),
                "cargo-plugin shim registry is missing `{}`",
                subcommand.get_name()
            );
        }
    }

    #[test]
    fn clap_parses_color_before_cargo_subcommand_prefix() {
        let parsed = CargoAllowCli::try_parse_from(normalized_args(argv(vec![
            "cargo-allow",
            "--color=always",
            "allow",
            "add",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "1",
            "--owner",
            "runtime",
            "--reason",
            "migration smoke",
        ])))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("CLI should parse root color before allow: {err}"))
        });

        assert_eq!(parsed.color, ColorChoice::Always);
        assert!(matches!(parsed.command, Some(CargoAllowCommand::Add(_))));
    }

    #[test]
    fn clap_parses_explicit_persistent_cache_mode() -> Result<(), String> {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "check",
            "--persistent-cache",
            "off",
        ]))
        .map_err(|err| format!("persistent cache mode should parse: {err}"))?;
        let Some(CargoAllowCommand::Check(args)) = parsed.command else {
            return Err("expected check command".to_string());
        };
        if args.persistent_cache != PersistentCacheMode::Off {
            return Err("persistent cache mode did not parse as off".to_string());
        }
        Ok(())
    }

    #[test]
    fn clap_rejects_unknown_persistent_cache_mode() -> Result<(), String> {
        let error = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "check",
            "--persistent-cache",
            "sometimes",
        ]))
        .err()
        .ok_or_else(|| "unknown persistent cache mode should fail".to_string())?;
        if error.kind() != clap::error::ErrorKind::InvalidValue {
            return Err("unknown mode returned the wrong parser error".to_string());
        }
        Ok(())
    }

    #[test]
    fn clap_rejects_persistent_cache_mode_on_non_check_commands() -> Result<(), String> {
        if CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "audit",
            "--persistent-cache",
            "off",
        ]))
        .is_ok()
        {
            return Err("persistent cache mode unexpectedly parsed for audit".to_string());
        }
        Ok(())
    }

    #[test]
    fn clap_exposes_package_version() {
        let mut command = CargoAllowCli::command();

        assert_eq!(
            command.get_version(),
            Some(env!("CARGO_PKG_VERSION")),
            "root CLI should expose the package version"
        );
        assert!(
            command.render_help().to_string().contains("-V, --version"),
            "root help should include the standard version flag"
        );
    }

    #[test]
    fn clap_propagates_version_to_subcommands() {
        // #2597: `cargo-allow <subcommand> --version` should print the package
        // version, not error with "unexpected argument". clap's
        // `propagate_version` mirrors the root `version` onto every subcommand.
        //
        // clap performs propagation during command build (triggered by
        // parsing), so we exercise it by parsing each subcommand with
        // `--version` and asserting the parse produces a `DisplayVersion`
        // error rather than an `UnknownArgument` error.
        for name in [
            "init",
            "audit",
            "check",
            "diff",
            "list",
            "explain",
            "why",
            "add",
            "propose",
            "worklist",
            "migrate",
            "refresh",
            "prune",
            "doctor",
            "tool",
            "completions",
            "reference",
        ] {
            let parsed =
                CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", name, "--version"]));
            let err = parsed.expect_err(
                "subcommand {name}: --version should short-circuit to a DisplayVersion error",
            );
            assert_eq!(
                err.kind(),
                clap::error::ErrorKind::DisplayVersion,
                "subcommand {name}: --version should be recognized, got {:?}",
                err.kind()
            );
        }
    }

    #[test]
    fn clap_parses_markdown_alias() {
        let parsed =
            CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "check", "--format", "md"]))
                .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Check(check::CheckArgs {
                artifact_dir: None,
                emit: None,
                format: OutputFormat::Markdown,
                ..
            }))
        ));
    }

    #[test]
    fn diff_without_base_parses_and_auto_detects() {
        // --base is now optional; merge-base is auto-detected at runtime (#2788).
        let parsed = CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "diff"]))
            .unwrap_or_else(|err| {
                std::panic::panic_any(format!("diff without --base should parse: {err}"))
            });
        assert!(matches!(parsed.command, Some(CargoAllowCommand::Diff(_))));
    }

    #[test]
    fn clap_parses_source_tree_root_for_check() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "check",
            "--root",
            "fixtures/source-snapshot",
        ]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse --root: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Check(check::CheckArgs {
                artifact_dir: None,
                emit: None,
                root: RootArgs { root: Some(root) },
                ..
            })) if root == Path::new("fixtures/source-snapshot")
        ));
    }

    #[test]
    fn clap_leaves_check_mode_unset_when_not_provided() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "check"]))
            .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse check: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Check(check::CheckArgs {
                artifact_dir: None,
                emit: None,
                mode: None,
                ..
            }))
        ));
    }

    #[test]
    fn clap_parses_explicit_check_mode() {
        let parsed =
            CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "check", "--mode", "strict"]))
                .unwrap_or_else(|err| {
                    std::panic::panic_any(format!("CLI should parse --mode strict: {err}"))
                });

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Check(check::CheckArgs {
                artifact_dir: None,
                emit: None,
                mode: Some(mode),
                ..
            })) if mode == "strict"
        ));
    }

    #[test]
    fn clap_parses_repeatable_check_deny_statuses() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "check",
            "--deny",
            "review_due",
            "--deny",
            "stale",
        ]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse --deny: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Check(check::CheckArgs {
                artifact_dir: None,
                emit: None,
                deny,
                ..
            })) if deny == vec!["review_due".to_string(), "stale".to_string()]
        ));
    }

    #[test]
    fn clap_rejects_unknown_kind_filters_for_report_commands() {
        for args in [
            vec!["cargo-allow", "audit", "--kind", "unsfae"],
            vec!["cargo-allow", "check", "--kind", "unsfae"],
            vec![
                "cargo-allow",
                "diff",
                "--base",
                "origin/main",
                "--kind",
                "unsfae",
            ],
            vec!["cargo-allow", "propose", "--kind", "unsfae"],
        ] {
            let err = CargoAllowCli::try_parse_from(argv(args.clone()))
                .expect_err("unknown kind should fail closed");
            assert!(
                err.to_string().contains("unknown kind"),
                "unexpected parse error for {args:?}: {err}"
            );
        }
    }

    #[test]
    fn check_help_describes_policy_default_as_source_tree_gate_mode() {
        let mut command = CargoAllowCli::command();
        let help = command.render_help().to_string();

        assert!(help.contains("Source-tree exception ledger and policy scanner"));

        let Some(check) = command.find_subcommand_mut("check") else {
            std::panic::panic_any("check subcommand should exist");
        };
        let help = check.render_help().to_string();

        assert!(help.contains("CI gate"));
        assert!(help.contains("--deny"));
        assert!(help.contains("occurrence_headroom"));
        assert!(!help.contains("workspace.default_mode"));
    }

    #[test]
    fn check_mode_help_text_matches_gate_semantics() {
        // #2592 follow-up: the --mode descriptions must match what each mode
        // actually does. Two prior inaccuracies are locked out here:
        //   1. strict must NOT claim to fail on drift, because
        //      MatchStatus::is_failure_in_strict explicitly excludes
        //      LocationDrift (policy.rs).
        //   2. release must document that it is currently equivalent to
        //      strict and that deny escalation is driven by --deny, not
        //      implicit in the mode.
        let mut command = CargoAllowCli::command();
        let Some(check) = command.find_subcommand_mut("check") else {
            std::panic::panic_any("check subcommand should exist");
        };
        let help = check.render_help().to_string();

        // strict: claims exception for location_drift, not blanket "fails on drift".
        assert!(
            help.contains("except location_drift"),
            "strict help should document the location_drift exception: {help}"
        );
        assert!(
            !help.contains("stale/review_due/drift"),
            "strict help must not claim to fail on drift: {help}"
        );
        // release: documents equivalence to strict and --deny ownership.
        assert!(
            help.contains("Currently equivalent to strict"),
            "release help should document equivalence to strict: {help}"
        );
        assert!(
            help.contains("driven by --deny"),
            "release help should point to --deny for escalation: {help}"
        );
    }

    #[test]
    fn diff_help_describes_kind_filter_as_source_and_policy_posture() {
        let mut command = CargoAllowCli::command();
        let Some(diff) = command.find_subcommand_mut("diff") else {
            std::panic::panic_any("diff subcommand should exist");
        };
        let help = diff.render_help().to_string();

        assert!(help.contains("Filter source findings and allow-entry policy changes by kind"));
    }

    #[test]
    fn diff_help_describes_base_and_head_as_posture_revisions() {
        let mut command = CargoAllowCli::command();
        let Some(diff) = command.find_subcommand_mut("diff") else {
            std::panic::panic_any("diff subcommand should exist");
        };
        let help = diff.render_help().to_string();

        assert!(help.contains("Base Git revision; resolves to an exact commit before comparison"));
        assert!(
            help.contains(
                "Optional head Git revision; defaults to committed HEAD and resolves first"
            )
        );
    }

    #[test]
    fn quiet_flag_works_after_subcommand() {
        // #global-flags: --quiet and --color are global, so they should parse
        // both before and after the subcommand name, matching cargo/git convention.
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "check",
            "--quiet",
            "--mode",
            "no-new",
        ]))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("CLI should parse --quiet after subcommand: {err}"))
        });

        assert!(parsed.quiet);
        assert!(matches!(parsed.command, Some(CargoAllowCommand::Check(_))));
    }

    #[test]
    fn color_flag_works_after_subcommand() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "check",
            "--color=never",
            "--mode",
            "no-new",
        ]))
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("CLI should parse --color after subcommand: {err}"))
        });

        assert_eq!(parsed.color, ColorChoice::Never);
    }

    #[test]
    fn add_rejects_path_without_line() {
        let result = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "add",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--owner",
            "core",
            "--reason",
            "test",
        ]));
        assert!(
            result.is_err(),
            "--path without --line should fail at parse time"
        );
        let err = result.expect_err("should error");
        assert!(
            err.to_string().contains("--line"),
            "error should mention --line: {err}"
        );
    }

    #[test]
    fn add_rejects_line_without_path() {
        let result = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "add",
            "--kind",
            "panic",
            "--line",
            "42",
            "--owner",
            "core",
            "--reason",
            "test",
        ]));
        assert!(
            result.is_err(),
            "--line without --path should fail at parse time"
        );
        let err = result.expect_err("should error");
        assert!(
            err.to_string().contains("--path"),
            "error should mention --path: {err}"
        );
    }

    fn argv(items: Vec<&str>) -> Vec<String> {
        items.into_iter().map(String::from).collect()
    }
}
