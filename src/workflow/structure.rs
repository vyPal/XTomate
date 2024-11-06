use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use toml::Table;

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

    pub fn add_task(
        &mut self,
        name: String,
        command: String,
        dependencies: Option<Vec<Dependency>>,
    ) {
        self.tasks.insert(
            name,
            Task {
                command,
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
}

impl Task {
    pub fn get_dependencies(&self) -> Option<&Vec<Dependency>> {
        self.dependencies.as_ref()
    }
}
