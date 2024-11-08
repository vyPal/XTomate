use libloading::{Library, Symbol};
use serde_json;
use std::ffi::CString;
use std::os::raw::c_char;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use super::structure::{Dependency, WorkFlow};

pub struct Runner {
    workflow: WorkFlow,
    tasks: HashMap<String, RunnerTask>,
    order: Vec<Vec<String>>,
    plugin_path: PathBuf,
    plugins: Vec<RunnerPlugin>,
}

struct RunnerPlugin {
    name: String,
    plugin: Library,
}

struct RunnerTask {
    success: Arc<Mutex<Option<bool>>>,
}

impl Runner {
    pub fn new(workflow: WorkFlow, plugin_path: PathBuf) -> Self {
        Runner {
            workflow,
            tasks: HashMap::new(),
            order: vec![],
            plugin_path,
            plugins: vec![],
        }
    }

    pub fn load(&mut self) {
        let plugins = self.workflow.get_plugins();
        for plugin in plugins {
            unsafe {
                let lib_filename = match std::env::consts::OS {
                    "linux" => format!("lib{}.so", plugin.name),
                    "macos" => format!("lib{}.dylib", plugin.name),
                    "windows" => format!("{}.dll", plugin.name),
                    _ => panic!("Unsupported OS"),
                };

                let lib_path = self.plugin_path.join(lib_filename);
                let lib = Library::new(lib_path).expect("Failed to load plugin");

                let initialize: Symbol<unsafe extern "C" fn(*const c_char) -> i32> =
                    lib.get(b"initialize").unwrap();

                let config_json = serde_json::to_string(&plugin.get_config()).unwrap();
                let config_cstr = CString::new(config_json).unwrap();

                initialize(config_cstr.as_ptr());

                self.plugins.push(RunnerPlugin {
                    name: plugin.name.clone(),
                    plugin: lib,
                });
            }
        }

        let tasks = self.workflow.get_tasks();
        if tasks.is_empty() {
            return;
        }

        for (name, _) in tasks.iter() {
            let runnertask = RunnerTask {
                success: Arc::new(Mutex::new(None)),
            };

            self.tasks.insert(name.clone(), runnertask);
        }

        self.determine_order().expect("Failed to determine order");
    }

    pub fn teardown(&self) {
        for plugin in self.plugins.iter() {
            unsafe {
                let teardown: Symbol<unsafe extern "C" fn() -> i32> =
                    plugin.plugin.get(b"teardown").unwrap();
                teardown();
            }
        }
    }

