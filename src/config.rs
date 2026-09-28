//! Settings, from two files:
//!
//! - `.neoarchivist/config.toml` at the project root holds what defines the shared vault:
//!   where it lives, and what its memos and decisions folders are called. Everyone on the
//!   project must agree on these, so they are set per project only.
//! - `$XDG_CONFIG_HOME/neoarchivist/config.toml` (or `~/.config/…`) holds personal
//!   preferences. Setting a vault key there is an error, not a silent override.
//!
//! Both files are optional, and every key has a default.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The settings that define the vault, from `.neoarchivist/config.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Project {
    /// The vault, relative to the project root.
    pub vault: String,
    /// The folder of daily memos, relative to the vault.
    pub memos: String,
    /// The folder of decisions, relative to the vault.
    pub decisions: String,
}

impl Default for Project {
    fn default() -> Self {
        Project {
            vault: "docs".into(),
            memos: "Memos".into(),
            decisions: "Decisions".into(),
        }
    }
}

/// Personal preferences, from the user's config file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct User {
    /// Whether the Stop hook reminds Claude to record work before it finishes.
    pub stop_reminder: bool,
}

impl Default for User {
    fn default() -> Self {
        User { stop_reminder: true }
    }
}

pub const PROJECT_KEYS: [&str; 3] = ["vault", "memos", "decisions"];

#[derive(Debug, Clone)]
pub struct Config {
    /// The project root: where `.neoarchivist/` lives and the vault is found from.
    pub root: PathBuf,
    pub project: Project,
    pub user: User,
    /// The user config directory, `$XDG_CONFIG_HOME/neoarchivist`.
    pub user_dir: PathBuf,
}

impl Config {
    /// Loads the settings for the project at `root`, or the one found from the current
    /// directory: `$CLAUDE_PROJECT_DIR` when Claude Code runs us, else the enclosing git
    /// repository, else the current directory.
    pub fn load(root: Option<&Path>) -> Result<Config> {
        let root = match root {
            Some(root) => root.to_path_buf(),
            None => find_root()?,
        };
        let project = read_project(&root)?;
        let user_dir = user_dir();
        let user = read_user(&user_dir)?;
        Ok(Config {
            root,
            project,
            user,
            user_dir,
        })
    }

    pub fn vault_dir(&self) -> PathBuf {
        self.root.join(&self.project.vault)
    }

    pub fn memos(&self) -> &str {
        &self.project.memos
    }

    pub fn decisions(&self) -> &str {
        &self.project.decisions
    }

    /// Obsidian setting presets, next to the user's config file.
    pub fn presets_dir(&self) -> PathBuf {
        self.user_dir.join("presets")
    }

    pub fn project_file(&self) -> PathBuf {
        self.root.join(".neoarchivist/config.toml")
    }

    /// Sets the vault's settings for a project that has no settings file yet. Once the
    /// file exists it is the only place they change, so a different value is an error.
    pub fn set_project(
        &mut self,
        vault: Option<String>,
        memos: Option<String>,
        decisions: Option<String>,
    ) -> Result<()> {
        let exists = self.project_file().is_file();
        for (key, value, current) in [
            ("vault", vault, &mut self.project.vault),
            ("memos", memos, &mut self.project.memos),
            ("decisions", decisions, &mut self.project.decisions),
        ] {
            let Some(value) = value else { continue };
            if exists && value != *current {
                bail!(
                    "The project's `{key}` is already \"{current}\", in .neoarchivist/config.toml. Change it there, if it should change."
                );
            }
            check_folder(key, &value, Path::new(".neoarchivist/config.toml"))?;
            *current = value;
        }
        Ok(())
    }
}

/// A folder setting must stay inside the project.
fn check_folder(key: &str, value: &str, file: &Path) -> Result<()> {
    let path = Path::new(value);
    if value.trim().is_empty()
        || path.is_absolute()
        || path.components().any(|c| matches!(c, std::path::Component::ParentDir))
    {
        bail!(
            "{}: `{key}` must be a folder inside the project, not `{value}`.",
            file.display()
        );
    }
    Ok(())
}

fn find_root() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_PROJECT_DIR").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    let cwd = std::env::current_dir().context("can't read the current directory")?;
    let toplevel = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&cwd)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()));
    Ok(toplevel.unwrap_or(cwd))
}

fn user_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"));
    base.join("neoarchivist")
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn read_project(root: &Path) -> Result<Project> {
    let file = root.join(".neoarchivist/config.toml");
    if !file.is_file() {
        return Ok(Project::default());
    }
    let text = std::fs::read_to_string(&file).with_context(|| format!("can't read {}", file.display()))?;
    let project: Project = toml::from_str(&text).with_context(|| format!("{} isn't valid", file.display()))?;
    for (key, value) in [
        ("vault", &project.vault),
        ("memos", &project.memos),
        ("decisions", &project.decisions),
    ] {
        check_folder(key, value, &file)?;
    }
    Ok(project)
}

fn read_user(dir: &Path) -> Result<User> {
    let file = dir.join("config.toml");
    if !file.is_file() {
        return Ok(User::default());
    }
    let text = std::fs::read_to_string(&file).with_context(|| format!("can't read {}", file.display()))?;
    let table: toml::Table = toml::from_str(&text).with_context(|| format!("{} isn't valid", file.display()))?;
    if let Some(key) = PROJECT_KEYS.iter().find(|k| table.contains_key(**k)) {
        bail!(
            "{}: `{key}` defines the shared vault, so it is set per project, in .neoarchivist/config.toml, not in your own config.",
            file.display()
        );
    }
    toml::from_str(&text).with_context(|| format!("{} isn't valid", file.display()))
}
