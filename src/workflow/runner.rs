use std::{cell::RefCell, collections::{HashMap, HashSet, VecDeque}};

use super::structure::{Dependency, WorkFlow};

pub struct Runner {
    workflow: WorkFlow,
    tasks: HashMap<String, RunnerTask>,
    order: Vec<Vec<String>>,
}

struct RunnerTask {
    success: RefCell<Option<bool>>,
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
            let runnertask = RunnerTask { success: RefCell::new(None) };

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
        if let Some(task) = self.workflow.get_task(task_name) {
            if let Some(dependencies) = task.get_dependencies() {
                for dep in dependencies.iter() {
                    match dep {
                        Dependency::Simple(dependency) => {
                            if self.needs_run(dependency) {
                                self.run(dependency);
                            }
                            if !self.check_dependency_status(dependency, "success") {
                                panic!("Dependency failed: {}, terminating workflow!", dependency);
                            }
                        }
                        Dependency::Status(dep) => {
                            let dependency = dep.keys().next().unwrap();
                            let required_status = dep.get(dependency).unwrap().as_str().unwrap();
                            if self.needs_run(dependency) {
                                self.run(dependency);
                            }
                            if !self.check_dependency_status(dependency, required_status) {
                                panic!("Dependency did not satisfy state {}: {}, terminating workflow!", required_status, dependency);
                            }
                        }
                    }
                }
            }

            self.execute_task(task_name);
        }
    }

    fn execute_task(&self, task_name: &str) {
        let task = self.workflow.get_task(task_name).unwrap();

        let mut env: Vec<(String, String)> = vec![];
        if let Some(task_env) = task.get_env() {
            for (key, value) in task_env.iter() {
                env.push((key.clone(), value.as_str().unwrap().to_string()));
            }
        }

        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(&task.command)
            .envs(env)
            .stdout(std::process::Stdio::inherit())
            .output();

        let success = output.map(|o| o.status.success()).unwrap_or(false);
        if let Some(runner_task) = self.tasks.get(task_name) {
            *runner_task.success.borrow_mut() = Some(success);
        }
    }

    pub fn run_all(&self) {
        for stage in self.order.iter() {
            for task in stage {
                self.run(task);
            }
        }
    }

    fn check_dependency_status(&self, task: &str, status: &str) -> bool {
        if let Some(runner_task) = self.tasks.get(task) {
            match status {
                "success" => runner_task.success.borrow().unwrap_or(false),
                "failure" | "fail" => !runner_task.success.borrow().unwrap_or(true),
                "any" => true,
                _ => panic!("Unknown status: {}", status),
            }
        } else {
            false
        }
    }

    fn needs_run(&self, task: &str) -> bool {
        if let Some(runner_task) = self.tasks.get(task) {
            runner_task.success.borrow().is_none()
        } else {
            false
        }
    }
}
