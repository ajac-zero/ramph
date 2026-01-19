use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

mod amp;
mod prompts;
mod types;
mod workflows;

#[derive(Parser)]
#[command(name = "ramph", about = "Ralph workflow runner using Amp")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute stories from an existing PRD
    Run(RunArgs),
    /// Interactively create a new PRD with AI assistance
    Plan(PlanArgs),
}

#[derive(Args)]
struct RunArgs {
    #[arg(short, long, default_value = ".")]
    cwd: PathBuf,

    #[arg(long, default_value = "prd.json")]
    prd: PathBuf,

    /// Optional path to progress file (uses embedded default if not specified)
    #[arg(long)]
    progress: Option<PathBuf>,

    /// Optional path to prompt file (uses embedded default if not specified)
    #[arg(long)]
    prompt: Option<PathBuf>,

    #[arg(long, default_value_t = 25)]
    max_iterations: usize,
}

#[derive(Args)]
struct PlanArgs {
    #[arg(short, long, default_value = ".")]
    cwd: PathBuf,

    #[arg(short, long, default_value = "prd.json")]
    output: PathBuf,

    /// Initial project description (optional, can be provided interactively)
    #[arg(short, long)]
    description: Option<String>,

    /// Overwrite existing PRD file if it exists
    #[arg(long, default_value_t = false)]
    force: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run(args) => {
            workflows::run_command(
                args.cwd,
                args.prd,
                args.progress,
                args.prompt,
                args.max_iterations,
            )
            .await
        }
        Commands::Plan(args) => {
            workflows::run_plan_command(args.cwd, args.output, args.description, args.force).await
        }
    }
}
