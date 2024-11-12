use std::collections::HashMap;

use toml::Table;

#[derive(Default)]
pub struct Context {
    variables: HashMap<String, String>,
}

impl Context {
    pub fn new() -> Self {
        Context {
            variables: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: String, value: String) {
        self.variables.insert(key, value);
    }

    pub fn resolve(&self, input: &str) -> String {
        let mut resolved = input.to_string();
        for (key, value) in &self.variables {
            resolved = resolved.replace(&format!("{{{{{}}}}}", key), value);
        }
        resolved
    }

    pub fn resolve_table(&self, table: &Table) -> Table {
        let mut resolved = Table::new();
        for (key, value) in table {
            match value {
                toml::Value::String(s) => {
                    resolved.insert(key.clone(), toml::Value::String(self.resolve(s)));
                }
                toml::Value::Table(t) => {
                    resolved.insert(key.clone(), toml::Value::Table(self.resolve_table(t)));
                }
                _ => {
                    resolved.insert(key.clone(), value.clone());
                }
            }
        }
        resolved
    }
}
