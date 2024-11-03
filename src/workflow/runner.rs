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
                for dependency in dependencies.iter() {
                    println!("Running dependency: {}", dependency);
                    self.run(dependency);
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
}
