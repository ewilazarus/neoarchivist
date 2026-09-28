//! `neoarchivist preset`: saves a vault's Obsidian settings as a named preset, and applies
//! one to another vault. Presets live in `$XDG_CONFIG_HOME/neoarchivist/presets/<name>/`,
//! mirroring `.obsidian/`.
//!
//! A preset holds settings only, never notes or plugin code. Community plugins are saved as
//! their IDs plus their data.json, so they can be installed fresh. From workspace.json it
//! keeps only the layout, the sidebars and ribbon, never which notes are open.
//!
//! `apply` merges key by key. A key the vault doesn't set yet takes the preset's value. A
//! key the vault already sets to something else is a conflict: the vault keeps its value
//! unless `--prefer preset`. The layout is one conflict as a whole, and a vault's open
//! notes are kept either way.

use crate::config::Config;
use crate::util::write_file;
use anyhow::{Result, bail};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

/// Settings files merged key by key. Anything else in .obsidian/ is left alone.
const SETTINGS: [&str; 7] = [
    "app.json",
    "appearance.json",
    "core-plugins.json",
    "daily-notes.json",
    "hotkeys.json",
    "types.json",
    "graph.json",
];
/// graph.json keys that only record where the user last left the view.
const GRAPH_VOLATILE: [&str; 3] = ["scale", "close", "search"];
/// workspace.json keys that make up the layout. The rest is which notes are open.
const LAYOUT_KEYS: [&str; 3] = ["left", "right", "left-ribbon"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Prefer {
    Vault,
    Preset,
}

fn dir(config: &Config, name: &str) -> Result<PathBuf> {
    if name.is_empty() || name.contains('/') || name.starts_with('.') {
        bail!("Preset names are plain words, like 'mine'.");
    }
    Ok(config.presets_dir().join(name))
}

fn obsidian_dir(config: &Config) -> Result<PathBuf> {
    let vault = config.vault_dir();
    if !vault.is_dir() {
        bail!("No vault at {}.", vault.display());
    }
    Ok(vault.join(".obsidian"))
}

fn read_value(path: &Path) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(json!({}))
}

/// A settings file, with the graph's volatile keys left out. Missing files read as `{}`.
fn read_settings(path: &Path) -> Value {
    let mut v = read_value(path);
    if path.file_name().is_some_and(|n| n == "graph.json") {
        if let Value::Object(m) = &mut v {
            for k in GRAPH_VOLATILE {
                m.shift_remove(k);
            }
        }
    }
    v
}

/// The layout part of a workspace.json, with note references and search text removed.
fn layout_of(workspace: &Value) -> Value {
    fn scrub(v: &mut Value) {
        match v {
            Value::Object(m) => {
                m.shift_remove("file");
                for key in ["query", "searchQuery"] {
                    if m.contains_key(key) {
                        m.insert(key.into(), Value::String(String::new()));
                    }
                }
                for child in m.values_mut() {
                    scrub(child);
                }
            }
            Value::Array(a) => a.iter_mut().for_each(scrub),
            _ => {}
        }
    }
    let mut out = Map::new();
    if let Value::Object(m) = workspace {
        for k in LAYOUT_KEYS {
            if let Some(v) = m.get(k) {
                out.insert(k.into(), v.clone());
            }
        }
    }
    let mut out = Value::Object(out);
    scrub(&mut out);
    out
}

/// A main area with one empty tab, for a vault that has no workspace.json yet.
fn empty_main() -> Value {
    json!({"main": {"id": "5e0a4c1a0000main", "type": "split", "direction": "vertical",
      "children": [{"id": "5e0a4c1a0000tabs", "type": "tabs", "children": [{"id": "5e0a4c1a0000leaf",
      "type": "leaf", "state": {"type": "empty", "state": {}, "icon": "lucide-file", "title": "New tab"}}]}]}})
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default() + "\n"
}

