//! `neoarchivist graph`: shows or sets the colour groups of the vault's graph view, in
//! `.obsidian/graph.json`.
//!
//! `show` prints the groups in order, with their colours as hex. `set` writes groups in the
//! order given, since the first group that matches a note colours it, and converts each
//! colour to the integer Obsidian stores. Groups already there were chosen by someone, so
//! `set` changes them only when told how: `--replace` swaps them all, and `--add` appends
//! the new ones, recolouring any group whose query is already there. Every other setting in
//! graph.json is kept, and the Groups panel is left open so the new groups show.

use crate::config::Config;
use crate::util::{cell, write_file};
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum How {
    Refuse,
    Replace,
    Add,
}

fn file(config: &Config) -> std::path::PathBuf {
    config.vault_dir().join(".obsidian/graph.json")
}

fn read(config: &Config) -> Result<Map<String, Value>> {
    let vault = config.vault_dir();
    if !vault.is_dir() {
        bail!("No vault at {}.", vault.display());
    }
    let path = file(config);
    if !path.is_file() {
        return Ok(Map::new());
    }
    let text = std::fs::read_to_string(&path)?;
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(map)) => Ok(map),
        _ => bail!("{} isn't valid JSON.", path.display()),
    }
}

/// The groups as a table.
pub fn table(graph: &Map<String, Value>) -> String {
    let groups = graph
        .get("colorGroups")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if groups.is_empty() {
        return "No colour groups.\n".into();
    }
    let mut out = String::from("| # | Colour | Query |\n|---|---|---|\n");
    for (i, g) in groups.iter().enumerate() {
        let rgb = g.pointer("/color/rgb").and_then(Value::as_u64).unwrap_or(0);
        let query = g.get("query").and_then(Value::as_str).unwrap_or("");
        out += &format!("| {} | #{:06X} | {} |\n", i + 1, rgb, cell(query));
    }
    out
}

pub fn show(config: &Config) -> Result<String> {
    Ok(table(&read(config)?))
}

/// Parses `query=#RRGGBB` into a colour group.
fn group(arg: &str) -> Result<Value> {
    let Some((query, hex)) = arg.rsplit_once('=') else {
        bail!("A group is '<query>=#RRGGBB', not '{arg}'.");
    };
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("'#{hex}' isn't a colour: use six hex digits, like #4C8BF5.");
    }
    if query.is_empty() {
        bail!("'{arg}' has no query.");
    }
    let rgb = u32::from_str_radix(hex, 16).context("bad colour")?;
    Ok(json!({"query": query, "color": {"a": 1, "rgb": rgb}}))
}

pub fn set(config: &Config, how: How, args: &[String]) -> Result<String> {
    let new: Vec<Value> = args.iter().map(|a| group(a)).collect::<Result<_>>()?;
    if new.is_empty() {
        bail!("Give at least one group, as '<query>=#RRGGBB'.");
    }
    let mut graph = read(config)?;
    let existing = graph
        .get("colorGroups")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !existing.is_empty() && how == How::Refuse {
        bail!(
            "{}graph.json already has {} colour groups, which someone chose. Pass --replace to swap them for these, or --add to keep them and add these.",
            table(&graph),
            existing.len()
        );
    }
    let groups = match how {
        How::Add => {
            let mut groups = existing;
            for g in new {
                match groups.iter_mut().find(|e| e.get("query") == g.get("query")) {
                    Some(e) => *e = g,
                    None => groups.push(g),
                }
            }
            groups
        }
        _ => new,
    };
    graph.insert("colorGroups".into(), Value::Array(groups));
    graph.insert("collapse-color-groups".into(), Value::Bool(false));
    let path = file(config);
    write_file(
        &path,
        &(serde_json::to_string_pretty(&Value::Object(graph.clone()))? + "\n"),
    )?;
    Ok(format!("Wrote {}:\n{}", path.display(), table(&graph)))
}
