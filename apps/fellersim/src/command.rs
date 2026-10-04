use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "fellersim",
    about = "Discover, inspect, and simulate Fellowship builds offline",
    version
)]
pub struct Cli {
    /// Return results, errors, and help as JSON.
    #[arg(long, global = true)]
    pub json: bool,
    /// Hide progress messages; errors are still shown.
    #[arg(long, global = true)]
    pub quiet: bool,
    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn command_with_help() -> clap::Command {
        let mut command = Self::command();
        command.build();
        let mut reference = String::from(
            "Shared options:\n  Use --json and --quiet with any command; place them before the help command.\n\n\
             An action priority list (APL) controls which abilities a character uses.\n\n\
             Command reference:\n",
        );
        append_command_help(&command, &mut reference);
        reference.push_str(
            "\nExamples:\n  fellersim describe --json\n  fellersim run --help\n  fellersim character init --help",
        );
        command.after_help(reference)
    }
}

fn append_command_help(command: &clap::Command, reference: &mut String) {
    for child in command
        .get_subcommands()
        .filter(|child| !child.is_hide_set() && child.get_name() != "help")
    {
        let mut display = child.clone();
        reference.push('\n');
        reference.push_str(&display.render_usage().to_string());
        reference.push('\n');
        // Hide shared flags only in these display copies. Parsing, focused help,
        // and JSON discovery retain the original command definitions.
        display = display
            .mut_args(|arg| {
                if arg.is_global_set() || arg.get_id() == "help" {
                    arg.hide(true)
                } else {
                    arg
                }
            })
            .help_template("{about}\n\n{all-args}");
        reference.push_str(display.render_help().to_string().trim_end());
        reference.push('\n');
        append_command_help(child, reference);
    }
}

