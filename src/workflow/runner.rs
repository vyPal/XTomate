use libloading::{Library, Symbol};
use semver::VersionReq;
use serde_json;
use std::ffi::CString;
use std::os::raw::c_char;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
};
use toml::{Table, Value};

use crate::plugins;

use super::placeholders::Context;
use super::structure::{Dependency, WorkFlow};

pub struct Runner {
    workflow: WorkFlow,
    tasks: HashMap<String, RunnerTask>,
    order: Vec<Vec<String>>,
    plugin_manager: plugins::manager::PluginManager,
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
    pub fn new(workflow: WorkFlow, plugin_manager: plugins::manager::PluginManager) -> Self {
        Runner {
            workflow,
            tasks: HashMap::new(),
            order: vec![],
            plugin_manager,
            plugins: vec![],
        }
    }

    pub fn load(&mut self) {
        let version_req = VersionReq::parse(&self.workflow.version).unwrap();
        if !version_req.matches(&semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap()) {
            panic!(
                "Workflow version mismatch: required {}, found {}",
                self.workflow.version,
                env!("CARGO_PKG_VERSION")
            );
        }

        let plugins = self.workflow.get_plugins();
        if let Some(plugins) = plugins {
            for plugin in plugins {
                unsafe {
                    self.plugin_manager
                        .verify_plugin(
                            plugin.name.clone(),
                            plugin.source.clone(),
                            plugin.version.clone(),
                        )
                        .unwrap();

                    let lib_path = self
                        .plugin_manager
                        .get_plugin(plugin.name.as_str())
                        .unwrap();
                    let lib =
                        Library::new(lib_path.get_install_path()).expect("Failed to load plugin");

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
        let success;

        let mut context = Context::new();

        if let Some(on_start) = task.get_on_start() {
            for finish in on_start.iter() {
                match finish {
                    Dependency::Simple(task) => match parse_dependency(task) {
                        ("task", task) => {
                            Box::pin(self.run(task)).await;
                        }
                        ("template", template) => {
                            let _ = self.execute_template(
                                task_name,
                                template,
                                &Table::new(),
                                &mut context,
                            );
                        }
                        ("plugin", plugin) => {
                            let _ = self
                                .execute_plugin(plugin, &Table::new(), &mut context)
                                .await;
                        }
                        _ => panic!("Invalid dependency: {}", task),
                    },
                    Dependency::Status(dep) => match parse_dependency(dep.keys().next().unwrap()) {
                        ("task", task) => {
                            Box::pin(self.run(task)).await;
                        }
                        ("template", template) => {
                            let _ = self.execute_template(
                                task_name,
                                template,
                                &Table::new(),
                                &mut context,
                            );
                        }
                        ("plugin", plugin) => {
                            let _ = self
                                .execute_plugin(
                                    plugin,
                                    dep.values()
                                        .next()
                                        .unwrap_or(&Value::Table(Table::new()))
                                        .as_table()
                                        .unwrap(),
                                    &mut context,
                                )
                                .await;
                        }
                        _ => panic!("Invalid dependency: {}", dep.keys().next().unwrap()),
                    },
                }
            }
        }

        if task.template.is_some() {
            if let Some(config) = task.get_config() {
                for (key, value) in config.iter() {
                    context.set(key.clone(), value.as_str().unwrap().to_string());
                }
            }
            success = self
                .execute_template(
                    &task_name,
                    task.template.clone().unwrap().as_str(),
                    task.get_env().unwrap_or(&Table::new()),
                    &mut context,
                )
                .await;
        } else if task.command.is_some() {
            success = self
                .execute_command(
                    &task_name,
                    task.command.clone().unwrap().as_str(),
                    task.get_env().unwrap_or(&Table::new()),
                    task.retry.unwrap_or(0),
                    task.retry_delay.unwrap_or(0),
                    &mut context,
                )
                .await;
        } else if task.plugin.is_some() {
            success = self
                .execute_plugin(
                    task.plugin.clone().unwrap().as_str(),
                    task.get_config().unwrap_or(&Table::new()),
                    &mut context,
                )
                .await;
        } else {
            panic!("Task `{}` has no command, plugin, or template", task_name);
        }

        if !success {
            if let Some(on_error) = task.get_on_error() {
                for finish in on_error.iter() {
                    match finish {
                        Dependency::Simple(task) => match parse_dependency(task) {
                            ("task", task) => {
                                Box::pin(self.run(task)).await;
                            }
                            ("template", template) => {
                                let _ = self.execute_template(
                                    task_name,
                                    template,
                                    &Table::new(),
                                    &mut context,
                                );
                            }
                            ("plugin", plugin) => {
                                let _ = self
                                    .execute_plugin(plugin, &Table::new(), &mut context)
                                    .await;
                            }
                            _ => panic!("Invalid dependency: {}", task),
                        },
                        Dependency::Status(dep) => {
                            match parse_dependency(dep.keys().next().unwrap()) {
                                ("task", task) => {
                                    Box::pin(self.run(task)).await;
                                }
                                ("template", template) => {
                                    let _ = self.execute_template(
                                        task_name,
                                        template,
                                        &Table::new(),
                                        &mut context,
                                    );
                                }
                                ("plugin", plugin) => {
                                    let _ = self
                                        .execute_plugin(
                                            plugin,
                                            dep.values()
                                                .next()
                                                .unwrap_or(&Value::Table(Table::new()))
                                                .as_table()
                                                .unwrap(),
                                            &mut context,
                                        )
                                        .await;
                                }
                                _ => panic!("Invalid dependency: {}", dep.keys().next().unwrap()),
                            }
                        }
                    }
                }
            }
        }

        if let Some(runner_task) = self.tasks.get(task_name) {
            *runner_task.success.lock().expect("Failed to lock mutex") = Some(success);
        }

        if let Some(on_finish) = task.get_on_finish() {
            for finish in on_finish.iter() {
                match finish {
                    Dependency::Simple(task) => match parse_dependency(task) {
                        ("task", task) => {
                            Box::pin(self.run(task)).await;
                        }
                        ("template", template) => {
                            let _ = self.execute_template(
                                task_name,
                                template,
                                &Table::new(),
                                &mut context,
                            );
                        }
                        ("plugin", plugin) => {
                            let _ = self
                                .execute_plugin(plugin, &Table::new(), &mut context)
                                .await;
                        }
                        _ => panic!("Invalid dependency: {}", task),
                    },
                    Dependency::Status(dep) => match parse_dependency(dep.keys().next().unwrap()) {
                        ("task", task) => {
                            Box::pin(self.run(task)).await;
                        }
                        ("template", template) => {
                            let _ = self.execute_template(
                                task_name,
                                template,
                                &Table::new(),
                                &mut context,
                            );
                        }
                        ("plugin", plugin) => {
                            let _ = self
                                .execute_plugin(
                                    plugin,
                                    dep.values()
                                        .next()
                                        .unwrap_or(&Value::Table(Table::new()))
                                        .as_table()
                                        .unwrap(),
                                    &mut context,
                                )
                                .await;
                        }
                        _ => panic!("Invalid dependency: {}", dep.keys().next().unwrap()),
                    },
                }
            }
        }
    }

    async fn execute_template(
        &self,
        task_name: &str,
        template_name: &str,
        environment: &Table,
        context: &mut Context,
    ) -> bool {
        let template = self
            .workflow
            .get_template(template_name)
            .expect("Template not found");

        let mut env: Vec<(String, String)> = vec![];
        for (key, value) in environment.iter() {
            let resolved_value = context.resolve(value.as_str().unwrap());
            env.push((key.clone(), resolved_value));
        }
        if let Some(template_env) = template.get_env() {
            for (key, value) in template_env.iter() {
                let resolved_value = context.resolve(value.as_str().unwrap());
                env.push((key.clone(), resolved_value));
            }
        }

        if let Some(dependencies) = template.get_dependencies() {
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
                            panic!(
                                "Dependency did not satisfy state {}: {}, terminating workflow!",
                                required_status, dependency
                            );
                        }
                    }
                }
            }
        }

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(context.resolve(template.command.as_ref().unwrap()))
            .envs(env.clone())
            .output()
            .await;

