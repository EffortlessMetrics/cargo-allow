use std::path::PathBuf;

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use clap::{Command, CommandFactory, Parser};
use clap_complete::Shell;

use crate::cli::CargoAllowCli;
use crate::emit_text;

/// Generate a shell completion script for cargo-allow.
///
/// The script is produced from the same clap command graph the binary parses
/// with, so completions cannot drift from the real flags and subcommands.
///
/// Write the script somewhere your shell loads at startup:
///
/// ```text
/// # bash
/// cargo-allow completions bash > ~/.local/share/bash-completion/completions/cargo-allow
///
/// # zsh (a directory already on your $fpath)
/// cargo-allow completions zsh > ~/.zfunc/_cargo-allow
///
/// # fish
/// cargo-allow completions fish > ~/.config/fish/completions/cargo-allow.fish
///
/// # powershell (append to your profile)
/// cargo-allow completions powershell >> $PROFILE
/// ```
///
/// `elvish` is also supported. Run with `--help` for the current list.
///
/// Completions are for the `cargo-allow` binary. Invoked through cargo as
/// `cargo allow ...`, argument completion is handled by cargo, not by this
/// script.
#[derive(Debug, Clone, Parser)]
pub(crate) struct CompletionsArgs {
    /// Shell to generate a completion script for.
    #[arg(value_enum)]
    pub(crate) shell: Shell,
    /// Write the script to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

pub(crate) fn cmd_completions(args: &CompletionsArgs) -> CargoAllowResult<()> {
    emit_text(args.output.as_deref(), &render_completions(args.shell)?)?;
    Ok(())
}

fn render_completions(shell: Shell) -> CargoAllowResult<String> {
    let mut command = visible_command(&CargoAllowCli::command());
    let mut buffer: Vec<u8> = Vec::new();
    clap_complete::generate(shell, &mut command, "cargo-allow", &mut buffer);
    // `generate` writes shell script text, so this conversion does not fail in
    // practice. Kept fallible rather than unwrapped: this is a panic-scanning
    // tool, and a "cannot happen" is not a reason to panic in its own binary.
    String::from_utf8(buffer).map_err(completion_render_error)
}

fn completion_render_error(error: std::string::FromUtf8Error) -> CargoAllowError {
    CargoAllowError::with_kind(CargoAllowErrorKind::Artifact, format!("not UTF-8: {error}"))
}

/// Rebuild the clap command graph with hidden subcommands and hidden arguments
/// removed.
///
/// `clap_complete`'s AOT generators walk `get_subcommands()` and
/// `get_arguments()` without checking `is_hide_set()`, so handing the live
/// graph straight to `generate` leaks every `#[command(hide = true)]` command
/// (for example `release-identity`) into all five shell scripts. clap 4.6 has
/// no public API to remove a subcommand from a built `Command`, so this
/// rebuilds the tree from the public getters, keeping only visible nodes.
fn visible_command(command: &Command) -> Command {
    let mut filtered = Command::new(command.get_name().to_string())
        .about(resettable(command.get_about().cloned()))
        .long_about(resettable(command.get_long_about().cloned()))
        .version(resettable(
            command
                .get_version()
                .map(|value| clap::builder::Str::from(value.to_string())),
        ))
        .author(resettable(
            command
                .get_author()
                .map(|value| clap::builder::Str::from(value.to_string())),
        ))
        .visible_aliases(command.get_visible_aliases().map(str::to_owned))
        .aliases(command.get_aliases().map(str::to_owned))
        .args(
            command
                .get_arguments()
                .filter(|argument| !argument.is_hide_set())
                .cloned(),
        )
        .groups(command.get_groups().cloned())
        .subcommands(
            command
                .get_subcommands()
                .filter(|subcommand| !subcommand.is_hide_set())
                .map(visible_command),
        )
        .allow_external_subcommands(command.is_allow_external_subcommands_set())
        .disable_help_subcommand(command.is_disable_help_subcommand_set())
        .subcommand_required(command.is_subcommand_required_set())
        .arg_required_else_help(command.is_arg_required_else_help_set())
        .propagate_version(command.is_propagate_version_set());
    if let Some(value_name) = command.get_subcommand_value_name() {
        filtered = filtered.subcommand_value_name(value_name.to_string());
    }
    if let Some(heading) = command.get_subcommand_help_heading() {
        filtered = filtered.subcommand_help_heading(heading.to_string());
    }
    filtered
}

fn resettable<T>(value: Option<T>) -> clap::builder::Resettable<T> {
    match value {
        Some(value) => clap::builder::Resettable::Value(value),
        None => clap::builder::Resettable::Reset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_render_errors_are_artifacts() -> Result<(), String> {
        let utf8_error = String::from_utf8(vec![0xff])
            .expect_err("invalid UTF-8 should produce a conversion error");
        let error = completion_render_error(utf8_error);
        assert_eq!(error.kind(), CargoAllowErrorKind::Artifact);
        assert!(error.to_string().contains("not UTF-8"));
        Ok(())
    }

    /// Every supported shell produces a non-empty script naming the binary.
    /// Generation is driven by the live command graph, so this also fails if
    /// the root command is renamed out from under the completion output.
    #[test]
    fn every_shell_renders_a_script_naming_the_binary() {
        for shell in [
            Shell::Bash,
            Shell::Zsh,
            Shell::Fish,
            Shell::PowerShell,
            Shell::Elvish,
        ] {
            let script = render_completions(shell)
                .unwrap_or_else(|err| std::panic::panic_any(format!("{shell}: {err}")));
            assert!(
                !script.trim().is_empty(),
                "{shell} completion script was empty"
            );
            assert!(
                script.contains("cargo-allow"),
                "{shell} completion script should name the binary"
            );
        }
    }

    /// The point of generating from the clap graph is that completions list
    /// the real subcommands. Spot-check the ones an operator reaches first.
    #[test]
    fn scripts_include_the_first_hour_subcommands() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            let script = render_completions(shell)
                .unwrap_or_else(|err| std::panic::panic_any(format!("{shell}: {err}")));
            for subcommand in ["init", "check", "audit", "why", "add", "doctor"] {
                assert!(
                    script.contains(subcommand),
                    "{shell} completion script should mention `{subcommand}`"
                );
            }
        }
    }

