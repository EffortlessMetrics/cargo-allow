use std::path::PathBuf;

use allow_core::{CargoAllowError, CargoAllowResult};
use clap::{CommandFactory, Parser};
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
    let mut command = CargoAllowCli::command();
    let mut buffer: Vec<u8> = Vec::new();
    clap_complete::generate(shell, &mut command, "cargo-allow", &mut buffer);
    String::from_utf8(buffer).map_err(|err| {
        CargoAllowError::new(format!("completion script was not valid UTF-8: {err}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
