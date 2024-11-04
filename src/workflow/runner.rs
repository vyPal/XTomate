use super::structure::WorkFlow;

pub struct Runner {
    workflow: WorkFlow,
}

impl Runner {
    pub fn new(workflow: WorkFlow) -> Self {
        Runner { workflow }
    }

    pub fn run(&self, task_name: &str) {
        let task = self.workflow.get_task(task_name).unwrap();
        match &task.get_dependencies() {
            Some(dependencies) => {
                for (dependency, status) in dependencies.iter() {
                    println!("Running dependency: {} with status: {}", dependency, status);
                    if self.check_dependency_status(dependency, status) {
                        self.run(dependency);
                    } else {
                        println!("Skipping dependency: {} due to status: {}", dependency, status);
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
