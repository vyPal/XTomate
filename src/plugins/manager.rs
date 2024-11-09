use std::env::consts;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::{fs, path::PathBuf};

use colored::*;
use git2::build::{CheckoutBuilder, RepoBuilder};
use git2::{FetchOptions, Progress, RemoteCallbacks, Repository};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

#[derive(Serialize, Deserialize, Debug)]
pub struct PluginManager {
    pub plugins: Vec<Plugin>,
    pub plugin_dir: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Plugin {
    pub name: String,
    pub source: String, // Either a local path or a git URL
    pub version: Option<String>,

    #[serde(skip)]
    install_path: PathBuf,
    #[serde(skip)]
    build_path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    #[serde(rename = "runner_version")]
    pub xtomate_version: String,
    pub build: String,
    pub output_dir: String,
}

impl PluginManager {
    pub fn new(plugin_dir: PathBuf) -> Self {
        PluginManager {
            plugins: vec![],
            plugin_dir,
        }
    }

    pub fn load_or_default(
        plugin_dir: PathBuf,
        save: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(&plugin_dir)?;
        match std::fs::read_to_string(plugin_dir.join("plugins.toml")) {
            Ok(plugins) => {
                let plugins: PluginManager = toml::from_str(&plugins)?;
                Ok(plugins)
            }
            Err(_) => {
                let plugins = PluginManager::new(plugin_dir);
                if save {
                    plugins.save()?;
                }
                Ok(plugins)
            }
        }
    }

    pub fn add_plugin(&mut self, plugin: Plugin) {
        self.plugins.push(plugin);
    }

    pub fn get_plugin(&self, name: &str) -> Option<&Plugin> {
        self.plugins.iter().find(|p| p.name == name)
    }

    pub fn get_plugin_mut(&mut self, name: &str) -> Option<&mut Plugin> {
        self.plugins.iter_mut().find(|p| p.name == name)
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let toml_string = toml::to_string(self)?;
        let mut file = File::create(self.plugin_dir.join("plugins.toml"))?;
        file.write_all(toml_string.as_bytes())?;
        Ok(())
    }

    pub fn install_plugin(&mut self, name: String) -> Result<(), Box<dyn std::error::Error>> {
        let install_path = self.plugin_dir.join("installed").join(&name);
        fs::create_dir_all(&install_path)?;
        let build_path = self.plugin_dir.join("build").join(&name);
        fs::create_dir_all(&build_path)?;
        let plugin = self.get_plugin_mut(&name).unwrap();
        plugin.set_build_path(build_path.clone());

        if is_git_url(&mut plugin.source.to_string()) {
            let state = RefCell::new(State {
                progress: None,
                total: 0,
                current: 0,
                path: None,
                newline: false,
            });
            let mut cb = RemoteCallbacks::new();
            cb.transfer_progress(|stats| {
                let mut state = state.borrow_mut();
                state.progress = Some(stats.to_owned());
                print(&mut *state);
                true
            });

            let mut co = CheckoutBuilder::new();
            co.progress(|path, cur, total| {
                let mut state = state.borrow_mut();
                state.path = path.map(|p| p.to_path_buf());
                state.current = cur;
                state.total = total;
                print(&mut *state);
            });

            let mut fo = FetchOptions::new();
            fo.remote_callbacks(cb);

            if let Ok(repo) = Repository::open(build_path.clone()) {
                let mut remote = repo.remote_anonymous(&plugin.source)?;
                remote.fetch(&["refs/heads/*:refs/remotes/origin/*"], Some(&mut fo), None)?;
                let default_branch = remote.default_branch()?;
                let branch_refspec = default_branch.as_str().unwrap_or("refs/heads/main");
                remote.fetch(&[branch_refspec], Some(&mut fo), None)?;
                let head = repo.revparse_single("FETCH_HEAD")?;
                repo.checkout_tree(&head, None)?;
                repo.set_head_detached(head.id())?;
            } else {
                RepoBuilder::new()
                    .fetch_options(fo)
                    .with_checkout(co)
                    .clone(&plugin.source, &build_path)?;
            }
        } else {
            let local_path = Path::new(&plugin.source);
            if !local_path.exists() {
                return Err("Local path does not exist".into());
            }

            fs::copy(local_path, &build_path)?;
        }

        let manifest_path = build_path.join("plugin.toml");
        let manifest = fs::read_to_string(&manifest_path)?;
        let manifest: PluginManifest = toml::from_str(&manifest)?;

        if manifest.name != name {
            return Err("Plugin name does not match manifest name".into());
        }

        let plugin_version_req = VersionReq::parse(&plugin.version.as_ref().unwrap())?;
        let plugin_version = Version::parse(&manifest.version)?;
        if !plugin_version_req.matches(&plugin_version) {
            return Err("Plugin version does not match manifest version".into());
        }

        let plugin_xtomate_version_req = VersionReq::parse(&manifest.xtomate_version)?;
        let xtomate_version = Version::parse(env!("CARGO_PKG_VERSION"))?;
        if !plugin_xtomate_version_req.matches(&xtomate_version) {
            return Err("XTomate version does not match manifest version".into());
        }

        println!("{}", format!("Building plugin: {}", name).green());
        let build_command = Command::new("sh")
            .arg("-c")
            .arg(&manifest.build)
            .current_dir(&build_path)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .output()?;

        if !build_command.status.success() {
            return Err("Failed to build plugin".into());
        }

        let output_path = build_path.join(&manifest.output_dir).join(format!(
            "{}{}{}",
            consts::DLL_PREFIX,
            name,
            consts::DLL_SUFFIX
        ));
        let install_path = install_path.join(format!(
            "{}{}{}",
            consts::DLL_PREFIX,
            name,
            consts::DLL_SUFFIX
        ));
        plugin.set_install_path(install_path.clone());
        fs::create_dir_all(install_path.parent().unwrap())?;
        fs::copy(output_path, install_path)?;

        Ok(())
    }