#[derive(Subcommand)]
pub enum Command {
    /// Simulate a character's damage over repeated fights.
    Run(Execution),
    /// Check a character, simulation settings, and ability rules for errors.
    Validate(Input),
    /// Show a character's calculated stats and check any supplied ability rules.
    Prepare(Inspection),
    /// Show command details, defaults, limits, schemas, and examples.
    Describe {
        /// Command to describe; omit for all commands. Quote nested names, e.g. "apl explain".
        command: Option<String>,
    },
    /// Show the simulator, game data, model, and output format versions.
    Version,
    /// Find heroes, equipment, abilities, and names to use in ability rules.
    Catalog(Catalog),
    /// Show the JSON schema for an input or output document.
    Schema {
        /// Schema name, e.g. character, request, or run-response; describe lists all names.
        document: String,
    },
    /// Create character files for the simulator and Character Planner.
    #[command(subcommand)]
    Character(CharacterCommand),
    /// Get default ability rules or inspect how a character uses them.
    #[command(subcommand)]
    Apl(AplCommand),
    /// Show which abilities a character chooses during one simulated fight.
    Trace(Trace),
    /// Simulate multiple named builds, continuing if one fails.
    Batch(Batch),
    /// Compare builds with a baseline and estimate the uncertainty in damage differences.
    Compare(Compare),
}
#[derive(Subcommand)]
pub enum CharacterCommand {
    /// Create a character JSON file with no equipment.
    Init {
        /// Choose a hero by ID; use catalog heroes to list IDs.
        #[arg(long)]
        hero: String,
    },
}
#[derive(Subcommand)]
pub enum AplCommand {
    /// Show the bundled default ability rules for a hero.
    Default {
        /// Choose a hero by ID; use catalog heroes to list IDs.
        #[arg(long)]
        hero: String,
    },
    /// Explain how ability rules are interpreted for a character.
    Explain(Inspection),
}
#[derive(Clone, Copy, Debug, Default, ValueEnum, PartialEq)]
pub enum Detail {
    #[default]
    Summary,
    Full,
}
#[derive(Args, Default)]
pub struct Input {
    /// Read a complete request JSON file; use - to read from standard input.
    #[arg(long, conflicts_with_all = ["character","apl","default_apl","config","iterations","targets","seed"])]
    pub input: Option<PathBuf>,
    /// Read a character JSON file exported from the Fellership Character Planner.
    #[arg(long)]
    pub character: Option<PathBuf>,
    /// Read ability rules from an action priority list (APL) file for this hero.
    #[arg(long, conflicts_with = "default_apl")]
    pub apl: Option<PathBuf>,
    /// Use the bundled default ability rules for the character's hero.
    #[arg(long)]
    pub default_apl: bool,
    /// Read simulation settings from JSON; command-line settings override the file.
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Set how many fights to simulate (100-100000; default: 10000 unless configured).
    #[arg(long)]
    pub iterations: Option<u32>,
    /// Set how many enemies to attack (1-20; default: 1 unless configured).
    #[arg(long)]
    pub targets: Option<u32>,
    /// Set a repeatable random seed (16 lowercase hex digits; default: configured or derived from inputs).
    #[arg(long)]
    pub seed: Option<String>,
}
#[derive(Args)]
pub struct Inspection {
    #[command(flatten)]
    pub input: Input,
    /// Choose a summary or full breakdowns and supporting model information.
    #[arg(long, value_enum, default_value = "summary")]
    pub detail: Detail,
}
#[derive(Args)]
pub struct Execution {
    #[command(flatten)]
    pub inspection: Inspection,
    /// Stop the command after this many real seconds (at least 1; no limit by default).
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    pub timeout: Option<u64>,
}
#[derive(Args)]
pub struct Trace {
    #[command(flatten)]
    pub execution: Execution,
    /// Choose which repeatable fight to trace, counting from zero (0-99999).
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u32).range(0..100000))]
    pub iteration_index: u32,
    /// Record decisions for this many simulated seconds (0-300); the fight still finishes.
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(0..=300))]
    pub seconds: u32,
    /// Record at most this many ability decisions (1-10000).
    #[arg(long, default_value_t = 200, value_parser = clap::value_parser!(u32).range(1..=10000))]
    pub decisions: u32,
}
#[derive(Args)]
pub struct Batch {
    /// Read batch JSON; paths are relative to that file, or the current directory with - (standard input).
    #[arg(long)]
    pub input: PathBuf,
    /// Run this many cases at once (1-4), sharing up to four workers.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=4))]
    pub jobs: u32,
    /// Choose a summary or full breakdowns and supporting model information.
    #[arg(long, value_enum, default_value = "summary")]
    pub detail: Detail,
    /// Stop the whole batch after this many real seconds (at least 1; no limit by default).
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    pub timeout: Option<u64>,
}
#[derive(Clone, Copy, ValueEnum)]
pub enum Metric {
    TotalDps,
    PrimaryTargetDps,
}
#[derive(Args)]
pub struct Compare {
    #[command(flatten)]
    pub batch: Batch,
    /// Compare every other case with this case ID from the batch file.
    #[arg(long)]
    pub baseline: String,
    /// Compare damage per second across all targets or only the primary target.
    #[arg(long, value_enum, default_value = "total-dps")]
    pub metric: Metric,
}
#[derive(Args)]
pub struct Catalog {
    #[arg(help = format!("Choose what to search: {}", crate::catalog::KINDS.join(", ")))]
    pub kind: String,
    /// Search for a hero by ID; use catalog heroes to list IDs.
    #[arg(long)]
    pub hero: Option<String>,
    /// Use a character JSON file to find abilities and rule names available to that build.
    #[arg(long)]
    pub character: Option<PathBuf>,
    /// Find text in IDs, names, descriptions, or rule names (ignores letter case).
    #[arg(long)]
    pub query: Option<String>,
    /// Find an entry with this exact ID.
    #[arg(long)]
    pub id: Option<String>,
    /// Find equipment for this slot ID, e.g. weapon.
    #[arg(long)]
    pub position: Option<String>,
    /// Find equipment available at this rarity.
    #[arg(long)]
    pub rarity: Option<String>,
    /// Find equipment available at this item level.
    #[arg(long)]
    pub level: Option<u32>,
    /// Return at most this many matching entries (1-500).
    #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u32).range(1..=500))]
    pub limit: u32,
    /// Skip this many matching entries before returning results.
    #[arg(long, default_value_t = 0)]
    pub offset: u32,
}
impl Command {
    pub fn identity(&self) -> &'static str {
        match self {
            Self::Run(_) => "run",
            Self::Validate(_) => "validate",
            Self::Prepare(_) => "prepare",
            Self::Describe { .. } => "describe",
            Self::Version => "version",
            Self::Catalog(_) => "catalog",
            Self::Schema { .. } => "schema",
            Self::Character(_) => "character init",
            Self::Apl(AplCommand::Default { .. }) => "apl default",
            Self::Apl(AplCommand::Explain(_)) => "apl explain",
            Self::Trace(_) => "trace",
            Self::Batch(_) => "batch",
            Self::Compare(_) => "compare",
        }
    }
}
