//! `neoarchivist init plan|apply`: makes init's file changes in a project, without
//! overwriting anything. It:
//!
//! - writes `.neoarchivist/config.toml`, if the project has none, with the settings in effect;
//! - adds the two marketplaces and plugins to `.claude/settings.json`, keeping the rest;
//! - creates the vault's memos and decisions folders;
//! - points the Daily notes core plugin at the memos folder, if it is enabled;
//! - writes the neoarchivist section into CLAUDE.md, replacing the text between its markers
//!   in CLAUDE.md, .claude/CLAUDE.md or AGENTS.md, or else appending it to CLAUDE.md. A
//!   project with only AGENTS.md needs `--agents-md`: `import` creates CLAUDE.md with
//!   `@AGENTS.md` and the section, and `append` adds the section to AGENTS.md;
//! - adds to .gitignore the Obsidian per-user settings it doesn't ignore yet, and with
//!   `--ignore-plugins` the community plugins too;
//! - with `--checks`, installs a pre-commit hook that runs `neoarchivist check --staged`,
//!   unless the clone already has one, and on GitHub a workflow that runs the check on
//!   every push and pull request with this version's release.
//!
//! A setting that already has a different value is reported as kept, never changed, so a
//! plugin someone disabled stays disabled. Running it again changes nothing. Only the
//! project's own .gitignore files count: a global or .git/info/exclude rule is personal.

use crate::config::Config;
use crate::util::{git, write_file};
use anyhow::{Result, bail};
use serde_json::{Map, Value, json};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const SECTION: &str = include_str!("../assets/CLAUDE-section.md");
const START: &str = "<!-- neoarchivist:start -->";
const END: &str = "<!-- neoarchivist:end -->";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AgentsMd {
    Import,
    Append,
}

pub struct Options {
    pub apply: bool,
    pub agents_md: Option<AgentsMd>,
    pub ignore_plugins: bool,
    pub checks: bool,
}

struct Run<'a> {
    config: &'a Config,
    opts: &'a Options,
    out: String,
    changed: bool,
}

impl Run<'_> {
    fn path(&self, rel: &str) -> PathBuf {
        self.config.root.join(rel)
    }

    fn change(&mut self, line: impl AsRef<str>) {
        self.changed = true;
        self.say(line);
    }

    fn say(&mut self, line: impl AsRef<str>) {
        let _ = writeln!(self.out, "{}", line.as_ref());
    }

    fn new_file(&self, rel: &str) -> &'static str {
        if self.path(rel).exists() { "" } else { " new file" }
    }
}

/// The section, with the vault's folder filled in.
fn section(config: &Config) -> String {
    SECTION.replace("{{vault}}", &config.project.vault)
}

pub fn run(config: &Config, opts: &Options) -> Result<String> {
    let mut r = Run {
        config,
        opts,
        out: String::new(),
        changed: false,
    };
    // Refuse before changing anything, rather than stop halfway.
    if let Some(file) = marked_file(&r) {
        if replaced(&std::fs::read_to_string(r.path(&file))?, &section(config)).is_none() {
            bail!("{file} has the neoarchivist start marker but no end marker. Fix it by hand, then run init again.");
        }
    }
    if opts.apply
        && opts.agents_md.is_none()
        && marked_file(&r).is_none()
        && claude_md(&r).is_none()
        && r.path("AGENTS.md").is_file()
    {
        bail!("The project has AGENTS.md but no CLAUDE.md. Pass --agents-md import or --agents-md append.");
    }
    config_file(&mut r)?;
    settings(&mut r)?;
    folders(&mut r)?;
    daily_notes(&mut r)?;
    claude_section(&mut r)?;
    gitignore(&mut r)?;
    checks(&mut r)?;
    if !r.changed {
        r.say("Nothing to change: the project already has everything init sets up.");
    }
    Ok(r.out)
}

fn config_file(r: &mut Run) -> Result<()> {
    let rel = ".neoarchivist/config.toml";
    if r.path(rel).is_file() {
        return Ok(());
    }
    let p = &r.config.project;
    r.change(format!(
        "{rel}: new file, with vault = \"{}\", memos = \"{}\", decisions = \"{}\"",
        p.vault, p.memos, p.decisions
    ));
    if r.opts.apply {
        let text = format!(
            "# neoarchivist's settings for this project, shared by everyone who works on it.\n\n\
             # The Obsidian vault, relative to the project root.\nvault = \"{}\"\n\n\
             # The folder of daily memos, and the folder of decisions, inside the vault.\nmemos = \"{}\"\ndecisions = \"{}\"\n",
            p.vault, p.memos, p.decisions
        );
        write_file(&r.path(rel), &text)?;
    }
    Ok(())
}

