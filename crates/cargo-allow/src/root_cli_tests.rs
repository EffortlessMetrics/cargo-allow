use std::path::Path;

use crate::*;

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use clap::Parser;

    #[test]
    fn normalized_args_accepts_cargo_subcommand_prefix() {
        let normalized = normalized_args(argv(vec!["cargo-allow", "allow", "audit"]));
        let expected = argv(vec!["cargo-allow", "audit"]);
        assert_eq!(normalized, expected);
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
    fn clap_parses_markdown_alias() {
        let parsed =
            CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "check", "--format", "md"]))
                .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Check(check::CheckArgs {
                format: OutputFormat::Markdown,
                ..
            }))
        ));
    }

    #[test]
    fn clap_requires_diff_base() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec!["cargo-allow", "diff"]));

        assert!(parsed.is_err());
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
                mode: Some(mode),
                ..
            })) if mode == "strict"
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

        assert!(help.contains("Source exception ledger for source trees"));

        let Some(check) = command.find_subcommand_mut("check") else {
            std::panic::panic_any("check subcommand should exist");
        };
        let help = check.render_help().to_string();

        assert!(help.contains("policy-configured source-tree gate mode"));
        assert!(!help.contains("workspace.default_mode"));
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

        assert!(help.contains(
            "Base git revision for policy, finding, and changed-file posture comparison"
        ));
        assert!(help.contains("Optional head git revision. Defaults to the current working tree"));
    }

    fn argv(items: Vec<&str>) -> Vec<String> {
        items.into_iter().map(String::from).collect()
    }
}
