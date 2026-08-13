use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use clap::{Command, CommandFactory, Parser, ValueEnum};
use serde::Serialize;
use std::path::PathBuf;

use crate::cli::CargoAllowCli;
use crate::emit_text;

/// Generate a deterministic command reference from the installed binary's
/// Clap command graph and the checked product support matrix.
///
/// The reference deliberately projects only product-level support/channel
/// facts here. The matrix remains the source of truth; this command does not
/// duplicate per-command support annotations or release qualification claims.
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
    #[value(name = "man", alias = "manpage")]
    Man,
}

#[derive(Debug, Serialize)]
struct CliReference {
    schema: &'static str,
    name: String,
    version: &'static str,
    support: SupportReference,
    about: Option<String>,
    usage: String,
    arguments: Vec<ArgumentReference>,
    commands: Vec<CommandReference>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct SupportReference {
    source: &'static str,
    schema: String,
    published_version: String,
    candidate_version: String,
    channels: Vec<ChannelReference>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct ChannelReference {
    name: String,
    available: bool,
    command: Option<String>,
    evidence: String,
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
    let reference = build_reference()?;
    let rendered = match args.format {
        ReferenceFormat::Markdown => render_markdown(&reference),
        ReferenceFormat::Json => render_json(&reference)?,
        ReferenceFormat::Man => render_manpage(&reference),
    };
    emit_text(args.output.as_deref(), &rendered)
}

#[cfg(test)]
const SUPPORT_MATRIX: &str = ::std::include_str!("../../../docs/support-matrix.toml");
const PACKAGE_MANIFEST: &str = ::std::include_str!("../Cargo.toml");
const SUPPORT_MATRIX_SOURCE: &str = "docs/support-matrix.toml";
const PACKAGE_SUPPORT_SOURCE: &str = "crates/cargo-allow/Cargo.toml package metadata";

fn build_reference() -> CargoAllowResult<CliReference> {
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

    Ok(CliReference {
        schema: "cargo-allow.cli-reference.v1",
        name,
        version: env!("CARGO_PKG_VERSION"),
        support: parse_packaged_support_reference(PACKAGE_MANIFEST)?,
        about,
        usage,
        arguments,
        commands,
    })
}

#[cfg(test)]
fn parse_support_reference(source: &str) -> CargoAllowResult<SupportReference> {
    let document = parse_toml(source, SUPPORT_MATRIX_SOURCE)?;
    let tool = required_table(&document, "tool", "support matrix")?;
    parse_support_table(&document, tool, &document, SUPPORT_MATRIX_SOURCE)
}

fn parse_packaged_support_reference(source: &str) -> CargoAllowResult<SupportReference> {
    let document = parse_toml(source, PACKAGE_SUPPORT_SOURCE)?;
    let package = required_table(&document, "package", PACKAGE_SUPPORT_SOURCE)?;
    let metadata = required_table(package, "metadata", PACKAGE_SUPPORT_SOURCE)?;
    let cargo_allow = required_table(metadata, "cargo-allow", PACKAGE_SUPPORT_SOURCE)?;
    let reference = required_table(cargo_allow, "reference", PACKAGE_SUPPORT_SOURCE)?;
    parse_support_table(reference, reference, reference, SUPPORT_MATRIX_SOURCE)
}

fn parse_toml(source: &str, source_name: &str) -> CargoAllowResult<toml::Table> {
    toml::from_str(source)
        .map_err(|error| reference_internal(format!("failed to parse {source_name}: {error}")))
}

fn reference_internal(message: impl Into<String>) -> CargoAllowError {
    CargoAllowError::with_kind(CargoAllowErrorKind::Internal, message)
}

fn parse_support_table(
    table: &toml::Table,
    tool: &toml::Table,
    channels: &toml::Table,
    source_name: &str,
) -> CargoAllowResult<SupportReference> {
    let schema = required_string(table, "schema_id", "support matrix")
        .or_else(|_| required_string(table, "schema", "packaged support metadata"))?;
    let published_version = required_string(tool, "published_version", "support matrix tool")?;
    let candidate_version = required_string(tool, "candidate_version", "support matrix tool")?;
    let channel_values = channels
        .get("channel")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| {
            reference_internal(format!(
                "{source_name} must define at least one [[channel]] row"
            ))
        })?;
    if channel_values.is_empty() {
        return Err(reference_internal(format!(
            "{source_name} must define at least one [[channel]] row"
        )));
    }
    let mut channels = Vec::with_capacity(channel_values.len());
    for (index, value) in channel_values.iter().enumerate() {
        let context = format!("support matrix channel {index}");
        let channel = value.as_table().ok_or_else(|| {
            reference_internal(format!("{source_name} {context} must be a table"))
        })?;
        channels.push(ChannelReference {
            name: required_string(channel, "name", &context)?,
            available: channel
                .get("available")
                .and_then(toml::Value::as_bool)
                .ok_or_else(|| {
                    reference_internal(format!(
                        "{source_name} {context} is missing boolean `available`"
                    ))
                })?,
            command: channel
                .get("command")
                .map(|value| {
                    value.as_str().map(str::to_owned).ok_or_else(|| {
                        reference_internal(format!(
                            "{source_name} {context} has a non-string `command`"
                        ))
                    })
                })
                .transpose()?,
            evidence: required_string(channel, "evidence", &context)?,
        });
    }
    Ok(SupportReference {
        source: SUPPORT_MATRIX_SOURCE,
        schema,
        published_version,
        candidate_version,
        channels,
    })
}

fn required_table<'a>(
    table: &'a toml::Table,
    key: &str,
    context: &str,
) -> CargoAllowResult<&'a toml::Table> {
    table
        .get(key)
        .and_then(toml::Value::as_table)
        .ok_or_else(|| reference_internal(format!("{context} is missing table `{key}`")))
}