    fn determine_order(&mut self) -> Result<(), String> {
        let tasks = self.workflow.get_tasks();
        let mut graph: HashMap<String, HashSet<String>> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut non_runnable_tasks = HashSet::new();

        for (task, dependencies) in tasks {
            if let Some(run) = &dependencies.run {
                if !run {
                    non_runnable_tasks.insert(task.clone());
                }
            }

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
            .filter(|&(task, &deg)| deg == 0 && !non_runnable_tasks.contains(task))
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
                            if *degree == 0 && !non_runnable_tasks.contains(dependent) {
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

        let runnable_task_count = tasks
            .keys()
            .filter(|task| !non_runnable_tasks.contains(*task))
            .count();

        if visited.len() != runnable_task_count {
            return Err("Cycle detected in task dependencies".to_string());
        }

        self.order = stages;
        Ok(())
    }

    pub async fn run(&self, task_name: &str) {
        if let Some(task) = self.workflow.get_task(task_name) {
            if let Some(dependencies) = task.get_dependencies() {
                for dep in dependencies.iter() {
                    match dep {
                        Dependency::Simple(dependency) => {
                            if self.needs_run(dependency) {
                                Box::pin(self.run(dependency)).await;
                            }
                            if !self.check_dependency_status(dependency, "success") {
                                panic!("Dependency failed: {}, terminating workflow!", dependency);
                            }
                        }
                        Dependency::Status(dep) => {
                            let dependency = dep.keys().next().unwrap();
                            let required_status = dep.get(dependency).unwrap().as_str().unwrap();
                            if self.needs_run(dependency) {
                                Box::pin(self.run(dependency)).await;
                            }
                            if !self.check_dependency_status(dependency, required_status) {
                                panic!("Dependency did not satisfy state {}: {}, terminating workflow!", required_status, dependency);
                            }
                        }
                    }
                }
            }

            self.execute_task(task_name).await;
        }
    }

    async fn execute_task(&self, task_name: &str) {
        let task = self.workflow.get_task(task_name).unwrap();
        let mut success;

        if let Some(command) = &task.command {
            let mut env: Vec<(String, String)> = vec![];
            if let Some(task_env) = task.get_env() {
                for (key, value) in task_env.iter() {
                    env.push((key.clone(), value.as_str().unwrap().to_string()));
                }
            }

            let output = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .envs(env.clone())
                .output()
                .await;

            success = output
                .map(|o| {
                    if !o.stdout.is_empty() {
                        println!(
                            "Task `{}` stdout:\n{}",
                            task_name,
                            String::from_utf8_lossy(&o.stdout)
                        );
                    }
                    if !o.stderr.is_empty() {
                        eprintln!(
                            "Task `{}` stderr:\n{}",
                            task_name,
                            String::from_utf8_lossy(&o.stderr)
                        );
                    }
                    o.status.success()
                })
                .unwrap_or(false);

            if let Some(retry) = task.retry {
                let mut retries = 0;
                while !success && retries < retry {
                    retries += 1;
                    if let Some(delay) = task.retry_delay {
                        tokio::time::sleep(std::time::Duration::from_secs(delay as u64)).await;
                    }

                    let output = tokio::process::Command::new("sh")
                        .arg("-c")
                        .arg(command)
                        .envs(env.clone())
                        .output()
                        .await;

                    success = output
                        .map(|o| {
                            if !o.stdout.is_empty() {
                                println!(
                                    "Task `{}` retry #{} stdout:\n{}",
                                    task_name,
                                    retries,
                                    String::from_utf8_lossy(&o.stdout)
                                );
                            }
                            if !o.stderr.is_empty() {
                                eprintln!(
                                    "Task `{}` retry #{} stderr:\n{}",
                                    task_name,
                                    retries,
                                    String::from_utf8_lossy(&o.stderr)
                                );
                            }
                            o.status.success()
                        })
                        .unwrap_or(false);
                }
            }
        } else if let Some(plugin) = &task.plugin {
            let plugin = self
                .plugins
                .iter()
                .find(|p| &p.name == plugin)
                .expect("Plugin not found");

            let config = task.get_config().unwrap();
            let mut env: Vec<(String, String)> = vec![];
            if let Some(task_env) = task.get_env() {
                for (key, value) in task_env.iter() {
                    env.push((key.clone(), value.as_str().unwrap().to_string()));
                }
            }

            unsafe {
                let execute: Symbol<unsafe extern "C" fn(*const c_char) -> i32> =
                    plugin.plugin.get(b"execute").unwrap();

                let config_json = serde_json::to_string(&config).unwrap();
                let config_cstr = CString::new(config_json).unwrap();

                execute(config_cstr.as_ptr());
            }
            success = true;
        } else {
            panic!("Task `{}` has no command or plugin", task_name);
        }

        if let Some(runner_task) = self.tasks.get(task_name) {
            *runner_task.success.lock().expect("Failed to lock mutex") = Some(success);
        }
    }

    pub async fn run_all(self: Arc<Self>) {
        if let Some(on_start) = self.workflow.get_on_start() {
            for finish in on_start.iter() {
                match finish {
                    Dependency::Simple(task) => {
                        if self.needs_run(task) {
                            self.run(task).await;
                        }
                    }
                    Dependency::Status(dep) => {
                        let task = dep.keys().next().unwrap();
                        let required_status = dep.get(task).unwrap().as_str().unwrap();
                        if self.needs_run(task) {
                            self.run(task).await;
                        }
                        if !self.check_dependency_status(task, required_status) {
                            panic!("On finish task did not satisfy state {}: {}, terminating workflow!", required_status, task);
                        }
                    }
                }
            }
        }

        for stage in self.order.iter() {
            let mut handles = vec![];
            for task in stage {
                let self_clone = Arc::clone(&self);
                let task_name = task.clone();
                let handle = tokio::task::spawn(async move {
                    self_clone.run(&task_name).await;
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.await.unwrap();
            }
        }

        if let Some(on_finish) = self.workflow.get_on_finish() {
            for finish in on_finish.iter() {
                match finish {
                    Dependency::Simple(task) => {
                        if self.needs_run(task) {
                            self.run(task).await;
                        }
                    }
                    Dependency::Status(dep) => {
                        let task = dep.keys().next().unwrap();
                        let required_status = dep.get(task).unwrap().as_str().unwrap();
                        if self.needs_run(task) {
                            self.run(task).await;
                        }
                        if !self.check_dependency_status(task, required_status) {
                            panic!("On finish task did not satisfy state {}: {}, terminating workflow!", required_status, task);
                        }
                    }
                }
            }
        }

        self.teardown();
    }

    fn check_dependency_status(&self, task: &str, status: &str) -> bool {
        if let Some(runner_task) = self.tasks.get(task) {
            match status {
                "success" => runner_task.success.lock().expect("f").unwrap_or(false),
                "failure" | "fail" => !runner_task.success.lock().expect("f").unwrap_or(true),
                "any" => true,
                _ => panic!("Unknown status: {}", status),
            }
        } else {
            false
        }
    }

    fn needs_run(&self, task: &str) -> bool {
        if let Some(runner_task) = self.tasks.get(task) {
            runner_task.success.lock().expect("f").is_none()
        } else {
            false
        }
    }
}
