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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context() {
        let mut context = Context::new();
        context.set("key".to_string(), "value".to_string());
        assert_eq!(context.resolve("{{key}}"), "value");
        assert_eq!(context.resolve("{{key}} {{key}}"), "value value");
        assert_eq!(context.resolve("{{key}} {{key2}}"), "value {{key2}}");
    }

    #[test]
    fn test_context_table() {
        let mut context = Context::new();
        context.set("key".to_string(), "value".to_string());
        let mut table = Table::new();
        table.insert("key".to_string(), toml::Value::String("{{key}}".to_string()));
        table.insert("key2".to_string(), toml::Value::String("{{key2}}".to_string()));
        let resolved = context.resolve_table(&table);
        assert_eq!(resolved.get("key").unwrap().as_str().unwrap(), "value");
        assert_eq!(resolved.get("key2").unwrap().as_str().unwrap(), "{{key2}}");
    }
}
