use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "fellersim",
    about = "Discover, inspect, and simulate Fellowship builds offline",
    version
)]
pub struct Cli {
    /// Emit exactly one versioned JSON response, including errors and help.
    #[arg(long, global = true)]
    pub json: bool,
    /// Suppress progress on stderr.
    #[arg(long, global = true)]
    pub quiet: bool,
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Subcommand)]
pub enum Command {
    /// Run a deterministic Monte Carlo simulation.
    Run(Execution),
    /// Validate a planner export, simulation options, and APL.
    Validate(Input),
    /// Inspect resolved character statistics and optionally compile an APL.
    Prepare(Inspection),
    /// Describe commands, defaults, limits, schemas, and examples.
    Describe { command: Option<String> },
    /// Report executable, data, model, and protocol versions.
    Version,
    /// Search legal choices and exact APL tokens.
    Catalog(Catalog),
    /// Emit a JSON Schema Draft 2020-12 document.
    Schema { document: String },
    #[command(subcommand)]
    Character(CharacterCommand),
    #[command(subcommand)]
    Apl(AplCommand),
    /// Observe APL decisions from one complete deterministic iteration.
    Trace(Trace),
    /// Run labeled cases, continuing after individual failures.
    Batch(Batch),
    /// Compare variants against a baseline using paired confidence intervals.
    Compare(Compare),
}
#[derive(Subcommand)]
pub enum CharacterCommand {
    /// Emit an unequipped, planner-compatible character file.
    Init {
        #[arg(long)]
        hero: String,
    },
}
#[derive(Subcommand)]
pub enum AplCommand {
    /// Emit the exact shipped default APL for a hero.
    Default {
        #[arg(long)]
        hero: String,
    },
    /// Explain the actual compiler's decisions for this character and APL.
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
    /// Complete versioned request file; use - for stdin.
    #[arg(long, conflicts_with_all = ["character","apl","default_apl","config","iterations","targets","seed"])]
    pub input: Option<PathBuf>,
    /// JSON exported directly from the Fellership Character Planner.
    #[arg(long)]
    pub character: Option<PathBuf>,
    /// APL source file matching the character's hero.
    #[arg(long, conflicts_with = "default_apl")]
    pub apl: Option<PathBuf>,
    /// Use the embedded default APL for the character's hero.
    #[arg(long)]
    pub default_apl: bool,
    /// Simulation options JSON; explicit flags take precedence.
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub iterations: Option<u32>,
    #[arg(long)]
    pub targets: Option<u32>,
    #[arg(long)]
    pub seed: Option<String>,
}
#[derive(Args)]
pub struct Inspection {
    #[command(flatten)]
    pub input: Input,
    #[arg(long, value_enum, default_value = "summary")]
    pub detail: Detail,
}
#[derive(Args)]
pub struct Execution {
    #[command(flatten)]
    pub inspection: Inspection,
    /// Command-wide wall-clock deadline in seconds.
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    pub timeout: Option<u64>,
}
#[derive(Args)]
pub struct Trace {
    #[command(flatten)]
    pub execution: Execution,
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u32).range(0..100000))]
    pub iteration_index: u32,
    /// Capture decisions through this simulated second; the encounter still finishes.
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(0..=300))]
    pub seconds: u32,
    #[arg(long, default_value_t = 200, value_parser = clap::value_parser!(u32).range(1..=10000))]
    pub decisions: u32,
}
#[derive(Args)]
pub struct Batch {
    /// Versioned batch manifest; paths resolve relative to its directory (cwd for stdin).
    #[arg(long)]
    pub input: PathBuf,
    /// Concurrent cases sharing a total budget of four workers.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=4))]
    pub jobs: u32,
    #[arg(long, value_enum, default_value = "summary")]
    pub detail: Detail,
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
    #[arg(long)]
    pub baseline: String,
    #[arg(long, value_enum, default_value = "total-dps")]
    pub metric: Metric,
}
#[derive(Args)]
pub struct Catalog {
    pub kind: String,
    #[arg(long)]
    pub hero: Option<String>,
    /// Filter against an actual build rather than the general hero catalog.
    #[arg(long)]
    pub character: Option<PathBuf>,
    #[arg(long)]
    pub query: Option<String>,
    #[arg(long)]
    pub id: Option<String>,
    #[arg(long)]
    pub position: Option<String>,
    #[arg(long)]
    pub rarity: Option<String>,
    #[arg(long)]
    pub level: Option<u32>,
    #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u32).range(1..=500))]
    pub limit: u32,
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
