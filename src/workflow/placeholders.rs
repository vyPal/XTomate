use std::collections::HashMap;

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
}