    /// Every `#[command(hide = true)]` subcommand must stay out of all five
    /// completion scripts. This is the regression guard for the rc.1
    /// `E.completions.entrypoint` leak: `clap_complete` does not filter
    /// `is_hide_set()` itself, so the generator must hand it a filtered tree.
    #[test]
    fn hidden_commands_never_reach_a_completion_script() -> Result<(), String> {
        let command = CargoAllowCli::command();
        let hidden = hidden_names(&command);
        if !hidden.iter().any(|name| name == "release-identity") {
            return Err("the fixture should include the named release-identity leak".to_string());
        }
        for shell in [
            Shell::Bash,
            Shell::Zsh,
            Shell::Fish,
            Shell::PowerShell,
            Shell::Elvish,
        ] {
            let script = render_completions(shell).map_err(|err| format!("{shell}: {err}"))?;
            for name in &hidden {
                if script.contains(name.as_str()) {
                    return Err(format!(
                        "{shell} completion script leaked hidden name `{name}`"
                    ));
                }
            }
        }
        Ok(())
    }

    fn hidden_names(command: &Command) -> Vec<String> {
        let mut names = Vec::new();
        for argument in command.get_arguments() {
            let Some(long) = argument.get_long() else {
                continue;
            };
            if argument.is_hide_set() {
                names.push(long.to_string());
            }
        }
        for subcommand in command.get_subcommands() {
            if subcommand.is_hide_set() {
                names.push(subcommand.get_name().to_string());
            } else {
                names.extend(hidden_names(subcommand));
            }
        }
        names
    }

    /// Completions are consumed by `source`/`eval`, so a stray panic or a
    /// half-written script is worse than none. Generation must be pure and
    /// repeatable.
    #[test]
    fn generation_is_deterministic() {
        for shell in [
            Shell::Bash,
            Shell::Zsh,
            Shell::Fish,
            Shell::PowerShell,
            Shell::Elvish,
        ] {
            let first = render_completions(shell)
                .unwrap_or_else(|err| std::panic::panic_any(format!("{shell}: {err}")));
            let second = render_completions(shell)
                .unwrap_or_else(|err| std::panic::panic_any(format!("{shell}: {err}")));
            assert_eq!(first, second, "{shell} completion script was not stable");
        }
    }
}
