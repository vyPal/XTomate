use std::collections::{HashMap, HashSet, VecDeque};

use super::structure::{Dependency, WorkFlow};

pub struct Runner {
    workflow: WorkFlow,
    tasks: HashMap<String, RunnerTask>,
    order: Vec<Vec<String>>,
}

struct RunnerTask {
    success: Option<bool>,
}

impl Runner {
    pub fn new(workflow: WorkFlow) -> Self {
        Runner {
            workflow,
            tasks: HashMap::new(),
            order: vec![],
        }
    }

    pub fn load(&mut self) {
        let tasks = self.workflow.get_tasks();
        if tasks.is_empty() {
            return;
        }

        for (name, _) in tasks.iter() {
            let runnertask = RunnerTask { success: None };

            self.tasks.insert(name.clone(), runnertask);
        }

        self.determine_order().expect("Failed to determine order");
    }

    fn determine_order(&mut self) -> Result<(), String> {
        let tasks = self.workflow.get_tasks();
        let mut graph: HashMap<String, HashSet<String>> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        for (task, dependencies) in tasks {
            graph.entry(task.clone()).or_default();
            in_degree.entry(task.clone()).or_insert(0);

            for dependency in dependencies.get_dependencies().unwrap_or(&vec![]) {
                match &dependency {
                    Dependency::Simple(dependency) => {
                        graph
                            .entry(dependency.clone())
                            .or_default()
                            .insert(task.clone());
                        *in_degree.entry(task.clone()).or_insert(0) += 1;
                    }
                    Dependency::Status(dep) => {
                        let dependency = dep.keys().next().unwrap();
                        graph
                            .entry(dependency.clone())
                            .or_default()
                            .insert(task.clone());
                        *in_degree.entry(task.clone()).or_insert(0) += 1;
                    }
                }
            }
        }

        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|&(_, &deg)| deg == 0)
            .map(|(task, _)| task.clone())
            .collect();

        let mut stages: Vec<Vec<String>> = vec![];
        let mut visited: HashSet<String> = HashSet::new();

        while !queue.is_empty() {
            let mut current_stage = vec![];

            for _ in 0..queue.len() {
                let task = queue.pop_front().unwrap();
                current_stage.push(task.clone());
                visited.insert(task.clone());

                if let Some(dependents) = graph.get(&task) {
                    for dependent in dependents {
                        if let Some(degree) = in_degree.get_mut(dependent) {
                            *degree -= 1;
                            if *degree == 0 {
                                queue.push_back(dependent.clone());
                            }
                        }
                    }
                }
            }

            if !current_stage.is_empty() {
                stages.push(current_stage);
            }
        }

        if visited.len() != tasks.len() {
            return Err("Cycle detected in task dependencies".to_string());
        }

        self.order = stages;
        Ok(())
    }

    pub fn run(&self, task_name: &str) {
        let task = self.workflow.get_task(task_name).unwrap();

        match &task.get_dependencies() {
            Some(dependencies) => {
                for dep in dependencies.iter() {
                    match dep {
                        Dependency::Simple(dependency) => {
                            if self.check_dependency_status(dependency, "success") {
                                self.run(dependency);
                            }
                        }
                        Dependency::Status(dep) => {
                            let dependency = dep.keys().next().unwrap();
                            let required_status = dep.get(dependency).unwrap().as_str().unwrap();

                            if self.check_dependency_status(dependency, required_status) {
                                self.run(dependency);
                            }
                        }
                    }
                }
            }
            None => {}
        }

        std::process::Command::new("sh")
            .arg("-c")
            .arg(&task.command)
            .stdout(std::process::Stdio::inherit())
            .output()
            .expect("Failed to execute process");
    }

    pub fn run_all(&self) {
        for stage in self.order.iter() {
            for task in stage {
                self.run(task);
            }
        }
    }

    fn check_dependency_status(&self, task: &str, status: &str) -> bool {
        let task = self.tasks.get(task).unwrap();
        match status {
            "success" => task.success.unwrap_or(false),
            "failure" => task.success.unwrap_or(true),
            _ => false,
        }
    }
}
