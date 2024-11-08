use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub plugin_dir: String,
}

impl Config {
    pub fn default() -> Self {
        Config {
            plugin_dir: ProjectDirs::from("me", "vyPal", "XTomate")
                .unwrap()
                .data_dir()
                .to_str()
                .unwrap()
                .to_string(),
        }
    }

    pub fn get_plugin_dir(&self) -> &str {
        &self.plugin_dir
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = ProjectDirs::from("me", "vyPal", "XTomate")
            .unwrap()
            .config_dir()
            .join("config.toml");
        let config = std::fs::read_to_string(config_path)?;
        let config: Config = toml::from_str(&config)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = ProjectDirs::from("me", "vyPal", "XTomate")
            .unwrap()
            .config_dir()
            .join("config.toml");
        let toml_string = toml::to_string(self)?;
        std::fs::create_dir_all(config_path.clone())?;
        let mut file = File::create(config_path)?;
        file.write_all(toml_string.as_bytes())?;
        Ok(())
    }

    pub fn load_or_default(save: bool) -> Result<Self, Box<dyn std::error::Error>> {
        match Config::load() {
            Ok(config) => Ok(config),
            Err(_) => {
                let config = Config::default();
                if save {
                    config.save()?;
                }
                Ok(config)
            }
        }
    }
}
