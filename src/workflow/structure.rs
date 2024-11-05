use serde::{Deserialize, Serialize};
use toml::Table;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug)]
pub struct WorkFlow {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    tasks: HashMap<String, Task>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Task {
    pub command: String,
    dependencies: Option<Vec<Dependency>>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(untagged)]
pub enum Dependency {
    Simple(String),
    Status(Table),
}

impl WorkFlow {
    pub fn new(name: String, version: String, description: Option<String>) -> Self {
        WorkFlow {
            name,
            version,
            description,
            tasks: HashMap::new(),
        }
    }

    pub fn add_task(&mut self, name: String, command: String, dependencies: Option<Vec<Dependency>>) {
        self.tasks.insert(
            name,
            Task {
                command,
                dependencies,
            },
        );
    }

    pub fn remove_task(&mut self, name: &str) {
        self.tasks.remove(name);
    }

    pub fn get_task(&self, name: &str) -> Option<&Task> {
        self.tasks.get(name)
    }

    pub fn get_tasks(&self) -> &HashMap<String, Task> {
        &self.tasks
    }
}

impl Task {
    pub fn new(command: String, dependencies: Option<Vec<Dependency>>) -> Self {
        Task {
            command,
            dependencies,
        }
    }

    pub fn add_dependency(&mut self, dependency: Dependency) {
        if let Some(ref mut dependencies) = self.dependencies {
            dependencies.push(dependency);
        } else {
            self.dependencies = Some(vec![dependency]);
        }
    }

    pub fn remove_dependency(&mut self, dependency: &Dependency) {
        if let Some(ref mut dependencies) = self.dependencies {
            dependencies.retain(|d| d != dependency);
        }
    }

    pub fn get_dependencies(&self) -> Option<&Vec<Dependency>> {
        self.dependencies.as_ref()
    }
}
