mod catalog;
mod command;
mod discovery;
mod input;
mod orchestration;
mod output;

use clap::{Parser, error::ErrorKind};
use command::Cli;
use output::{Failure, Outcome};
#[tokio::main]
async fn main() -> std::process::ExitCode {
    let args = std::env::args_os().collect::<Vec<_>>();
    let json_mode = args.iter().any(|a| a == "--json");
    let attempted_command = args
        .iter()
        .skip(1)
        .find(|arg| !arg.to_string_lossy().starts_with('-'))
        .map(|arg| arg.to_string_lossy().into_owned())
        .unwrap_or_else(|| "fellersim".into());
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) => {
            let (command, outcome) = match e.kind() {
                ErrorKind::DisplayHelp => (
                    "help",
                    Outcome::success(
                        serde_json::json!({"text":e.to_string(), "discovery":discovery::describe(None).unwrap()}),
                    ),
                ),
                ErrorKind::DisplayVersion => ("version", Outcome::success(output::versions())),
                _ => (
                    attempted_command.as_str(),
                    Outcome::failed(Failure::input("invalid-arguments", e.to_string(), "")),
                ),
            };
            return output::emit(command, json_mode, outcome).into();
        }
    };
    let command = cli.command.identity();
    let outcome = match orchestration::execute(cli.command, cli.quiet).await {
        Ok(o) => o,
        Err(f) => Outcome::failed(f),
    };
    output::emit(command, cli.json, outcome).into()
}
