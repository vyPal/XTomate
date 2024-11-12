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
            .to_path_buf();
        let toml_string = toml::to_string(self)?;
        let _ = std::fs::create_dir_all(config_path.clone());
        let mut file = File::create(config_path.join("config.toml"))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let config = Config::default();
        assert_eq!(
            config.get_plugin_dir(),
            ProjectDirs::from("me", "vyPal", "XTomate")
                .unwrap()
                .data_dir()
                .to_str()
                .unwrap()
        );
    }

    #[test]
    fn test_load() {
        let config = Config::default();
        config.save().unwrap();
        let loaded_config = Config::load().unwrap();
        assert_eq!(config.get_plugin_dir(), loaded_config.get_plugin_dir());
    }

    #[test]
    fn test_load_or_default() {
        let config = Config::load_or_default(false).unwrap();
        assert_eq!(
            config.get_plugin_dir(),
            ProjectDirs::from("me", "vyPal", "XTomate")
                .unwrap()
                .data_dir()
                .to_str()
                .unwrap()
        );
    }

    #[test]
    fn test_save() {
        let config = Config::default();
        config.save().unwrap();
        let loaded_config = Config::load().unwrap();
        assert_eq!(config.get_plugin_dir(), loaded_config.get_plugin_dir());
    }

    #[test]
    fn test_save_load() {
        let config = Config::default();
        config.save().unwrap();
        let loaded_config = Config::load().unwrap();
        assert_eq!(config.get_plugin_dir(), loaded_config.get_plugin_dir());
    }

    #[test]
    fn test_save_load_or_default() {
        let config = Config::load_or_default(true).unwrap();
        let loaded_config = Config::load().unwrap();
        assert_eq!(config.get_plugin_dir(), loaded_config.get_plugin_dir());
    }
}
