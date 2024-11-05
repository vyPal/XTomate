use std::collections::HashMap;

use super::structure::{WorkFlow,Dependency};

pub struct Runner {
    workflow: WorkFlow,
    tasks: HashMap<String, RunnerTask>
}

struct RunnerTask {
    task: String,
    running: bool,
    success: bool,
}

impl Runner {
    pub fn new(workflow: WorkFlow) -> Self {
        Runner { workflow, tasks: HashMap::new() }
    }

    pub fn load(&self) {
        let tasks = self.workflow.get_tasks();
    }

    pub fn run(&self, task_name: &str) {
        let task = self.workflow.get_task(task_name).unwrap();
        match &task.get_dependencies() {
            Some(dependencies) => {
                for dep in dependencies.iter() {
                    match &dep {
                        Dependency::Simple(dependency) => {
                            println!("Running dependency: {}", dependency);
                            if self.check_dependency_status(dependency, "success") {
                                self.run(dependency);
                            } else {
                                println!("Skipping dependency: {} due to status: {}", dependency, "success");
                            }
                        }
                        Dependency::Status(dep) => {
                            let dependency = dep.keys().next().unwrap();
                            println!("Running dependency: {} with status: {}", dependency, dep.get(dependency).unwrap().as_str().unwrap());
                            if self.check_dependency_status(dependency, dep.get(dependency).unwrap().as_str().unwrap()) {
                                self.run(dependency);
                            } else {
                                println!("Skipping dependency: {} due to status: {}", dependency, dep.get(dependency).unwrap().as_str().unwrap());
                            }
                        }
                    }
                }
            }
            None => {}
        }
        println!("Running task: {}", task_name);
        println!("Command: {}", task.command);
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(&task.command)
            .output()
            .expect("failed to execute process");
        println!("status: {}", output.status);
        println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    }

    fn check_dependency_status(&self, dependency: &str, status: &str) -> bool {
        // Add logic to check the status of the dependency
        // For now, we assume all dependencies are successful
        true
    }
}
