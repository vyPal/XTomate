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
    plugins: Option<Vec<Plugin>>,
    templates: Option<Vec<TaskTemplate>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Task {
    pub command: Option<String>,
    pub template: Option<String>,
    pub retry: Option<usize>,
    pub retry_delay: Option<usize>,
    pub run: Option<bool>,
    pub plugin: Option<String>,
    on_start: Option<Vec<Dependency>>,
    on_finish: Option<Vec<Dependency>>,
    on_error: Option<Vec<Dependency>>,
    config: Option<Table>,
    env: Option<Table>,
    dependencies: Option<Vec<Dependency>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TaskTemplate {
    pub name: String,
    pub command: Option<String>,
    pub retry: Option<usize>,
    pub retry_delay: Option<usize>,
    pub run: Option<bool>,
    pub env: Option<Table>,
    pub dependencies: Option<Vec<Dependency>>,
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
            plugins: None,
            templates: None,
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
                template: None,
                config: None,
                run: None,
                retry: None,
                retry_delay: None,
                env: None,
                dependencies,
                on_start: None,
                on_finish: None,
                on_error: None,
            },
        );
    }

    pub fn get_task(&self, name: &str) -> Option<&Task> {
        self.tasks.get(name)
    }

    pub fn get_tasks(&self) -> &HashMap<String, Task> {
        &self.tasks
    }

    pub fn get_plugins(&self) -> Option<&Vec<Plugin>> {
        self.plugins.as_ref()
    }

    pub fn get_on_finish(&self) -> Option<&Vec<Dependency>> {
        self.on_finish.as_ref()
    }

    pub fn get_on_start(&self) -> Option<&Vec<Dependency>> {
        self.on_start.as_ref()
    }

    pub fn get_template(&self, name: &str) -> Option<&TaskTemplate> {
        self.templates
            .as_ref()
            .clone()
            .expect("No templates defined")
            .iter()
            .find(|t| t.name == name)
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

    pub fn get_on_error(&self) -> Option<&Vec<Dependency>> {
        self.on_error.as_ref()
    }

    pub fn get_on_finish(&self) -> Option<&Vec<Dependency>> {
        self.on_finish.as_ref()
    }

    pub fn get_on_start(&self) -> Option<&Vec<Dependency>> {
        self.on_start.as_ref()
    }
}

impl TaskTemplate {
    pub fn get_dependencies(&self) -> Option<&Vec<Dependency>> {
        self.dependencies.as_ref()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow() {
        let xtomate_version = env!("CARGO_PKG_VERSION");
        let mut workflow = WorkFlow::new("test".to_string(), xtomate_version.to_string(), None);
        workflow.add_task("task1".to_string(), "echo Hello".to_string(), None);
        workflow.add_task(
            "task2".to_string(),
            "echo World".to_string(),
            Some(vec![Dependency::Simple("task1".to_string())]),
        );
        assert_eq!(
            workflow.get_task("task1").unwrap().command,
            Some("echo Hello".to_string())
        );
        assert_eq!(
            workflow.get_task("task2").unwrap().dependencies,
            Some(vec![Dependency::Simple("task1".to_string())])
        );
    }

    #[test]
    fn test_task() {
        let task = Task {
            command: Some("echo Hello".to_string()),
            plugin: None,
            template: None,
            config: None,
            run: None,
            retry: None,
            retry_delay: None,
            env: None,
            dependencies: Some(vec![Dependency::Simple("task1".to_string())]),
            on_start: None,
            on_finish: None,
            on_error: None,
        };
        assert_eq!(
            task.get_dependencies(),
            Some(&vec![Dependency::Simple("task1".to_string())])
        );
    }
}
