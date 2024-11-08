use colored::*;
use libc::c_char;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json;
use std::ffi::CStr;
use std::fs::OpenOptions;
use std::io::Write;
use std::str::FromStr;
use std::sync::{LazyLock, Mutex};

#[derive(Serialize, PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

impl FromStr for LogLevel {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "info" => Ok(LogLevel::Info),
            "debug" => Ok(LogLevel::Debug),
            "warn" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            _ => Err(()),
        }
    }
}

impl<'de> Deserialize<'de> for LogLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = String::deserialize(deserializer)?;
        s.to_lowercase()
            .parse::<LogLevel>()
            .map_err(|e| serde::de::Error::custom(format!("Invalid log level: {:?}", e)))
    }
}

fn default_log_file() -> String {
    "default.log".to_string()
}

fn default_log_to_file() -> bool {
    true
}

fn default_log_to_console() -> bool {
    true
}

fn default_minimum_log_level() -> LogLevel {
    LogLevel::Info
}

fn default_max_log_file_size() -> u64 {
    10 * 1024 * 1024 // 10MB
}

fn default_max_log_file_count() -> u32 {
    5
}

fn default_enable_log_rotation() -> bool {
    true
}

fn default_log_to_file_colored() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
struct PluginConfig {
    app_name: String,

    #[serde(default = "default_log_file")]
    log_file: String,

    #[serde(default = "default_log_to_file")]
    log_to_file: bool,

    #[serde(default = "default_log_to_console")]
    log_to_console: bool,

    #[serde(default = "default_minimum_log_level")]
    minimum_log_level: LogLevel,

    #[serde(default = "default_max_log_file_size")]
    max_log_file_size: u64,

    #[serde(default = "default_max_log_file_count")]
    max_log_file_count: u32,

    #[serde(default = "default_enable_log_rotation")]
    enable_log_rotation: bool,

    #[serde(default = "default_log_to_file_colored")]
    log_to_file_colored: bool,
}

#[derive(Serialize, Deserialize)]
struct ExecutionInput {
    message: String,

    #[serde(default = "default_minimum_log_level")]
    level: LogLevel,

    app_name: Option<String>,

    sub_app_name: Option<String>,
}

static APP_NAME: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));
static LOG_FILE: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));
static LOG_TO_FILE: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(false));
static LOG_TO_CONSOLE: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(false));
static MIN_LOG_LEVEL: LazyLock<Mutex<LogLevel>> = LazyLock::new(|| Mutex::new(LogLevel::Info));
static MAX_LOG_FILE_SIZE: LazyLock<Mutex<u64>> = LazyLock::new(|| Mutex::new(10 * 1024 * 1024)); // 10MB
static MAX_LOG_FILE_COUNT: LazyLock<Mutex<u32>> = LazyLock::new(|| Mutex::new(5));
static ENABLE_LOG_ROTATION: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(true));
static LOG_TO_FILE_COLORED: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(false));

#[no_mangle]
pub extern "C" fn initialize(config: *const c_char) -> i32 {
    let config_cstr = unsafe { CStr::from_ptr(config) };
    let config_str = config_cstr.to_str().unwrap_or("");

    let config: PluginConfig = serde_json::from_str(config_str).unwrap();

    *APP_NAME.lock().unwrap() = config.app_name.clone();
    *LOG_FILE.lock().unwrap() = config.log_file.clone();
    *LOG_TO_FILE.lock().unwrap() = config.log_to_file;
    *LOG_TO_CONSOLE.lock().unwrap() = config.log_to_console;
    *MIN_LOG_LEVEL.lock().unwrap() = config.minimum_log_level;
    *MAX_LOG_FILE_SIZE.lock().unwrap() = config.max_log_file_size;
    *MAX_LOG_FILE_COUNT.lock().unwrap() = config.max_log_file_count;
    *ENABLE_LOG_ROTATION.lock().unwrap() = config.enable_log_rotation;
    *LOG_TO_FILE_COLORED.lock().unwrap() = config.log_to_file_colored;

    0
}

