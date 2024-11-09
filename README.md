# XTomate
Your poweful automation ally

## Introduction
XTomate is a simple automation tool that allows you to define workflows in TOML files and run them. It is designed to be simple to use and easy to extend.

## Installation
The easiest way to install XTomate is to clone the repository and install with cargo.

```bash
git clone https://github.com/vyPal/XTomate.git
cd XTomate
cargo install --path .
```

## Usage
To run a workflow, you need to create a TOML file with the workflow definition and run the `xt` command with the path to the file.

```bash
xt run workflow
```

## Plugins
XTomate is designed to be extensible with plugins. Plugins are simple dynamic libraries that implement necessary traits. 
Each plugin needs to implement a `initialize` function, a `execute` function and a `teardown` function. The `initialize` function is called when the plugin is loaded, the `execute` function is called when the plugin is used and the `teardown` function is called when the plugin is unloaded.

Both the `initialize` and `execute` functions take a JSON object encoded as a string as an argument. This object contains the configuration for the plugin. The `teardown` function does not take any arguments.

All functions return a 32-bit integer. A return value of 0 indicates success, while a non-zero value indicates an error.

Any plugin that will be loaded should be placed in the `plugins` directory in the root of the project. The user may also specify a custom directory to load plugins from using the `--plugins` flag.

### Writing a plugin
(I will write a more detailed guide later, but for now just extend this template)

```rust
// Libraries used by the plugin
use std::ffi::CStr;
use libc::c_char;
use std::sync::{LazyLock, Mutex};
use serde::{Serialize, Deserialize};
use serde_json;

// Configuration struct for the plugin
#[derive(Serialize, Deserialize)]
struct PluginConfig {
    app_name: String,
}

// Input struct for the plugin
#[derive(Serialize, Deserialize)]
struct ExecutionInput {
    message: String,
}

// Global variable to store the app name
static APP_NAME: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));

// Plugin initialization function
#[no_mangle]
pub extern "C" fn initialize(config: *const c_char) -> i32 {
    let config_cstr = unsafe { CStr::from_ptr(config) };
    let config_str = config_cstr.to_str().unwrap_or("");
    
    let config: PluginConfig = serde_json::from_str(config_str).unwrap();
    let mut app_name = APP_NAME.lock().unwrap();
    *app_name = config.app_name.clone();
    0
}

// Plugin execution function
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

// Plugin teardown function
#[no_mangle]
pub extern "C" fn teardown() -> i32 {
    APP_NAME.lock().unwrap().clear();
    0
}
```

## Workflow TOML syntax
(Partially so that I don't forget)

```toml
# Basic workflow information
name = "example"
version = "0.1.0" # XTomate version required to run the workflow

# Tasks to run on special events
on_start = ["log_start"]
on_finish = ["notify_send", "log_done"]

# Plugin configurations
[[plugins]]
name = "notify_send" # Plugin name
source = "https://github.com/vyPal/xtomate-plugin-notify-send" # Plugin source
version = "^0.1.0" # Plugin version
config = { app_name = "XTomate" } # Plugin configuration
[[plugins]]
name = "logger"
source = "vyPal/xtomate-plugin-logger" # Shorter way to write source
version = "^0.1.0"
config = { app_name = "XTomate", log_file = "xtomate.log" }

# Task configurations
[tasks.notify_send]
run = false # This will prevent the task to run on its own, it will have to be called explicitly
plugin = "notify_send" # Plugin to use
config = { message = "Workflow finished" } # Plugin configuration

[tasks.log_done]
run = false
plugin = "logger"
config = { message = "Workflow finished", level = "info", sub_app_name = "Status" }

[tasks.log_start]
run = false
plugin = "logger"
config = { message = "Workflow started", level = "info", sub_app_name = "Status" }

[tasks.prepdir]
# Command to run
command = """
mkdir testdir
cd testdir
touch hello.py
"""

[tasks.createprogram]
command = '''
echo "print('hello world')" > testdir/hello.py
'''
dependencies = [{"prepdir" = "success"}] # Dependencies to run before this task

[tasks.writefile]
command = '''
echo "$WORLD $HELLO" > testdir/hello.txt
'''
dependencies = [{"prepdir" = "success"}] # Dependencies to run before this task with a specific status
env = {HELLO = "world", WORLD = "hello"} # Environment variables to set before running the command

[tasks.runprogram]
command = "python testdir/hello.py && cat testdir/hello.txt"
dependencies = [{"createprogram" = "success"}, {"writefile" = "success"}]
```
