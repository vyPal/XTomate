use clap::{Parser, Subcommand};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use toml::to_string;

use workflow::runner::Runner;
use workflow::structure::{Dependency, WorkFlow};

mod config;
mod plugins;
mod workflow;

#[derive(Parser)]
#[command(name = "XTomate", version, about, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Creates a new workflow
    Create {
        /// The name of the workflow
        name: String,
    },
    /// Deletes a workflow
    Delete {
        /// The name of the workflow
        name: String,

        /// Whether to force deletion
        #[arg(short, long)]
        force: bool,
    },
    /// Runs a workflow
    Run {
        /// The name of the workflow
        name: String,
    },
}

fn write_workflow(workflow: &WorkFlow, file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let toml_string = to_string(workflow)?;
    let mut file = File::create(file_path)?;
    file.write_all(toml_string.as_bytes())?;
    Ok(())
}

fn read_workflow(file_path: &mut String) -> Result<WorkFlow, Box<dyn std::error::Error>> {
    if !file_path.ends_with(".toml") {
        file_path.push_str(".toml");
    }
    let file = std::fs::read_to_string(file_path)?;
    let workflow: WorkFlow = toml::from_str(&file)?;
    Ok(workflow)
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Create { name }) => {
            let xtomate_version = env!("CARGO_PKG_VERSION");
            let mut workflow = WorkFlow::new(name.to_string(), xtomate_version.to_string(), None);
            workflow.add_task("task1".to_string(), "echo Hello".to_string(), None);
            workflow.add_task(
                "task2".to_string(),
                "echo World".to_string(),
                Some(vec![Dependency::Simple("task1".to_string())]),
            );
            write_workflow(&workflow, &format!("{}.toml", name)).unwrap();
            println!("Creating workflow: {}", name);
        }
        Some(Commands::Delete { name, force }) => {
            if *force {
                println!("Force deleting workflow: {}", name);
            } else {
                println!("Deleting workflow: {}", name);
            }
        }
        Some(Commands::Run { name }) => {
            let config = config::Config::load_or_default(true).unwrap();
            let plugin_manager = plugins::manager::PluginManager::load_or_default(
                PathBuf::from(config.get_plugin_dir()),
                true,
            )
            .unwrap();
            let workflow = read_workflow(&mut name.clone()).unwrap();
            let mut runner = Runner::new(workflow, plugin_manager);
            runner.load();
            Arc::new(runner).run_all().await;
        }
        None => {
            println!("No command provided");
        }
    }
}
