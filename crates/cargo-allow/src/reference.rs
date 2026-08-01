use allow_core::{CargoAllowError, CargoAllowResult};
use clap::{Command, CommandFactory, Parser, ValueEnum};
use serde::Serialize;
use std::path::PathBuf;

use crate::cli::CargoAllowCli;
use crate::emit_text;

/// Generate a deterministic command reference from the installed binary's
/// Clap command graph.
///
/// This command documents CLI grammar only. Support channels, mutation
/// posture, and artifact contracts remain owned by their respective guides
/// and registries until the broader #2479 release/reference work lands.
#[derive(Debug, Clone, Parser)]
pub(crate) struct ReferenceArgs {
    /// Reference format.
    #[arg(long, value_enum, default_value_t = ReferenceFormat::Markdown)]
    pub(crate) format: ReferenceFormat,
    /// Write the reference to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ReferenceFormat {
    #[value(alias = "md")]
    Markdown,
    Json,
}

#[derive(Debug, Serialize)]
struct CliReference {
    schema: &'static str,
    name: String,
    version: &'static str,
    about: Option<String>,
    usage: String,
    arguments: Vec<ArgumentReference>,
    commands: Vec<CommandReference>,
}

#[derive(Debug, Serialize)]
struct CommandReference {
    name: String,
    path: String,
    aliases: Vec<String>,
    about: Option<String>,
    usage: String,
    arguments: Vec<ArgumentReference>,
    commands: Vec<CommandReference>,
}

#[derive(Debug, Serialize)]
struct ArgumentReference {
    id: String,
    short: Option<String>,
    long: Option<String>,
    help: Option<String>,
    required: bool,
    value_names: Vec<String>,
    default_values: Vec<String>,
    possible_values: Vec<PossibleValueReference>,
}

#[derive(Debug, Serialize)]
struct PossibleValueReference {
    name: String,
    aliases: Vec<String>,
    help: Option<String>,
}

pub(crate) fn cmd_reference(args: &ReferenceArgs) -> CargoAllowResult<()> {
    let reference = build_reference();
    let rendered = match args.format {
        ReferenceFormat::Markdown => render_markdown(&reference),
        ReferenceFormat::Json => render_json(&reference)?,
    };
    emit_text(args.output.as_deref(), &rendered)
}

fn build_reference() -> CliReference {
    let mut command = CargoAllowCli::command();
    command.build();
    let name = command.get_name().to_string();
    let about = command.get_about().map(ToString::to_string);
    let usage = command_usage(&command, &name);
    let arguments = collect_arguments(&command);
    let commands = command
        .get_subcommands()
        .filter(|subcommand| !subcommand.is_hide_set())
        .map(|subcommand| collect_command(subcommand, &name))
        .collect();

    CliReference {
        schema: "cargo-allow.cli-reference.v1",
        name,
        version: env!("CARGO_PKG_VERSION"),
        about,
        usage,
        arguments,
        commands,
    }
}

fn collect_command(command: &Command, parent_path: &str) -> CommandReference {
    let name = command.get_name().to_string();
    let path = format!("{parent_path} {name}");
    let aliases = command
        .get_name_and_visible_aliases()
        .into_iter()
        .skip(1)
        .map(str::to_owned)
        .collect();
    let about = command.get_about().map(ToString::to_string);
    let usage = command_usage(command, &path);
    let arguments = collect_arguments(command);
    let commands = command
        .get_subcommands()
        .filter(|subcommand| !subcommand.is_hide_set())
        .map(|subcommand| collect_command(subcommand, &path))
        .collect();

    CommandReference {
        name,
        path,
        aliases,
        about,
        usage,
        arguments,
        commands,
    }
}

fn command_usage(command: &Command, path: &str) -> String {
    let mut command_for_usage = command.clone().bin_name(path);
    command_for_usage.render_usage().to_string()
}

fn collect_arguments(command: &Command) -> Vec<ArgumentReference> {
    command
        .get_arguments()
        .filter(|argument| !argument.is_hide_set())
        .map(|argument| ArgumentReference {
            id: argument.get_id().to_string(),
            short: argument.get_short().map(|short| short.to_string()),
            long: argument.get_long().map(str::to_owned),
            help: argument.get_help().map(ToString::to_string),
            required: argument.is_required_set(),
            value_names: argument
                .get_value_names()
                .unwrap_or_default()
                .iter()
                .map(ToString::to_string)
                .collect(),
            default_values: argument
                .get_default_values()
                .iter()
                .map(|value| value.to_string_lossy().into_owned())
                .collect(),
            possible_values: argument
                .get_possible_values()
                .into_iter()
                .filter(|value| !value.is_hide_set())
                .map(|value| PossibleValueReference {
                    name: value.get_name().to_string(),
                    aliases: value
                        .get_name_and_aliases()
                        .skip(1)
                        .map(str::to_owned)
                        .collect(),
                    help: value.get_help().map(ToString::to_string),
                })
                .collect(),
        })
        .collect()
}

fn render_json(reference: &CliReference) -> CargoAllowResult<String> {
    serde_json::to_string_pretty(reference)
        .map(|json| format!("{json}\n"))
        .map_err(|err| CargoAllowError::new(format!("failed to render CLI reference JSON: {err}")))
}

