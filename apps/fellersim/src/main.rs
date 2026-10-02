use clap::{Args, Parser, Subcommand};
use fellersim_core::{
    preparation::{
        CharacterBuild, INPUT_SCHEMA_VERSION, SimulationInput, SimulationOptions, prepare,
    },
    simulate_owned,
};
use serde::Deserialize;
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Parser)]
#[command(
    name = "fellersim",
    about = "Run Fellowship character simulations locally",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Run(Input),
    Validate(Input),
    Version,
}
#[derive(Args)]
struct Input {
    #[arg(
        long,
        help = "JSON build exported from the Fellership Character Planner"
    )]
    character: PathBuf,
    #[arg(long)]
    apl: PathBuf,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    iterations: Option<u32>,
    #[arg(long)]
    targets: Option<u32>,
    #[arg(long)]
    seed: Option<String>,
    #[arg(long, help = "Write a structured JSON result to stdout")]
    json: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CharacterFile {
    format: String,
    version: u32,
    build: CharacterBuild,
}

fn read_character(path: &PathBuf) -> Result<CharacterBuild, String> {
    let file: CharacterFile =
        serde_json::from_str(&read(path)?).map_err(|e| format!("{}: {e}", path.display()))?;
    if file.format != "fellership-character-planner" || file.version != 6 {
        return Err(format!(
            "{}: Expected a Fellership Character Planner export with format \"fellership-character-planner\" and version 6.",
            path.display()
        ));
    }
    Ok(file.build)
}

fn read(path: &PathBuf) -> Result<String, String> {
    let file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut contents = String::new();
    use std::io::Read;
    file.take(1024 * 1024 + 1)
        .read_to_string(&mut contents)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if contents.len() > 1024 * 1024 {
        return Err(format!("{} exceeds 1 MiB", path.display()));
    }
    Ok(contents)
}
fn request(input: &Input) -> Result<SimulationInput, String> {
    let character = read_character(&input.character)?;
    let mut options: SimulationOptions = match &input.config {
        Some(path) => {
            serde_json::from_str(&read(path)?).map_err(|e| format!("{}: {e}", path.display()))?
        }
        None => SimulationOptions::default(),
    };
    if let Some(n) = input.iterations {
        options.iterations = n;
    }
    if let Some(n) = input.targets {
        options.targets = n;
    }
    if let Some(seed) = &input.seed {
        options.seed = Some(seed.clone());
    }
    Ok(SimulationInput {
        schema_version: INPUT_SCHEMA_VERSION,
        run_id: "cli".into(),
        data_build_id: fellersim_data::build_id().into(),
        character,
        apl_source: read(&input.apl)?,
        options,
    })
}
#[tokio::main]
async fn main() -> std::process::ExitCode {
    // Include the data/model versions with both conventional version spellings.
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        version();
        return std::process::ExitCode::SUCCESS;
    }
    match run(Cli::parse()).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            std::process::ExitCode::FAILURE
        }
    }
}
fn version() {
    println!(
        "fellersim {}\nGame build: {}",
        env!("CARGO_PKG_VERSION"),
        fellersim_data::build_id()
    );
    for hero in fellersim_data::catalog()["heroes"]
        .as_object()
        .unwrap()
        .values()
    {
        println!(
            "{}: {}",
            hero["name"].as_str().unwrap(),
            hero["evidence"]["modelRevision"].as_str().unwrap()
        );
    }
}
async fn run(cli: Cli) -> Result<(), String> {
    let (input, validate) = match cli.command {
        Command::Run(i) => (i, false),
        Command::Validate(i) => (i, true),
        Command::Version => {
            version();
            return Ok(());
        }
    };
    let request = prepare(&request(&input)?).map_err(|e| e.to_string())?;
    if validate {
        if input.json {
            println!(
                "{}",
                serde_json::json!({"valid":true,"simulatorVersion":env!("CARGO_PKG_VERSION"),"dataBuildId":request.data_build_id,"modelVersion":request.model_version,"seed":request.seed})
            );
        } else {
            println!(
                "Character and APL are valid. Game build: {}. Seed: {}.",
                request.data_build_id, request.seed
            );
        }
        return Ok(());
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancelled.clone();
    let mut worker = tokio::task::spawn_blocking(move || {
        simulate_owned(request, worker_cancel, |p| {
            if p.completed_iterations == 0 || p.completed_iterations == p.total_iterations {
                eprintln!(
                    "Iterations: {}/{}",
                    p.completed_iterations, p.total_iterations
                );
            }
        })
    });
    let result=tokio::select! {
        result=&mut worker=>result,
        signal=tokio::signal::ctrl_c()=>{signal.map_err(|e|e.to_string())?;cancelled.store(true,Ordering::Release);worker.await}
    }.map_err(|e|e.to_string())?.map_err(|e|e.to_string())?;
    if input.json {
        let mut output = io::BufWriter::new(io::stdout().lock());
        let mut value = serde_json::to_value(&result).map_err(|e| e.to_string())?;
        value["simulatorVersion"] = serde_json::json!(env!("CARGO_PKG_VERSION"));
        serde_json::to_writer_pretty(&mut output, &value).map_err(|e| e.to_string())?;
        writeln!(output)
            .and_then(|_| output.flush())
            .map_err(|e| e.to_string())?;
    } else {
        println!(
            "Fellersim {} · game build {}",
            env!("CARGO_PKG_VERSION"),
            fellersim_data::build_id()
        );
        println!(
            "Mean DPS: {:.2}\n95% confidence interval: {:.2}–{:.2}\nPrimary target DPS: {:.2}\nIterations: {}\nSeed: {}\nModel: {}",
            result.mean_dps,
            result.confidence_interval_95.low,
            result.confidence_interval_95.high,
            result.primary_target_dps,
            result.iterations,
            result.seed,
            result.model_version
        );
        for ability in &result.abilities {
            println!(
                "  {:30} {:>12.2} DPS",
                ability.ability_name, ability.mean_dps
            );
        }
    }
    Ok(())
}