fn rotate_log_file() {
    let enable_rotation = *ENABLE_LOG_ROTATION.lock().unwrap();
    if !enable_rotation {
        return;
    }

    let log_file_path = LOG_FILE.lock().unwrap().clone();
    let max_size = *MAX_LOG_FILE_SIZE.lock().unwrap();
    let max_count = *MAX_LOG_FILE_COUNT.lock().unwrap();

    let log_metadata = std::fs::metadata(&log_file_path);

    if let Ok(metadata) = log_metadata {
        if metadata.len() > max_size {
            let mut log_files: Vec<String> = std::fs::read_dir(".")
                .unwrap()
                .filter_map(|entry| {
                    entry.ok().and_then(|e| {
                        let path = e.path();
                        if path.is_file() && path.file_name()?.to_str()?.starts_with("log") {
                            Some(path.display().to_string())
                        } else {
                            None
                        }
                    })
                })
                .collect();

            log_files.sort();

            if log_files.len() >= max_count as usize {
                let file_to_remove = log_files.remove(0);
                std::fs::remove_file(file_to_remove).unwrap_or(());
            }

            let timestamp = chrono::Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();
            let archive_name = format!("{}_{}.log", log_file_path, timestamp);
            std::fs::rename(&log_file_path, archive_name).unwrap();
        }
    }
}

#[no_mangle]
pub extern "C" fn execute(input: *const c_char) -> i32 {
    let input_cstr = unsafe { CStr::from_ptr(input) };
    let input_str = input_cstr.to_str().unwrap_or("");

    let input_data: ExecutionInput = serde_json::from_str(input_str).unwrap();
    let min_level = MIN_LOG_LEVEL.lock().unwrap().clone();

    if input_data.level < min_level {
        return 0;
    }

    let mut app_name = APP_NAME.lock().unwrap().clone();
    if let Some(override_app_name) = input_data.app_name {
        app_name = override_app_name;
    }
    if let Some(sub_app_name) = input_data.sub_app_name {
        app_name = format!("{} -> {}", app_name, sub_app_name);
    }
    let timestamp = chrono::Utc::now().to_string();
    let timestamp_colored = timestamp.bright_red();
    let level_colored = match input_data.level {
        LogLevel::Debug => input_data.level.to_string().blue(),
        LogLevel::Info => input_data.level.to_string().green(),
        LogLevel::Warn => input_data.level.to_string().yellow(),
        LogLevel::Error => input_data.level.to_string().red(),
    };

    let app_name_colored = app_name.cyan();
    let message_colored = input_data.message.white();

    let log_message = format!(
        "[{}] [{}] {}: {}",
        timestamp_colored, level_colored, app_name_colored, message_colored
    );

    if *LOG_TO_CONSOLE.lock().unwrap() {
        println!("{}", log_message);
    }

    if *LOG_TO_FILE.lock().unwrap() {
        rotate_log_file();

        let log_file_path = LOG_FILE.lock().unwrap().clone();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)
            .expect("Failed to open log file");

        if *LOG_TO_FILE_COLORED.lock().unwrap() {
            if let Err(e) = writeln!(file, "{}", log_message) {
                eprintln!("Failed to write to log file: {}", e);
                return -1;
            }
        } else {
            let log_message_plain = format!(
                "[{}] [{}] {}: {}",
                timestamp,
                input_data.level.to_string(),
                app_name,
                input_data.message
            );
            if let Err(e) = writeln!(file, "{}", log_message_plain) {
                eprintln!("Failed to write to log file: {}", e);
                return -1;
            }
        }
    }

    0
}

#[no_mangle]
pub extern "C" fn teardown() -> i32 {
    APP_NAME.lock().unwrap().clear();
    LOG_FILE.lock().unwrap().clear();
    *LOG_TO_FILE.lock().unwrap() = false;
    *LOG_TO_CONSOLE.lock().unwrap() = false;
    0
}