    pub fn verify_plugin(
        &mut self,
        name: String,
        source: String,
        version: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let plugin = self.get_plugin(&name);
        if plugin.is_none() {
            let plugin_name = name.clone();
            let mut plugin = Plugin::new(name, source);
            plugin.set_version(version.unwrap_or("0.1.0".to_string()));
            self.add_plugin(plugin);
            self.install_plugin(plugin_name)?;
            self.save()?;
        } else {
            let plugin = plugin.unwrap();
            self.install_plugin(plugin.name.clone())?;
            self.save()?;
        }
        Ok(())
    }
}

impl Plugin {
    pub fn new(name: String, source: String) -> Self {
        Plugin {
            name,
            source,
            version: None,
            install_path: PathBuf::new(),
            build_path: PathBuf::new(),
        }
    }

    pub fn set_version(&mut self, version: String) {
        self.version = Some(version);
    }

    pub fn get_install_path(&self) -> &PathBuf {
        &self.install_path
    }

    pub fn set_install_path(&mut self, install_path: PathBuf) {
        self.install_path = install_path;
    }

    pub fn set_build_path(&mut self, build_path: PathBuf) {
        self.build_path = build_path;
    }
}

fn is_git_url(source: &mut String) -> bool {
    if source.starts_with("http://") || source.starts_with("https://") || source.ends_with(".git") {
        return true;
    }

    let parts: Vec<&str> = source.split('/').collect();
    if parts.len() == 2 {
        let local_path = Path::new(&source);

        if local_path.exists() {
            return false;
        }

        // Directly assign the formatted string to `*source`
        *source = format!("https://github.com/{}", source);
        return true;
    }

    false
}


struct State {
    progress: Option<Progress<'static>>,
    total: usize,
    current: usize,
    path: Option<PathBuf>,
    newline: bool,
}

fn print(state: &mut State) {
    let stats = state.progress.as_ref().unwrap();
    let network_pct = (100 * stats.received_objects()) / stats.total_objects();
    let index_pct = (100 * stats.indexed_objects()) / stats.total_objects();
    let co_pct = if state.total > 0 {
        (100 * state.current) / state.total
    } else {
        0
    };
    let kbytes = stats.received_bytes() / 1024;
    if stats.received_objects() == stats.total_objects() {
        if !state.newline {
            println!();
            state.newline = true;
        }
        print!(
            "Resolving deltas {}/{}\r",
            stats.indexed_deltas(),
            stats.total_deltas()
        );
    } else {
        print!(
            "net {:3}% ({:4} kb, {:5}/{:5})  /  idx {:3}% ({:5}/{:5})  \
             /  chk {:3}% ({:4}/{:4}) {}\r",
            network_pct,
            kbytes,
            stats.received_objects(),
            stats.total_objects(),
            index_pct,
            stats.indexed_objects(),
            stats.total_objects(),
            co_pct,
            state.current,
            state.total,
            state
                .path
                .as_ref()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        )
    }
    std::io::stdout().flush().unwrap();
}
