use clap::{Parser, Subcommand};
use toml::to_string;
use std::fs::File;
use std::io::Write;

use workflow::structure::WorkFlow;

mod workflow;

#[derive(Parser)]
#[command(name = "XTomate", version, about)]
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

fn read_workflow(file_path: &str) -> Result<WorkFlow, Box<dyn std::error::Error>> {
    let file = std::fs::read_to_string(file_path)?;
    let workflow: WorkFlow = toml::from_str(&file)?;
    Ok(workflow)
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Create { name }) => {
            let mut workflow = WorkFlow::new(name.to_string(), "0.1.0".to_string(), None);
            workflow.add_task("task1".to_string(), "echo Hello".to_string(), None);
            workflow.add_task("task2".to_string(), "echo World".to_string(), Some(vec!["task1".to_string()]));
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
            let workflow = read_workflow(&format!("{}.toml", name)).unwrap();
            println!("Running workflow: {}", name);
            for (task_name, task) in workflow.get_tasks().iter() {
                println!("Running task: {}", task_name);
                println!("Command: {}", task.command);
                match &task.get_dependencies() {
                    Some(dependencies) => {
                        for dependency in dependencies.iter() {
                            println!("Dependency: {}", dependency);
                        }
                    }
                    None => {}
                }
            }
        }
        None => {
            println!("No command provided");
        }
    }
}
