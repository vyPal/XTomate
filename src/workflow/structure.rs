use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use toml::Table;

#[derive(Serialize, Deserialize, Debug)]
pub struct WorkFlow {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    on_start: Option<Vec<Dependency>>,
    on_finish: Option<Vec<Dependency>>,
    tasks: HashMap<String, Task>,
    plugins: Vec<Plugin>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Task {
    pub command: Option<String>,
    pub retry: Option<usize>,
    pub retry_delay: Option<usize>,
    pub run: Option<bool>,
    pub plugin: Option<String>,
    config: Option<Table>,
    env: Option<Table>,
    dependencies: Option<Vec<Dependency>>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(untagged)]
pub enum Dependency {
    Simple(String),
    Status(Table),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Plugin {
    pub name: String,
    pub source: String,
    pub version: Option<String>,
    config: Option<Table>,
}

impl WorkFlow {
    pub fn new(name: String, version: String, description: Option<String>) -> Self {
        WorkFlow {
            name,
            version,
            description,
            on_finish: None,
            on_start: None,
            tasks: HashMap::new(),
            plugins: vec![],
        }
    }

    pub fn add_task(
        &mut self,
        name: String,
        command: String,
        dependencies: Option<Vec<Dependency>>,
    ) {
        self.tasks.insert(
            name,
            Task {
                command: Some(command),
                plugin: None,
                config: None,
                run: None,
                retry: None,
                retry_delay: None,
                env: None,
                dependencies,
            },
        );
    }

    pub fn get_task(&self, name: &str) -> Option<&Task> {
        self.tasks.get(name)
    }

    pub fn get_tasks(&self) -> &HashMap<String, Task> {
        &self.tasks
    }

    pub fn get_plugins(&self) -> &Vec<Plugin> {
        self.plugins.as_ref()
    }

    pub fn get_on_finish(&self) -> Option<&Vec<Dependency>> {
        self.on_finish.as_ref()
    }

    pub fn get_on_start(&self) -> Option<&Vec<Dependency>> {
        self.on_start.as_ref()
    }
}

impl Task {
    pub fn get_dependencies(&self) -> Option<&Vec<Dependency>> {
        self.dependencies.as_ref()
    }

    pub fn get_config(&self) -> Option<&Table> {
        self.config.as_ref()
    }

    pub fn get_env(&self) -> Option<&Table> {
        self.env.as_ref()
    }
}

impl Plugin {
    pub fn get_config(&self) -> Option<&Table> {
        self.config.as_ref()
    }
}