fn required_string(table: &toml::Table, key: &str, context: &str) -> CargoAllowResult<String> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| reference_internal(format!("{context} is missing string `{key}`")))
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
        .map_err(json_render_error)
}

fn json_render_error(error: serde_json::Error) -> CargoAllowError {
    CargoAllowError::with_kind(
        CargoAllowErrorKind::Artifact,
        format!("failed to render CLI reference JSON: {error}"),
    )
}

fn render_manpage(reference: &CliReference) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        ".TH CARGO-ALLOW 1 \"\" \"cargo-allow {}\" \"User Commands\"\n",
        roff_text(reference.version)
    ));
    out.push_str(".SH NAME\n");
    out.push_str("cargo-allow \\- source-tree exception ledger and policy scanner\n");
    out.push_str(".SH SYNOPSIS\n");
    out.push_str(&format!(".B {}\n", roff_text(&reference.usage)));
    out.push_str(".SH DESCRIPTION\n");
    out.push_str(&format!(
        "{}\n",
        roff_text(reference.about.as_deref().unwrap_or(""))
    ));
    render_man_support(&mut out, &reference.support);
    render_man_arguments(&mut out, &reference.arguments);
    out.push_str(".SH COMMANDS\n");
    for command in &reference.commands {
        render_man_command(&mut out, command);
    }
    out
}

fn render_man_support(out: &mut String, support: &SupportReference) {
    out.push_str(".SH SUPPORT\n");
    out.push_str(&format!(
        "Support source: {}\nSupport schema: {}\nPublished version: {}\nCandidate version: {}\n",
        roff_text(support.source),
        roff_text(&support.schema),
        roff_text(&support.published_version),
        roff_text(&support.candidate_version)
    ));
    for channel in &support.channels {
        out.push_str(&format!(
            "Channel {}: {}. Evidence: {}\n",
            roff_text(&channel.name),
            if channel.available {
                "available"
            } else {
                "not available"
            },
            roff_text(&channel.evidence)
        ));
        if let Some(command) = &channel.command {
            out.push_str(&format!("Command: {}\n", roff_text(command)));
        }
    }
}

fn render_man_arguments(out: &mut String, arguments: &[ArgumentReference]) {
    if arguments.is_empty() {
        return;
    }
    out.push_str(".SH OPTIONS\n");
    for argument in arguments {
        out.push_str(".TP\n");
        out.push_str(&format!(".B {}\n", roff_text(&argument_names(argument))));
        let mut details = argument.help.clone().unwrap_or_default();
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
        if !values.is_empty() {
            details.push_str(" Values: ");
            details.push_str(&values);
        }
        if !argument.default_values.is_empty() {
            details.push_str(" Default: ");
            details.push_str(&argument.default_values.join(", "));
        }
        out.push_str(&format!("{}\n", roff_text(&details)));
    }
}

fn render_man_command(out: &mut String, command: &CommandReference) {
    out.push_str(".TP\n");
    out.push_str(&format!(".B {}\n", roff_text(&command.path)));
    let mut details = command.about.clone().unwrap_or_default();
    if !details.is_empty() {
        details.push(' ');
    }
    details.push_str(&command.usage);
    out.push_str(&format!("{}\n", roff_text(&details)));
    for child in &command.commands {
        render_man_command(out, child);
    }
}