fn render_markdown(reference: &CliReference) -> String {
    let mut out = String::new();
    out.push_str("# cargo-allow command reference\n\n");
    out.push_str("Generated from the installed binary's Clap command graph.\n\n");
    out.push_str(&format!("- Version: `{}`\n", reference.version));
    out.push_str("- Machine format: `cargo-allow reference --format json`\n");
    out.push_str("- Scope: exact CLI grammar; support and release state remain in the published command registry.\n\n");
    render_command_markdown(
        &mut out,
        &reference.name,
        reference.about.as_deref(),
        &reference.usage,
        &reference.arguments,
    );
    for command in &reference.commands {
        render_command_tree_markdown(&mut out, command);
    }
    out
}

fn render_command_tree_markdown(out: &mut String, command: &CommandReference) {
    render_command_markdown(
        out,
        &command.path,
        command.about.as_deref(),
        &command.usage,
        &command.arguments,
    );
    for child in &command.commands {
        render_command_tree_markdown(out, child);
    }
}

fn render_command_markdown(
    out: &mut String,
    heading: &str,
    about: Option<&str>,
    usage: &str,
    arguments: &[ArgumentReference],
) {
    out.push_str(&format!("## `{heading}`\n\n"));
    if let Some(about) = about {
        out.push_str(&format!("{}\n\n", markdown_text(about)));
    }
    out.push_str("```text\n");
    out.push_str(usage.trim());
    out.push_str("\n```\n\n");
    if arguments.is_empty() {
        return;
    }
    out.push_str("| Argument | Required | Values | Default | Description |\n");
    out.push_str("| --- | ---: | --- | --- | --- |\n");
    for argument in arguments {
        let names = argument_names(argument);
        let values = if argument.possible_values.is_empty() {
            argument.value_names.join(", ")
        } else {
            argument
                .possible_values
                .iter()
                .flat_map(|value| {
                    std::iter::once(value.name.as_str())
                        .chain(value.aliases.iter().map(String::as_str))
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        let default = argument.default_values.join(", ");
        let help = argument.help.as_deref().unwrap_or("");
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            markdown_text(&names),
            if argument.required { "yes" } else { "no" },
            markdown_values(&values),
            markdown_values(&default),
            markdown_cell(help),
        ));
    }
    out.push('\n');
}

fn argument_names(argument: &ArgumentReference) -> String {
    match (&argument.short, &argument.long) {
        (Some(short), Some(long)) => format!("-{short}, --{long}"),
        (Some(short), None) => format!("-{short}"),
        (None, Some(long)) => format!("--{long}"),
        (None, None) => argument.id.clone(),
    }
}

fn markdown_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('|', "\\|")
        .replace(['\r', '\n'], " ")
}

fn markdown_cell(value: &str) -> String {
    markdown_text(value).trim().to_string()
}

fn markdown_values(value: &str) -> String {
    value
        .split(", ")
        .filter(|item| !item.is_empty())
        .map(|item| format!("`{}`", markdown_text(item)))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use serde_json::Value;

    #[test]
    fn reference_covers_every_clap_subcommand_and_argument() {
        let mut command = CargoAllowCli::command();
        command.build();
        let reference = build_reference();
        assert_eq!(reference.name, command.get_name());
        assert_command_reference(&command, &reference.arguments, &reference.commands);
    }

    fn assert_command_reference(
        command: &Command,
        arguments: &[ArgumentReference],
        commands: &[CommandReference],
    ) {
        let expected_arguments = command
            .get_arguments()
            .filter(|argument| !argument.is_hide_set())
            .count();
        assert_eq!(
            arguments.len(),
            expected_arguments,
            "argument count drift for `{}`",
            command.get_name()
        );
        let expected_commands = command
            .get_subcommands()
            .filter(|subcommand| !subcommand.is_hide_set())
            .count();
        assert_eq!(
            commands.len(),
            expected_commands,
            "subcommand count drift for `{}`",
            command.get_name()
        );
        for subcommand in command
            .get_subcommands()
            .filter(|subcommand| !subcommand.is_hide_set())
        {
            let documented = commands
                .iter()
                .find(|candidate| candidate.name == subcommand.get_name())
                .unwrap_or_else(|| {
                    std::panic::panic_any(format!(
                        "CLI reference is missing `{}`",
                        subcommand.get_name()
                    ))
                });
            assert_command_reference(subcommand, &documented.arguments, &documented.commands);
        }
    }

    #[test]
    fn json_reference_is_deterministic_and_machine_parseable() {
        let reference = build_reference();
        let first = render_json(&reference)
            .unwrap_or_else(|err| std::panic::panic_any(format!("render JSON: {err}")));
        let second = render_json(&reference)
            .unwrap_or_else(|err| std::panic::panic_any(format!("render JSON: {err}")));
        assert_eq!(first, second);
        let value: Value = serde_json::from_str(&first)
            .unwrap_or_else(|err| std::panic::panic_any(format!("parse JSON: {err}")));
        assert_eq!(value["schema"], "cargo-allow.cli-reference.v1");
        assert_eq!(value["version"], env!("CARGO_PKG_VERSION"));
        assert!(
            value["commands"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
        );
    }

    #[test]
    fn markdown_reference_contains_first_hour_commands_and_no_checkout_path() {
        let markdown = render_markdown(&build_reference());
        for command in ["init", "check", "audit", "list", "explain", "doctor"] {
            assert!(markdown.contains(&format!("`cargo-allow {command}`")));
        }
        assert!(!markdown.contains(env!("CARGO_MANIFEST_DIR")));
        assert!(markdown.contains("--color"));
        assert!(markdown.contains("--format"));
        assert!(!markdown.contains('\r'));
    }
}