/// Every file a preset holds, relative to its directory.
fn preset_files(dir: &Path) -> Vec<String> {
    let mut files: Vec<String> = SETTINGS
        .iter()
        .chain(["community-plugins.json"].iter())
        .filter(|f| dir.join(f).is_file())
        .map(|f| f.to_string())
        .collect();
    if dir.join("layout.json").is_file() {
        files.push("layout.json".into());
    }
    let mut data: Vec<String> = std::fs::read_dir(dir.join("plugins"))
        .map(|es| {
            es.flatten()
                .filter(|e| e.path().join("data.json").is_file())
                .map(|e| format!("plugins/{}/data.json", e.file_name().to_string_lossy()))
                .collect()
        })
        .unwrap_or_default();
    data.sort();
    files.extend(data);
    let mut snippets: Vec<String> = std::fs::read_dir(dir.join("snippets"))
        .map(|es| {
            es.flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "css"))
                .map(|e| format!("snippets/{}", e.file_name().to_string_lossy()))
                .collect()
        })
        .unwrap_or_default();
    snippets.sort();
    files.extend(snippets);
    files
}

pub fn list(config: &Config) -> String {
    let root = config.presets_dir();
    let mut names: Vec<(String, usize)> = std::fs::read_dir(&root)
        .map(|es| {
            es.flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| {
                    (
                        e.file_name().to_string_lossy().into_owned(),
                        preset_files(&e.path()).len(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    if names.is_empty() {
        return format!("No presets saved yet ({}).\n", root.display());
    }
    names.sort();
    names.iter().map(|(n, c)| format!("{n}\t{c} files\n")).collect()
}

pub fn save(config: &Config, name: &str) -> Result<String> {
    let obsidian = obsidian_dir(config)?;
    let dir = dir(config, name)?;
    if dir.exists() {
        bail!(
            "Preset '{name}' already exists at {}. Remove it first to replace it.",
            dir.display()
        );
    }
    std::fs::create_dir_all(&dir)?;
    for f in SETTINGS {
        if obsidian.join(f).is_file() {
            write_file(&dir.join(f), &pretty(&read_settings(&obsidian.join(f))))?;
        }
    }
    let plugins = obsidian.join("community-plugins.json");
    if plugins.is_file() {
        std::fs::copy(&plugins, dir.join("community-plugins.json"))?;
        for id in read_value(&plugins)
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let data = obsidian.join("plugins").join(id).join("data.json");
            if data.is_file() {
                std::fs::create_dir_all(dir.join("plugins").join(id))?;
                std::fs::copy(&data, dir.join("plugins").join(id).join("data.json"))?;
            }
        }
    }
    let workspace = obsidian.join("workspace.json");
    if workspace.is_file() {
        write_file(&dir.join("layout.json"), &pretty(&layout_of(&read_value(&workspace))))?;
    }
    if let Ok(entries) = std::fs::read_dir(obsidian.join("snippets")) {
        for e in entries
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "css"))
        {
            std::fs::create_dir_all(dir.join("snippets"))?;
            std::fs::copy(e.path(), dir.join("snippets").join(e.file_name()))?;
        }
    }
    let mut out = format!("Saved preset '{name}' to {}:\n", dir.display());
    for f in preset_files(&dir) {
        out += &format!("  {f}\n");
    }
    Ok(out)
}

fn unset(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => true,
        Some(Value::Array(a)) => a.is_empty(),
        Some(Value::Object(o)) => o.is_empty(),
        Some(Value::String(s)) => s.is_empty(),
        _ => false,
    }
}

/// One line per key: "add <key>" or "conflict <key> vault=<v> preset=<p>".
fn changes(vault: &Value, preset: &Value) -> Vec<(String, bool)> {
    let (Some(v), Some(p)) = (vault.as_object(), preset.as_object()) else {
        return Vec::new();
    };
    let mut keys: Vec<&String> = p.keys().collect();
    keys.sort();
    keys.into_iter()
        .filter_map(|k| {
            let (have, want) = (v.get(k), &p[k]);
            if have == Some(want) {
                None
            } else if unset(have) {
                Some((format!("add {k}"), false))
            } else {
                Some((
                    format!(
                        "conflict {k} vault={} preset={}",
                        have.map(|h| h.to_string()).unwrap_or_default(),
                        want
                    ),
                    true,
                ))
            }
        })
        .collect()
}

fn merge(vault: &Value, preset: &Value, prefer: Prefer) -> Value {
    let mut out = vault.as_object().cloned().unwrap_or_default();
    if let Some(p) = preset.as_object() {
        for (k, v) in p {
            if unset(out.get(k)) || prefer == Prefer::Preset {
                out.insert(k.clone(), v.clone());
            }
        }
    }
    Value::Object(out)
}

/// Shows what applying would change, and changes it when `apply`.
pub fn plan_or_apply(config: &Config, name: &str, apply: bool, prefer: Prefer) -> Result<String> {
    let obsidian = obsidian_dir(config)?;
    let dir = dir(config, name)?;
    if !dir.is_dir() {
        bail!("No preset named '{name}' in {}.", config.presets_dir().display());
    }
    let mut out = String::new();
    let mut any = false;
    for rel in preset_files(&dir) {
        let source = dir.join(&rel);
        let target = obsidian.join(&rel);
        if rel.starts_with("snippets/") {
            if target.is_file() {
                if std::fs::read(&source)? != std::fs::read(&target)? {
                    out += &format!("{rel}: exists and differs, kept the vault's\n");
                }
            } else {
                any = true;
                out += &format!("{rel}: new file\n");
                if apply {
                    std::fs::create_dir_all(target.parent().unwrap())?;
                    std::fs::copy(&source, &target)?;
                }
            }
            continue;
        }
        if rel == "layout.json" {
            let target = obsidian.join("workspace.json");
            let layout = read_value(&source);
            if !target.is_file() {
                any = true;
                out += "workspace.json: new file, layout only\n";
                if apply {
                    write_file(&target, &pretty(&merge(&empty_main(), &layout, Prefer::Preset)))?;
                }
            } else if layout_of(&read_value(&target)) != layout {
                any = true;
                match (apply, prefer) {
                    (true, Prefer::Preset) => {
                        out += "workspace.json: conflict layout -> took preset, open notes kept\n";
                        write_file(&target, &pretty(&merge(&read_value(&target), &layout, Prefer::Preset)))?;
                    }
                    (true, Prefer::Vault) => out += "workspace.json: conflict layout -> kept vault\n",
                    (false, _) => out += "workspace.json: conflict layout (the vault already has one)\n",
                }
            }
            continue;
        }
        if rel == "community-plugins.json" {
            let have: Vec<Value> = read_value(&target).as_array().cloned().unwrap_or_default();
            let want: Vec<Value> = read_value(&source).as_array().cloned().unwrap_or_default();
            let missing: Vec<Value> = want.into_iter().filter(|p| !have.contains(p)).collect();
            if !missing.is_empty() {
                any = true;
                out += &format!("{rel}:\n");
                for p in &missing {
                    out += &format!("  add {}\n", p.as_str().unwrap_or_default());
                }
                if apply {
                    let mut all = have;
                    all.extend(missing);
                    write_file(&target, &pretty(&Value::Array(all)))?;
                }
            }
            continue;
        }
        let preset = read_value(&source);
        let lines = changes(&read_settings(&target), &preset);
        if lines.is_empty() {
            continue;
        }
        any = true;
        if !target.is_file() {
            out += &format!("{rel}: new file, {} settings\n", lines.len());
            if apply {
                write_file(&target, &pretty(&preset))?;
            }
            continue;
        }
        out += &format!("{rel}:\n");
        for (line, conflict) in &lines {
            let outcome = match (apply, conflict, prefer) {
                (true, true, Prefer::Preset) => " -> took preset",
                (true, true, Prefer::Vault) => " -> kept vault",
                _ => "",
            };
            out += &format!("  {line}{outcome}\n");
        }
        if apply {
            // Merge into the full file, so the volatile graph keys the vault has are kept.
            write_file(&target, &pretty(&merge(&read_value(&target), &preset, prefer)))?;
        }
    }

    let graph = dir.join("graph.json");
    if graph.is_file() {
        let mut missing: Vec<String> = read_value(&graph)
            .get("colorGroups")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|g| g.get("query").and_then(Value::as_str))
            .flat_map(|q| {
                q.split_whitespace()
                    .filter_map(|w| w.strip_prefix("path:"))
                    .map(|p| p.trim_end_matches(')').to_string())
                    .collect::<Vec<_>>()
            })
            .filter(|p| !config.vault_dir().join(p).is_dir())
            .collect();
        missing.sort();
        missing.dedup();
        if !missing.is_empty() {
            out += &format!(
                "graph.json: colour groups match folders this vault doesn't have: {}\n",
                missing.join(" ")
            );
        }
    }
    let plugins = dir.join("community-plugins.json");
    if plugins.is_file() {
        for id in read_value(&plugins)
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if !obsidian.join("plugins").join(id).join("manifest.json").is_file() {
                out += &format!("not installed: {id}\n");
            }
        }
    }
    if !any {
        out += &format!("Nothing to change: the vault already has every setting in '{name}'.\n");
    }
    Ok(out)
}
