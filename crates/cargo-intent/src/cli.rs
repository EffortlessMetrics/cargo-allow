use cargo_intent::{
    IdentityFrameV1, IntentConfigV1, OutputFormat, ProcessExitFamilyV1, ProductIdentityV1,
    emit_frame, exit_code_for_family, load_config,
};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "cargo-intent",
    about = "Durable authored intent and obligation compiler",
    version,
    propagate_version = true
)]
pub struct CargoIntentCli {
    /// Repository root for intent evaluation.
    #[arg(long, default_value = ".")]
    pub root: PathBuf,
    /// Optional explicit config path (default: .allow/intent.toml).
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Human or JSON output.
    #[arg(long, value_enum, default_value_t = FormatArg::Human)]
    pub format: FormatArg,
    #[command(subcommand)]
    pub command: Option<CargoIntentCommand>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FormatArg {
    Human,
    Json,
}

impl From<FormatArg> for OutputFormat {
    fn from(value: FormatArg) -> Self {
        match value {
            FormatArg::Human => Self::Human,
            FormatArg::Json => Self::Json,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum CargoIntentCommand {
    /// Identity and capability surface.
    Identity,
    /// Change-oriented intent commands (verticals land in #2599-B).
    Change,
}

pub fn run() -> Result<ProcessExitFamilyV1, String> {
    let cli = CargoIntentCli::parse();
    let config = load_config(&cli.root, cli.config.as_deref())?;
    let output_format = OutputFormat::from(cli.format);
    match cli.command {
        None => {
            print_help()?;
            Ok(ProcessExitFamilyV1::Success)
        }
        Some(CargoIntentCommand::Identity) => {
            cmd_identity(&config, output_format)?;
            Ok(ProcessExitFamilyV1::Success)
        }
        Some(CargoIntentCommand::Change) => Err(
            "change commands are not available yet; see #2599-B for the first vertical"
                .to_string(),
        ),
    }
}

fn cmd_identity(config: &IntentConfigV1, format: OutputFormat) -> Result<(), String> {
    let identity = ProductIdentityV1::current(env!("CARGO_PKG_VERSION"));
    let frame = IdentityFrameV1::from_identity(&identity);
    let _profile = config.profile;
    let rendered = emit_frame(&frame, format)?;
    print!("{rendered}");
    Ok(())
}

fn print_help() -> Result<(), String> {
    use clap::CommandFactory;
    CargoIntentCli::command()
        .print_help()
        .map_err(|err| format!("print help: {err}"))?;
    println!();
    Ok(())
}

pub fn main_exit_code(result: Result<ProcessExitFamilyV1, String>) -> i32 {
    match result {
        Ok(family) => exit_code_for_family(family),
        Err(message) => {
            eprintln!("error: {message}");
            exit_code_for_family(ProcessExitFamilyV1::Usage)
        }
    }
}
