#[path = "cli.rs"]
mod cli;

fn main() {
    std::process::exit(cli::main_exit_code(cli::run()));
}