fn wanted_settings() -> Value {
    json!({
        "extraKnownMarketplaces": {
            "obsidian-skills": {"source": {"source": "github", "repo": "kepano/obsidian-skills"}},
            "neoarchivist": {"source": {"source": "github", "repo": "ewilazarus/neoarchivist"}}
        },
        "enabledPlugins": {"obsidian@obsidian-skills": true, "neoarchivist@neoarchivist": true}
    })
}

fn settings(r: &mut Run) -> Result<()> {
    let rel = ".claude/settings.json";
    let path = r.path(rel);
    let mut current: Map<String, Value> = if path.is_file() {
        match serde_json::from_str(&std::fs::read_to_string(&path)?) {
            Ok(Value::Object(m)) => m,
            _ => bail!("{rel} isn't valid JSON. Fix it, then run init again."),
        }
    } else {
        Map::new()
    };
    let wanted = wanted_settings();
    let mut lines = Vec::new();
    let mut adds = false;
    for (section, entries) in wanted.as_object().unwrap() {
        for (key, value) in entries.as_object().unwrap() {
            match current.get(section).and_then(|s| s.get(key)) {
                None | Some(Value::Null) => {
                    lines.push(format!("  add {section}.{key}"));
                    adds = true;
                }
                Some(v) if v == value => {}
                Some(v) => lines.push(format!("  kept {section}.{key}, already {v}")),
            }
        }
    }
    if lines.is_empty() {
        return Ok(());
    }
    if adds {
        let header = format!("{rel}:{}", r.new_file(rel));
        r.change(header);
    } else {
        r.say(format!("{rel}:"));
    }
    for l in &lines {
        r.say(l);
    }
    if r.opts.apply && adds {
        for (section, entries) in wanted.as_object().unwrap() {
            let s = current.entry(section.clone()).or_insert_with(|| json!({}));
            if let Value::Object(s) = s {
                for (key, value) in entries.as_object().unwrap() {
                    if s.get(key).is_none_or(Value::is_null) {
                        s.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        write_file(&path, &(serde_json::to_string_pretty(&Value::Object(current))? + "\n"))?;
    }
    Ok(())
}

fn folders(r: &mut Run) -> Result<()> {
    let vault = &r.config.project.vault;
    for folder in [r.config.memos().to_string(), r.config.decisions().to_string()] {
        let rel = format!("{vault}/{folder}");
        if r.path(&rel).is_dir() {
            continue;
        }
        r.change(format!("{rel}/: new folder"));
        if r.opts.apply {
            std::fs::create_dir_all(r.path(&rel))?;
        }
    }
    Ok(())
}

fn daily_notes(r: &mut Run) -> Result<()> {
    let vault = r.config.project.vault.clone();
    let core = r.path(&format!("{vault}/.obsidian/core-plugins.json"));
    let enabled = std::fs::read_to_string(&core)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .is_some_and(|v| match v {
            Value::Array(a) => a.iter().any(|x| x == "daily-notes"),
            Value::Object(o) => o.get("daily-notes") == Some(&Value::Bool(true)),
            _ => false,
        });
    if !enabled {
        return Ok(());
    }
    let rel = format!("{vault}/.obsidian/daily-notes.json");
    let path = r.path(&rel);
    let mut current: Map<String, Value> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let folder = current.get("folder").and_then(Value::as_str).unwrap_or("").to_string();
    let memos = r.config.memos().to_string();
    if folder.trim_end_matches('/') == memos {
        return Ok(());
    }
    if !folder.is_empty() {
        r.say(format!("{rel}:\n  kept folder, already \"{folder}\""));
        return Ok(());
    }
    let header = format!("{rel}:{}", r.new_file(&rel));
    r.change(header);
    r.say(format!("  add folder \"{memos}/\""));
    if r.opts.apply {
        current.insert("folder".into(), Value::String(format!("{memos}/")));
        write_file(&path, &(serde_json::to_string_pretty(&Value::Object(current))? + "\n"))?;
    }
    Ok(())
}

/// The file with the neoarchivist markers, if any.
fn marked_file(r: &Run) -> Option<String> {
    ["CLAUDE.md", ".claude/CLAUDE.md", "AGENTS.md"]
        .into_iter()
        .find_map(|f| {
            std::fs::read_to_string(r.path(f))
                .ok()
                .filter(|t| t.lines().any(|l| l == START))
                .map(|_| f.to_string())
        })
}

/// The CLAUDE.md to append to, if there is one.
fn claude_md(r: &Run) -> Option<String> {
    ["CLAUDE.md", ".claude/CLAUDE.md"]
        .into_iter()
        .find(|f| r.path(f).is_file())
        .map(String::from)
}

/// The text with everything between its markers replaced by the section, or None when the
/// start marker has no end marker after it.
fn replaced(text: &str, section: &str) -> Option<String> {
    let mut out = String::new();
    let mut lines = text.split_inclusive('\n');
    let mut done = false;
    while let Some(line) = lines.next() {
        if !done && line.trim_end_matches(['\n', '\r']) == START {
            out.push_str(section);
            loop {
                let l = lines.next()?;
                if l.trim_end_matches(['\n', '\r']) == END {
                    break;
                }
            }
            done = true;
        } else {
            out.push_str(line);
        }
    }
    Some(out)
}

fn append_section(path: &Path, section: &str) -> Result<()> {
    let mut text = std::fs::read_to_string(path).unwrap_or_default();
    if !text.is_empty() {
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text.push('\n');
    }
    text.push_str(section);
    write_file(path, &text)
}

fn claude_section(r: &mut Run) -> Result<()> {
    let section = section(r.config);
    if let Some(file) = marked_file(r) {
        let text = std::fs::read_to_string(r.path(&file))?;
        let new = replaced(&text, &section).unwrap_or_default();
        if new != text {
            r.change(format!("{file}: update the neoarchivist section"));
            if r.opts.apply {
                write_file(&r.path(&file), &new)?;
            }
        }
        return Ok(());
    }
    if let Some(file) = claude_md(r) {
        r.change(format!("{file}: append the neoarchivist section"));
        if r.opts.apply {
            append_section(&r.path(&file), &section)?;
        }
    } else if r.path("AGENTS.md").is_file() {
        match r.opts.agents_md {
            Some(AgentsMd::Import) => {
                r.change("CLAUDE.md: new file, importing AGENTS.md, with the neoarchivist section");
                if r.opts.apply {
                    write_file(&r.path("CLAUDE.md"), &format!("@AGENTS.md\n\n{section}"))?;
                }
            }
            Some(AgentsMd::Append) => {
                r.change("AGENTS.md: append the neoarchivist section");
                if r.opts.apply {
                    append_section(&r.path("AGENTS.md"), &section)?;
                }
            }
            None => r.change(
                "CLAUDE.md: needs a choice. The project has AGENTS.md but no CLAUDE.md: --agents-md import creates CLAUDE.md importing AGENTS.md, and --agents-md append adds the section to AGENTS.md.",
            ),
        }
    } else {
        r.change("CLAUDE.md: new file, with the neoarchivist section");
        if r.opts.apply {
            write_file(&r.path("CLAUDE.md"), &section)?;
        }
    }
    Ok(())
}

/// Whether a .gitignore in the project already ignores the path.
fn ignored(r: &Run, in_git: bool, path: &str) -> bool {
    let probe = if path.ends_with('/') {
        format!("{path}x")
    } else {
        path.to_string()
    };
    if !in_git {
        return std::fs::read_to_string(r.path(".gitignore")).is_ok_and(|t| t.lines().any(|l| l == path));
    }
    let Some(out) = git(&r.config.root, &["check-ignore", "-v", "--no-index", &probe]) else {
        return false;
    };
    let Some((meta, _)) = out.trim_end().split_once('\t') else {
        return false;
    };
    let mut parts = meta.splitn(3, ':');
    let (source, pattern) = (parts.next().unwrap_or(""), parts.nth(1).unwrap_or(""));
    let from_project = !source.starts_with('/')
        && !source.starts_with(".git/")
        && (source == ".gitignore" || source.ends_with("/.gitignore"));
    from_project && !pattern.starts_with('!')
}

fn gitignore(r: &mut Run) -> Result<()> {
    let vault = r.config.project.vault.clone();
    let in_git = git(&r.config.root, &["rev-parse", "--is-inside-work-tree"]).is_some();
    let personal: Vec<String> = [
        "workspace.json",
        "workspace-mobile.json",
        "graph.json",
        "app.json",
        "appearance.json",
    ]
    .iter()
    .map(|f| format!("{vault}/.obsidian/{f}"))
    .filter(|p| !ignored(r, in_git, p))
    .collect();
    let plugin_paths = [
        format!("{vault}/.obsidian/plugins/"),
        format!("{vault}/.obsidian/community-plugins.json"),
    ];
    let plugins: Vec<String> = if r.opts.ignore_plugins {
        plugin_paths
            .iter()
            .filter(|p| !ignored(r, in_git, p))
            .cloned()
            .collect()
    } else {
        Vec::new()
    };
    let mut shown = false;
    if !personal.is_empty() || !plugins.is_empty() {
        let header = format!(".gitignore:{}", r.new_file(".gitignore"));
        r.change(header);
        shown = true;
        for p in personal.iter().chain(&plugins) {
            r.say(format!("  add {p}"));
        }
        if r.opts.apply {
            let path = r.path(".gitignore");
            let mut text = std::fs::read_to_string(&path).unwrap_or_default();
            let group = |text: &mut String, comment: &str, paths: &[String]| {
                if paths.is_empty() {
                    return;
                }
                if !text.is_empty() {
                    if !text.ends_with('\n') {
                        text.push('\n');
                    }
                    text.push('\n');
                }
                text.push_str(comment);
                text.push('\n');
                for p in paths {
                    text.push_str(p);
                    text.push('\n');
                }
            };
            group(
                &mut text,
                "# Obsidian: each user's own panes, graph view, editor preferences and theme",
                &personal,
            );
            group(
                &mut text,
                "# Obsidian: community plugins, installed by each user",
                &plugins,
            );
            write_file(&path, &text)?;
        }
    }
    let has_plugins = r.path(&format!("{vault}/.obsidian/community-plugins.json")).is_file();
    if !r.opts.ignore_plugins && has_plugins && plugin_paths.iter().any(|p| !ignored(r, in_git, p)) {
        if !shown {
            r.say(".gitignore:");
        }
        r.say("  note: the vault has community plugins; --ignore-plugins ignores them too");
    }
    Ok(())
}

const PRE_COMMIT: &str = "#!/bin/sh\n\
# neoarchivist: check the docs vault before each commit. Skip once with --no-verify.\n\
command -v neoarchivist >/dev/null 2>&1 || { echo \"neoarchivist isn't installed, so the vault wasn't checked\" >&2; exit 0; }\n\
exec neoarchivist check --staged\n";

fn workflow() -> String {
    format!(
        r#"name: neoarchivist

# Checks the docs vault in every pushed commit: lint errors in the notes that changed, and
# the rule that past memos and older decisions keep their prose. `neoarchivist init`
# keeps the version in step with the plugin.

on:
  push:
  pull_request:

jobs:
  vault:
    runs-on: ubuntu-latest
    env:
      NEOARCHIVIST_VERSION: "{VERSION}"
    steps:
      - uses: actions/checkout@v5
        with:
          fetch-depth: 0
      - name: Install neoarchivist
        run: |
          curl -fsSL "https://github.com/ewilazarus/neoarchivist/releases/download/v$NEOARCHIVIST_VERSION/neoarchivist-x86_64-unknown-linux-musl.tar.gz" |
            tar -xz -C /usr/local/bin neoarchivist
      - name: Check the docs vault
        env:
          BASE: ${{{{ github.event.pull_request.base.sha || github.event.before }}}}
          DEFAULT_BRANCH: ${{{{ github.event.repository.default_branch }}}}
        run: |
          # A new branch has no "before": check what it adds to the default branch.
          if ! git cat-file -e "$BASE^{{commit}}" 2>/dev/null; then
            BASE=$(git merge-base HEAD "origin/$DEFAULT_BRANCH" 2>/dev/null || git rev-list --max-parents=0 HEAD | tail -n 1)
          fi
          neoarchivist check --range "$BASE..HEAD"
"#
    )
}

fn checks(r: &mut Run) -> Result<()> {
    let rel = ".github/workflows/neoarchivist.yml";
    let path = r.path(rel);
    // Once installed, the workflow is kept in step with this version on every run.
    if path.is_file() {
        let want = workflow();
        if std::fs::read_to_string(&path)? != want {
            r.change(format!("{rel}: update to neoarchivist {VERSION}"));
            if r.opts.apply {
                write_file(&path, &want)?;
            }
        }
    }
    if !r.opts.checks {
        return Ok(());
    }
    if git(&r.config.root, &["rev-parse", "--is-inside-work-tree"]).is_none() {
        r.say("pre-commit hook:\n  note: not a git repository yet, so no hook; run init again once it is");
    } else {
        let hooks = git(&r.config.root, &["rev-parse", "--git-path", "hooks"])
            .unwrap_or_default()
            .trim()
            .to_string();
        let hook = format!("{hooks}/pre-commit");
        let hook_path = r.path(&hook);
        if !hook_path.is_file() {
            r.change(format!("{hook}: new file, runs neoarchivist check --staged"));
            if r.opts.apply {
                write_file(&hook_path, PRE_COMMIT)?;
                make_executable(&hook_path)?;
            }
        } else if !std::fs::read_to_string(&hook_path)?.contains("neoarchivist check") {
            r.say(format!(
                "{hook}:\n  kept, it is someone else's; add `neoarchivist check --staged` to it"
            ));
        }
    }
    if !path.is_file() {
        let on_github = r.path(".github").is_dir()
            || git(&r.config.root, &["remote", "-v"]).is_some_and(|o| o.contains("github.com"));
        if on_github {
            r.change(format!(
                "{rel}: new file, runs the check on every push and pull request"
            ));
            if r.opts.apply {
                write_file(&path, &workflow())?;
            }
        } else {
            r.say("CI:\n  note: not on GitHub; run `neoarchivist check --range <base>..HEAD` in your CI");
        }
    }
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_: &Path) -> Result<()> {
    Ok(())
}
