//! `neoarchivist obsidian`: checks that the Obsidian CLI targets this project's vault, and
//! reloads Obsidian safely.
//!
//! Every project's vault tends to have the same folder name, so the CLI's `vault=` is
//! ambiguous, and it talks to whichever vault was focused last. `check` compares the path
//! that vault reports with this one, symlinks resolved, and says what to do when they
//! differ. `reload` runs `obsidian command id=app:reload` only once `check` passes; if
//! Obsidian isn't running there is nothing to reload, since it reads its settings at start.

use crate::config::Config;
use std::path::Path;
use std::process::Command;

fn running() -> bool {
    ["Obsidian", "obsidian"].iter().any(|name| {
        Command::new("pgrep")
            .args(["-x", name])
            .output()
            .is_ok_and(|o| o.status.success())
    })
}

/// Whether `obsidian` is on the PATH, without running it, which could start the app.
fn installed() -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join("obsidian").is_file()))
}

fn real(path: &str) -> String {
    Path::new(path)
        .canonicalize()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.to_string())
}

fn cli(args: &[&str]) -> Option<String> {
    let out = Command::new("obsidian").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Why the CLI can't be used on this vault, or None when it can.
fn problem(vault: &str) -> Option<String> {
    if !installed() {
        return Some("The Obsidian CLI isn't installed.".into());
    }
    if !running() {
        return Some("Obsidian isn't running, so its CLI can't answer.".into());
    }
    let target = cli(&["vault", "info=path"])
        .and_then(|o| o.lines().next().map(str::to_string))
        .unwrap_or_default();
    if target.is_empty() {
        return Some("The Obsidian CLI didn't report a vault.".into());
    }
    if real(&target) == vault {
        return None;
    }
    let known = cli(&["vaults", "verbose"])
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('\t').map(|(_, p)| p.to_string()))
        .any(|p| real(&p) == vault);
    Some(if known {
        format!(
            "The Obsidian CLI targets {target}, not {vault}, because it talks to the vault focused last. Switching to this vault's window in Obsidian points it here."
        )
    } else {
        format!(
            "Obsidian hasn't opened {vault} as a vault yet, and targets {target}. Opening the vault folder with Open folder as vault adds it."
        )
    })
}

/// Prints the verdict, and returns whether the CLI targets this vault.
pub fn check(config: &Config) -> bool {
    let Some(vault) = vault_path(config) else { return false };
    match problem(&vault) {
        Some(why) => {
            println!("{why} Use rg and fd until it targets this vault.");
            false
        }
        None => {
            println!("The Obsidian CLI targets this vault, {vault}.");
            true
        }
    }
}

/// Reloads Obsidian if it's running and targets this vault. Returns false when it couldn't.
pub fn reload(config: &Config) -> bool {
    let Some(vault) = vault_path(config) else { return false };
    if !running() {
        println!("Obsidian isn't running, so there's nothing to reload. It reads the settings when it next starts.");
        return true;
    }
    if let Some(why) = problem(&vault) {
        println!(
            "Didn't reload. {why} Or ask the user to run Reload app without saving from the command palette, in this vault's window."
        );
        return false;
    }
    let _ = cli(&["command", "id=app:reload"]);
    println!("Reloaded Obsidian on {vault}.");
    true
}

fn vault_path(config: &Config) -> Option<String> {
    let vault = config.vault_dir();
    match vault.canonicalize() {
        Ok(p) => Some(p.display().to_string()),
        Err(_) => {
            println!("No vault at {}.", vault.display());
            None
        }
    }
}