        let mut success = output
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

        if let Some(retry) = template.retry {
            let mut retries = 0;
            while !success && retries < retry {
                retries += 1;
                if let Some(delay) = template.retry_delay {
                    tokio::time::sleep(std::time::Duration::from_secs(delay as u64)).await;
                }

                let output = tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(context.resolve(template.command.as_ref().unwrap()))
                    .envs(env.clone())
                    .output()
                    .await;

                success = output.map(|o| o.status.success()).unwrap_or(false);
            }
        }

        success
    }

    async fn execute_command(
        &self,
        task_name: &str,
        command: &str,
        environment: &Table,
        retry: usize,
        retry_delay: usize,
        context: &mut Context,
    ) -> bool {
        let mut env: Vec<(String, String)> = vec![];

        for (key, value) in environment.iter() {
            let resolved_value = context.resolve(value.as_str().unwrap());
            env.push((key.clone(), resolved_value));
        }

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .envs(env.clone())
            .output()
            .await;

        let mut success = output
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

        if retry > 0 {
            let mut retries = 0;
            while !success && retries < retry {
                retries += 1;
                if retry_delay > 0 {
                    tokio::time::sleep(std::time::Duration::from_secs(retry_delay as u64)).await;
                }

                let output = tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .envs(env.clone())
                    .output()
                    .await;

                success = output.map(|o| o.status.success()).unwrap_or(false);
            }
        }

        success
    }

    async fn execute_plugin(
        &self,
        plugin_name: &str,
        config: &Table,
        context: &mut Context,
    ) -> bool {
        let plugin = self
            .plugins
            .iter()
            .find(|p| &p.name == plugin_name)
            .expect("Plugin not found");

        let config_resolved = context.resolve(&serde_json::to_string(&config).unwrap());

        unsafe {
            let execute: Symbol<unsafe extern "C" fn(*const c_char) -> i32> =
                plugin.plugin.get(b"execute").unwrap();

            let config_cstr = CString::new(config_resolved).unwrap();
            execute(config_cstr.as_ptr());
        }

        true
    }

    pub async fn run_all(self: Arc<Self>) {
        if let Some(on_start) = self.workflow.get_on_start() {
            for finish in on_start.iter() {
                match finish {
                    Dependency::Simple(task) => match parse_dependency(task) {
                        ("task", task) => {
                            Box::pin(self.run(task)).await;
                        }
                        ("template", template) => {
                            let _ = self.execute_template(
                                "on_start",
                                template,
                                &Table::new(),
                                &mut Context::new(),
                            );
                        }
                        ("plugin", plugin) => {
                            let _ = self
                                .execute_plugin(plugin, &Table::new(), &mut Context::new())
                                .await;
                        }
                        _ => panic!("Invalid dependency: {}", task),
                    },
                    Dependency::Status(dep) => match parse_dependency(dep.keys().next().unwrap()) {
                        ("task", task) => {
                            Box::pin(self.run(task)).await;
                        }
                        ("template", template) => {
                            let _ = self.execute_template(
                                "on_start",
                                template,
                                &Table::new(),
                                &mut Context::new(),
                            );
                        }
                        ("plugin", plugin) => {
                            let _ = self
                                .execute_plugin(
                                    plugin,
                                    dep.values()
                                        .next()
                                        .unwrap_or(&Value::Table(Table::new()))
                                        .as_table()
                                        .unwrap(),
                                    &mut Context::new(),
                                )
                                .await;
                        }
                        _ => panic!("Invalid dependency: {}", dep.keys().next().unwrap()),
                    },
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
                    Dependency::Simple(task) => match parse_dependency(task) {
                        ("task", task) => {
                            Box::pin(self.run(task)).await;
                        }
                        ("template", template) => {
                            let _ = self.execute_template(
                                "on_finish",
                                template,
                                &Table::new(),
                                &mut Context::new(),
                            );
                        }
                        ("plugin", plugin) => {
                            let _ = self
                                .execute_plugin(plugin, &Table::new(), &mut Context::new())
                                .await;
                        }
                        _ => panic!("Invalid dependency: {}", task),
                    },
                    Dependency::Status(dep) => match parse_dependency(dep.keys().next().unwrap()) {
                        ("task", task) => {
                            Box::pin(self.run(task)).await;
                        }
                        ("template", template) => {
                            let _ = self.execute_template(
                                "on_finish",
                                template,
                                &Table::new(),
                                &mut Context::new(),
                            );
                        }
                        ("plugin", plugin) => {
                            let _ = self
                                .execute_plugin(
                                    plugin,
                                    dep.values()
                                        .next()
                                        .unwrap_or(&Value::Table(Table::new()))
                                        .as_table()
                                        .unwrap(),
                                    &mut Context::new(),
                                )
                                .await;
                        }
                        _ => panic!("Invalid dependency: {}", dep.keys().next().unwrap()),
                    },
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

fn parse_dependency(dep: &str) -> (&str, &str) {
    let parts: Vec<&str> = dep.splitn(2, ':').collect();
    if parts.len() == 2 {
        (parts[0], parts[1])
    } else {
        ("task", dep)
    }
}
