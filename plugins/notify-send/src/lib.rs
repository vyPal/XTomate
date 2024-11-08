use std::ffi::CStr;
use libc::c_char;
use std::sync::{LazyLock, Mutex};
use serde::{Serialize, Deserialize};
use serde_json;

#[derive(Serialize, Deserialize)]
struct PluginConfig {
    app_name: String,
}

#[derive(Serialize, Deserialize)]
struct ExecutionInput {
    message: String,
}

static APP_NAME: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));

#[no_mangle]
pub extern "C" fn initialize(config: *const c_char) -> i32 {
    let config_cstr = unsafe { CStr::from_ptr(config) };
    let config_str = config_cstr.to_str().unwrap_or("");
    
    let config: PluginConfig = serde_json::from_str(config_str).unwrap();
    let mut app_name = APP_NAME.lock().unwrap();
    *app_name = config.app_name.clone();
    0
}

#[no_mangle]
pub extern "C" fn execute(input: *const c_char) -> i32 {
    let input_cstr = unsafe { CStr::from_ptr(input) };
    let input_str = input_cstr.to_str().unwrap_or("");
    
    let input_data: ExecutionInput = serde_json::from_str(input_str).unwrap();
    std::process::Command::new("notify-send")
        .arg("-a")
        .arg(APP_NAME.lock().unwrap().clone())
        .arg(input_data.message)
        .output()
        .expect("failed to execute process");
    0
}

#[no_mangle]
pub extern "C" fn teardown() -> i32 {
    APP_NAME.lock().unwrap().clear();
    0
}

