use std::path::PathBuf;
use std::process::ExitCode;

use caelum_core::projects::Project;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "caelum", version, about = "Your code. New horizons.")]
struct Cli {
    /// Open a local project (defaults to the current directory).
    #[arg(value_name = "PROJECT")]
    project: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Start an interactive agent session.
    Chat {
        #[arg(value_name = "PROJECT")]
        project: Option<PathBuf>,
    },
    /// Send a task to an agent.
    Run {
        prompt: String,
        #[arg(short, long, value_name = "PROJECT")]
        project: Option<PathBuf>,
    },
    /// List saved sessions.
    Sessions,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("caelum: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Some(Command::Chat { project }) => {
            if let Some(path) = project {
                print_project(&path)?;
            }
            Err("agent sessions are not available in this foundation build".into())
        }
        Some(Command::Run { prompt: _, project }) => {
            if let Some(path) = project {
                print_project(&path)?;
            }
            Err("agent execution is not available in this foundation build".into())
        }
        Some(Command::Sessions) => {
            println!("Session storage is not available in this foundation build.");
            Ok(())
        }
        None => {
            let path =
                cli.project
                    .unwrap_or(std::env::current_dir().map_err(|error| {
                        format!("could not determine the current directory: {error}")
                    })?);
            print_project(&path)
        }
    }
}

fn print_project(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let project = Project::open(path)?;
    println!("Project: {}", project.name());
    println!("Path: {}", project.path().display());
    println!("The desktop launcher and agent integrations are not wired up yet.");
    Ok(())
}