fn roff_text(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('-', "\\-")
        .replace(['\r', '\n'], " ");
    if escaped.starts_with('.') || escaped.starts_with('\'') {
        format!("\\&{escaped}")
    } else {
        escaped
    }
}

fn render_markdown(reference: &CliReference) -> String {
    let mut out = String::new();
    out.push_str("# cargo-allow command reference\n\n");
    out.push_str("Generated from the installed binary's Clap command graph.\n\n");
    out.push_str(&format!("- Version: `{}`\n", reference.version));
    out.push_str("- Machine format: `cargo-allow reference --format json`\n");
    out.push_str(&format!(
        "- Support source: `{}` (`{}`)\n",
        markdown_text(reference.support.source),
        markdown_text(&reference.support.schema)
    ));
    out.push_str(&format!(
        "- Published version: `{}`; candidate version: `{}`\n\n",
        markdown_text(&reference.support.published_version),
        markdown_text(&reference.support.candidate_version)
    ));
    out.push_str("## Support channels\n\n");
    out.push_str("| Channel | Available | Command | Evidence |\n| --- | ---: | --- | --- |\n");
    for channel in &reference.support.channels {
        out.push_str(&format!(
            "| `{}` | {} | {} | {} |\n",
            markdown_text(&channel.name),
            if channel.available { "yes" } else { "no" },
            channel
                .command
                .as_deref()
                .map(markdown_cell)
                .unwrap_or_default(),
            markdown_cell(&channel.evidence),
        ));
    }
    out.push_str("\nGenerated from the installed binary's exact Clap command graph; command grammar remains the reference's primary scope.\n\n");
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
    fn reference_covers_every_clap_subcommand_and_argument() -> Result<(), String> {
        let mut command = CargoAllowCli::command();
        command.build();
        let reference = build_reference().map_err(|error| error.to_string())?;
        assert_eq!(reference.name, command.get_name());
        assert_command_reference(&command, &reference.arguments, &reference.commands);
        Ok(())
    }

    #[test]
    fn support_reference_tracks_checked_matrix() -> Result<(), String> {
        let support = parse_packaged_support_reference(PACKAGE_MANIFEST)
            .map_err(|error| error.to_string())?;
        let repository_support =
            parse_support_reference(SUPPORT_MATRIX).map_err(|error| error.to_string())?;
        assert_eq!(support, repository_support);
        assert_eq!(support.source, SUPPORT_MATRIX_SOURCE);
        assert_eq!(support.schema, "cargo-allow.support-matrix.v1");
        assert_eq!(support.published_version, "0.1.11");
        assert_eq!(support.candidate_version, "0.2.0");
        assert_eq!(support.channels.len(), 3);
        assert_eq!(support.channels[0].name, "crates.io");
        assert!(support.channels[0].available);
        assert_eq!(
            support.channels[0].command.as_deref(),
            Some("cargo install cargo-allow --version 0.1.11 --locked")
        );
        assert!(!support.channels[1].available);
        Ok(())
    }

    #[test]
    fn support_reference_rejects_malformed_toml_as_internal() -> Result<(), String> {
        let error = parse_support_reference("[tool").expect_err("malformed TOML is invalid");
        assert_eq!(error.kind(), CargoAllowErrorKind::Internal);
        assert!(
            error
                .to_string()
                .contains("failed to parse docs/support-matrix.toml")
        );
        Ok(())
    }

    #[test]
    fn support_reference_rejects_structural_drift_as_internal() -> Result<(), String> {
        for (source, expected) in [
            (
                r#"
schema_id = "cargo-allow.support-matrix.v1"
[tool]
published_version = "0.1.11"
candidate_version = "0.2.0"
"#,
                "must define at least one [[channel]] row",
            ),
            (
                r#"
schema_id = "cargo-allow.support-matrix.v1"
channel = []
[tool]
published_version = "0.1.11"
candidate_version = "0.2.0"
"#,
                "must define at least one [[channel]] row",
            ),
            (
                r#"
schema_id = "cargo-allow.support-matrix.v1"
channel = ["not a table"]
[tool]
published_version = "0.1.11"
candidate_version = "0.2.0"
"#,
                "must be a table",
            ),
            (
                r#"
schema_id = "cargo-allow.support-matrix.v1"
[tool]
published_version = "0.1.11"
candidate_version = "0.2.0"
[[channel]]
name = "test"
evidence = "test"
"#,
                "missing boolean `available`",
            ),
        ] {
            let error = parse_support_reference(source).expect_err("structural drift is invalid");
            assert_eq!(error.kind(), CargoAllowErrorKind::Internal);
            assert!(error.to_string().contains(expected));
        }

        let error = parse_support_reference("").expect_err("missing tool table is invalid");
        assert_eq!(error.kind(), CargoAllowErrorKind::Internal);
        assert!(error.to_string().contains("missing table `tool`"));
        Ok(())
    }

    #[test]
    fn support_reference_rejects_missing_channel_evidence() -> Result<(), String> {
        let error = parse_support_reference(
            r#"
schema_id = "cargo-allow.support-matrix.v1"
[tool]
published_version = "0.1.11"
candidate_version = "0.2.0"
[[channel]]
name = "test"
available = true
"#,
        )
        .expect_err("support channel evidence is required");
        assert_eq!(error.kind(), CargoAllowErrorKind::Internal);
        assert!(error.to_string().contains("missing string `evidence`"));
        Ok(())
    }

    #[test]
    fn support_reference_rejects_non_string_channel_command() -> Result<(), String> {
        let error = parse_support_reference(
            r#"
schema_id = "cargo-allow.support-matrix.v1"
[tool]
published_version = "0.1.11"
candidate_version = "0.2.0"
[[channel]]
name = "test"
available = true
command = 42
evidence = "test"
"#,
        )
        .expect_err("non-string command must be rejected");
        assert_eq!(error.kind(), CargoAllowErrorKind::Internal);
        assert!(error.to_string().contains("non-string `command`"));
        Ok(())
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
    fn json_reference_is_deterministic_and_machine_parseable() -> Result<(), String> {
        let reference = build_reference().map_err(|error| error.to_string())?;
        let first = render_json(&reference).map_err(|error| error.to_string())?;
        let second = render_json(&reference).map_err(|error| error.to_string())?;
        assert_eq!(first, second);
        let value: Value = serde_json::from_str(&first).map_err(|error| error.to_string())?;
        assert_eq!(value["schema"], "cargo-allow.cli-reference.v1");
        assert_eq!(value["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(value["support"]["schema"], "cargo-allow.support-matrix.v1");
        assert_eq!(value["support"]["published_version"], "0.1.11");
        assert!(
            value["commands"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
        );
        Ok(())
    }

    #[test]
    fn json_render_errors_are_artifacts() -> Result<(), String> {
        let serialization_error = serde_json::from_str::<Value>("{")
            .expect_err("incomplete JSON should produce a serialization error");
        let error = json_render_error(serialization_error);
        assert_eq!(error.kind(), CargoAllowErrorKind::Artifact);
        assert!(
            error
                .to_string()
                .contains("failed to render CLI reference JSON")
        );
        Ok(())
    }

    #[test]
    fn markdown_reference_contains_support_and_first_hour_commands() -> Result<(), String> {
        let reference = build_reference().map_err(|error| error.to_string())?;
        let markdown = render_markdown(&reference);
        for command in ["init", "check", "audit", "list", "explain", "doctor"] {
            assert!(markdown.contains(&format!("`cargo-allow {command}`")));
        }
        assert!(!markdown.contains(env!("CARGO_MANIFEST_DIR")));
        assert!(markdown.contains("--color"));
        assert!(markdown.contains("--format"));
        assert!(markdown.contains("## Support channels"));
        assert!(markdown.contains("`docs/support-matrix.toml`"));
        assert!(markdown.contains("`crates.io` | yes"));
        assert!(!markdown.contains('\r'));
        Ok(())
    }

    #[test]
    fn manpage_is_deterministic_and_covers_support_options_and_commands() -> Result<(), String> {
        let reference = build_reference().map_err(|error| error.to_string())?;
        let first = render_manpage(&reference);
        let second = render_manpage(&reference);
        assert_eq!(first, second);
        assert!(first.starts_with(".TH CARGO-ALLOW 1"));
        assert!(first.contains(".SH OPTIONS"));
        assert!(first.contains(".SH COMMANDS"));
        assert!(first.contains("cargo\\-allow check"));
        assert!(first.contains("\\-\\-color"));
        assert!(first.contains(".SH SUPPORT"));
        assert!(first.contains("docs/support\\-matrix.toml"));
        assert!(first.contains("Support schema: cargo\\-allow.support\\-matrix.v1"));
        assert!(first.contains("Command: cargo install cargo\\-allow"));
        assert!(!first.contains(env!("CARGO_MANIFEST_DIR")));
        assert!(!first.contains('\r'));
        Ok(())
    }
}
