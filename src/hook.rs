//! `neoarchivist hook`: every Claude Code hook the plugin registers, reading the hook's JSON
//! on stdin. It keeps the vault behind `neoarchivist:agent`, and its history as written:
//!
//! - SubagentStart gives the agent its rules, with this project's folders filled in, and
//!   tells it whether its task is a JSON request or the user's slash command.
//! - PreToolUse on Agent and SendMessage checks every request to the agent against the
//!   contract, and refuses one that doesn't fit, saying what to fix.
//! - SubagentStop checks the agent's reply to a request the same way, and sends it back
//!   once to fix it.
//! - PreToolUse on Bash approves the agent's own commands: this binary, read-only git and
//!   `date`, each alone. A plugin agent can't carry permission rules of its own.
//! - PreToolUse on file tools: the vault's `.obsidian/` is settings, and left alone; anyone
//!   but the agent is refused the vault; a write that adds a bare `§4.2` is refused; today's
//!   memo, and decisions dated today, may be rewritten; a past memo or an older decision
//!   may only change checkbox state or re-point a link; a memo can't be written for a
//!   future day or created after its day; a new decision takes the next number, dated today.
//! - PostToolUse on file writes lints the note just written and hands back its errors, and
//!   notes changes to project files outside the vault, for the stop reminder.
//! - PostToolUse on Bash lints the notes a git merge, pull, rebase or cherry-pick changed.
//! - Stop reminds Claude, once, to send a record request when the session used the agent,
//!   then changed project files outside the vault, and hasn't sent one since. The user's
//!   `stop_reminder = false` turns it off.
//!
//! Anything unexpected fails open: the tool call goes through, and the error is reported
//! as a non-blocking hook error.

use crate::config::Config;
use crate::contract::{self, Side};
use crate::decision::next_number;
use crate::history::{memo_day, same_but_boxes_and_links};
use crate::lint::{bare_sections, lint};
use crate::util::{git, resolve, squeeze, today};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const AGENT: &str = "neoarchivist:agent";
const RECALL: &str = include_str!("../rules/recall.md");
const RECORD: &str = include_str!("../rules/record.md");

struct Hook {
    input: Value,
    event: String,
    session: String,
    agent: Option<String>,
    agent_type: String,
    tool: String,
    cwd: PathBuf,
    config: Config,
    today: String,
    state: PathBuf,
}

/// What the hook tells Claude Code, if anything.
enum Out {
    Quiet,
    Deny(String),
    Allow(String),
    Context(String),
    Block(String),
}

pub fn run(input: &str, today_override: Option<&str>) -> Result<Option<String>> {
    let input: Value = serde_json::from_str(input).context("the hook input isn't JSON")?;
    let s = |k: &str| input.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let cwd = PathBuf::from(s("cwd"));
    let root = std::env::var_os("CLAUDE_PROJECT_DIR")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cwd.as_os_str().is_empty() {
                std::env::current_dir().unwrap_or_default()
            } else {
                cwd.clone()
            }
        });
    let config = Config::load(Some(&root))?;
    let state = std::env::var_os("CLAUDE_PLUGIN_DATA")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("neoarchivist"))
        .join("sessions");
    let hook = Hook {
        event: s("hook_event_name"),
        session: if s("session_id").is_empty() {
            "unknown".into()
        } else {
            s("session_id")
        },
        agent: input.get("agent_id").and_then(Value::as_str).map(String::from),
        agent_type: s("agent_type"),
        tool: s("tool_name"),
        cwd: if cwd.as_os_str().is_empty() { root.clone() } else { cwd },
        today: today_override.map(String::from).unwrap_or_else(today),
        config,
        state,
        input,
    };
    Ok(match hook.dispatch()? {
        Out::Quiet => None,
        Out::Deny(reason) => Some(
            json!({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny", "permissionDecisionReason": reason}})
                .to_string(),
        ),
        Out::Allow(reason) => Some(
            json!({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "allow", "permissionDecisionReason": reason}})
                .to_string(),
        ),
        Out::Context(text) => {
            Some(json!({"hookSpecificOutput": {"hookEventName": hook.event, "additionalContext": text}}).to_string())
        }
        Out::Block(reason) => Some(json!({"decision": "block", "reason": reason}).to_string()),
    })
}

fn key(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "_.-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

impl Hook {
    fn str_at(&self, pointer: &str) -> String {
        self.input
            .pointer(pointer)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }

    fn by_agent(&self) -> bool {
        self.agent.is_some() && self.agent_type == AGENT
    }

    fn marker(&self, name: &str) -> PathBuf {
        self.state.join(format!("{}.{name}", key(&self.session)))
    }

    fn contract_marker(&self, agent: &str) -> PathBuf {
        self.state.join(format!("{}.contract", key(agent)))
    }

    fn touch(&self, path: &Path) {
        let _ = std::fs::create_dir_all(&self.state);
        let _ = std::fs::OpenOptions::new().create(true).append(true).open(path);
        self.prune();
    }

    /// Drops markers from sessions older than a week.
    fn prune(&self) {
        let Ok(entries) = std::fs::read_dir(&self.state) else {
            return;
        };
        let week = std::time::Duration::from_secs(7 * 24 * 3600);
        for e in entries.flatten() {
            if e.metadata()
                .and_then(|m| m.modified())
                .is_ok_and(|t| t.elapsed().is_ok_and(|age| age > week))
            {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }

    fn has_vault(&self) -> bool {
        let v = self.config.vault_dir();
        v.join("Conventions.md").is_file() || v.join(self.config.memos()).is_dir()
    }

    fn to_agent(&self) -> &'static str {
        "Send the neoarchivist:agent agent a request instead: load the neoarchivist:contract skill for the format."
    }

    fn dispatch(&self) -> Result<Out> {
        match self.event.as_str() {
            "SubagentStart" if self.agent_type == AGENT => Ok(self.give_rules()),
            "SubagentStop" => Ok(self.check_reply()),
            "Stop" => Ok(self.stop_reminder()),
            "PostToolUse" => match self.tool.as_str() {
                "Bash" => Ok(self.lint_merged()),
                "Edit" | "Write" | "MultiEdit" => Ok(self.after_write()),
                _ => Ok(Out::Quiet),
            },
            "PreToolUse" => match self.tool.as_str() {
                "Agent" | "SendMessage" => Ok(self.check_request()),
                "Bash" => Ok(self.approve_command()),
                "Read" | "Grep" | "Glob" | "Edit" | "Write" | "MultiEdit" => self.guard_file(),
                _ => Ok(Out::Quiet),
            },
            _ => Ok(Out::Quiet),
        }
    }

    // --- the contract --------------------------------------------------------------------

    fn bin(&self) -> String {
        match std::env::var_os("CLAUDE_PLUGIN_ROOT").filter(|d| !d.is_empty()) {
            Some(root) => PathBuf::from(root).join("bin/neoarchivist").display().to_string(),
            None => std::env::current_exe()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "neoarchivist".into()),
        }
    }

    /// The agent's rules, with this project's folders and the binary's path filled in.
    pub fn rules(&self) -> String {
        let fill = |s: &str| {
            s.replace("{{bin}}", &self.bin())
                .replace("{{vault}}", &self.config.project.vault)
                .replace("{{memos}}", self.config.memos())
                .replace("{{decisions}}", self.config.decisions())
        };
        format!(
            "# The recall rules\n\n{}\n\n# The record rules\n\n{}",
            fill(RECALL),
            fill(RECORD)
        )
    }

    fn give_rules(&self) -> Out {
        self.touch(&self.marker("used"));
        let pending = self.marker("pending");
        let lines = std::fs::read_to_string(&pending).unwrap_or_default();
        let mode = if !lines.trim().is_empty() {
            // One line per request sent: this agent takes one of them.
            let rest: String = lines.lines().skip(1).map(|l| format!("{l}\n")).collect();
            let _ = std::fs::write(&pending, rest);
            if let Some(agent) = &self.agent {
                self.touch(&self.contract_marker(agent));
            }
            "This task came from the main agent, so it is a JSON request, and your final message is a JSON reply."
        } else {
            "This task is the user's slash command, so your final message is for the user to read: plain Markdown, not JSON. If you need the user's answers, end with the numbered questions, and say they can answer in the conversation for Claude to pass on."
        };
        Out::Context(format!("{mode}\n\n{}", self.rules()))
    }

    fn check_request(&self) -> Out {
        let message = if self.tool == "Agent" {
            if self.str_at("/tool_input/subagent_type") != AGENT {
                return Out::Quiet;
            }
            self.str_at("/tool_input/prompt")
        } else {
            let m = self
                .input
                .pointer("/tool_input/message")
                .or_else(|| self.input.pointer("/tool_input/content"));
            let m = match m {
                Some(Value::String(s)) => s.clone(),
                Some(v) => v.to_string(),
                None => return Out::Quiet,
            };
            let t = m.trim_start();
            if !(t.starts_with('{') || t.starts_with("```")) || !m.contains("\"kind\"") {
                return Out::Quiet;
            }
            m
        };
        self.touch(&self.marker("used"));
        let problems = contract::check(Side::Request, &message);
        if !problems.is_empty() {
            return Out::Deny(format!(
                "This request to neoarchivist:agent doesn't fit its contract: {} Load the neoarchivist:contract skill for the format, and send it again.",
                problems.join(" ")
            ));
        }
        if self.tool == "Agent" {
            let pending = self.marker("pending");
            let mut lines = std::fs::read_to_string(&pending).unwrap_or_default();
            lines.push_str("pending\n");
            let _ = std::fs::create_dir_all(&self.state);
            let _ = std::fs::write(&pending, lines);
        }
        // A record request covers the changes made so far, so the stop reminder waits.
        if contract::parse(&message).is_ok_and(|o| o.get("kind") == Some(&json!("record"))) {
            let _ = std::fs::remove_file(self.marker("changed"));
        }
        Out::Quiet
    }

    fn check_reply(&self) -> Out {
        let Some(agent) = &self.agent else { return Out::Quiet };
        if self.agent_type != AGENT || !self.contract_marker(agent).exists() {
            return Out::Quiet;
        }
        if self.input.get("stop_hook_active") == Some(&Value::Bool(true)) {
            return Out::Quiet;
        }
        let problems = contract::check(Side::Reply, &self.str_at("/last_assistant_message"));
        if problems.is_empty() {
            return Out::Quiet;
        }
        Out::Block(format!(
            "Your reply doesn't fit your contract: {} Reply again with one JSON object, as your instructions describe, and nothing else.",
            problems.join(" ")
        ))
    }

    /// Approves the agent's own commands. Anything with a shell operator, a substitution or
    /// a redirection goes through the usual permission check.
    fn approve_command(&self) -> Out {
        if !self.by_agent() {
            return Out::Quiet;
        }
        let command = self.str_at("/tool_input/command");
        let command = command.trim();
        if command.contains(|c| ";&|`$<>(){}\n".contains(c)) {
            return Out::Quiet;
        }
        let command = command.replace(['"', '\''], "");
        let bin = self.bin();
        let rest = command
            .strip_prefix(&format!("{bin} "))
            .or_else(|| command.strip_prefix("neoarchivist "))
            .map(|r| r.split_whitespace().next().unwrap_or(""));
        let approved = match rest {
            Some(sub) => !sub.is_empty() && sub.chars().all(|c| c.is_ascii_lowercase() || c == '-') && sub != "hook",
            None if command.starts_with("git ") => {
                let re = regex::Regex::new(r"^git( -C [^ ]+)? (status|diff|log|show|ls-files|ls-tree|rev-parse)( |$)")
                    .unwrap();
                re.is_match(&command) && !format!(" {command}").contains(" --output")
            }
            None if command.starts_with("obsidian ") => {
                let re =
                    regex::Regex::new(r"^obsidian (vault|vaults|read|search|search:context|backlinks|tasks|file)( |$)")
                        .unwrap();
                re.is_match(&command)
            }
            None => command == "date" || command == "date +%F",
        };
        if approved {
            Out::Allow("A command neoarchivist:agent's rules call for: neoarchivist itself, a read-only git or Obsidian CLI command, or date.".into())
        } else {
            Out::Quiet
        }
    }

    // --- files -------------------------------------------------------------------------

    /// The target path relative to the vault, or None when it's outside the vault.
    fn vault_relative(&self) -> Option<String> {
        let target = self.str_at("/tool_input/file_path");
        let target = if target.is_empty() {
            self.str_at("/tool_input/path")
        } else {
            target
        };
        if target.is_empty() {
            return None;
        }
        let vault = self.config.vault_dir().canonicalize().ok()?;
        let path = Path::new(&target);
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.cwd.join(path)
        };
        let path = resolve(&path);
        if path == vault {
            return Some(".".into());
        }
        path.strip_prefix(&vault)
            .ok()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
    }

    fn guard_file(&self) -> Result<Out> {
        let Some(rel) = self.vault_relative() else {
            return Ok(Out::Quiet);
        };
        if rel == ".obsidian" || rel.starts_with(".obsidian/") {
            return Ok(Out::Quiet);
        }
        if !self.by_agent() {
            return Ok(Out::Deny(format!(
                "The vault is kept by the neoarchivist:agent agent, and nobody else reads or writes it. {}",
                self.to_agent()
            )));
        }
        if matches!(self.tool.as_str(), "Read" | "Grep" | "Glob") {
            return Ok(Out::Quiet);
        }
        let file = self.config.vault_dir().join(&rel);
        let current = if file.is_file() {
            std::fs::read_to_string(&file).unwrap_or_default()
        } else {
            String::new()
        };
        let pairs = self.edit_pairs(&current);
        let today = self.today.as_str();

        if pairs.iter().any(|(old, new)| bare_sections(new) > bare_sections(old)) {
            return Ok(Out::Deny(
                "This change adds a bare section reference such as §4.2. Obsidian can link to the section itself, so write it as a heading link that keeps the number as its text, for example [[Spec#4.2 Assertion|Spec §4.2]], or [[#4.2 Assertion|§4.2]] within the same note, and escape the | as \\| inside a table. Link a section in an outside document with a Markdown link to its URL.".into(),
            ));
        }
        let memos = self.config.memos();
        let only_boxes_and_links = pairs.iter().all(|(old, new)| same_but_boxes_and_links(old, new));

        if let Some(day) = memo_day(&self.config, &rel) {
            if day > today {
                return Ok(Out::Deny(format!(
                    "{rel} is in the future. A memo records what happened, so write in today's memo, {memos}/{today}.md, and put plans in the project's notes."
                )));
            }
            if day < today {
                if !file.is_file() {
                    return Ok(Out::Deny(format!(
                        "{rel} is a past day, and new history goes in today's memo, {memos}/{today}.md."
                    )));
                }
                if !only_boxes_and_links {
                    return Ok(Out::Deny(format!(
                        "{rel} is a past memo. This change touches more than checkboxes and links. A past memo's prose is history: the only edits it gets are ticking or unticking a box (without touching its text) or re-pointing a broken [[link]] while keeping its displayed text. Write anything new in today's memo, {memos}/{today}.md."
                    )));
                }
            }
            return Ok(Out::Quiet);
        }

        let decisions = self.config.decisions();
        if let Some(name) = rel
            .strip_prefix(&format!("{decisions}/"))
            .filter(|n| !n.contains('/') && n.ends_with(".md"))
        {
            if !file.is_file() {
                let next = next_number(&self.config);
                let re = regex::Regex::new(r"^([0-9]{5})-[a-z0-9][a-z0-9-]*\.md$").unwrap();
                let Some(number) = re.captures(name).map(|c| c[1].to_string()) else {
                    return Ok(Out::Deny(format!(
                        "New decisions are named {decisions}/NNNNN-short-slug.md: the next number, {next}, then a short lowercase slug. `neoarchivist next-decision <title>` prints the whole path."
                    )));
                };
                if number.parse::<u32>().unwrap_or(0) < next.parse::<u32>().unwrap_or(1) {
                    return Ok(Out::Deny(format!(
                        "Decision numbers are never reused or filled in, including numbers taken on other branches. The next one is {next}."
                    )));
                }
                let content = self.str_at("/tool_input/content");
                if !content.lines().any(|l| l == format!("date: {today}")) {
                    return Ok(Out::Deny(format!(
                        "A new decision carries `date: {today}` in its frontmatter: decisions are recorded on the day they're made."
                    )));
                }
                return Ok(Out::Quiet);
            }
            if current.lines().any(|l| l == format!("date: {today}")) {
                return Ok(Out::Quiet);
            }
            if !only_boxes_and_links {
                return Ok(Out::Deny(format!(
                    "{rel} is a recorded decision. This change touches more than checkboxes and links. Decisions keep the rationale as it was when the choice was made. If the project changed direction, create a new decision that links to this one, and update the notes that describe the current state."
                )));
            }
        }
        Ok(Out::Quiet)
    }

    /// The (before, after) text of each change the tool makes.
    fn edit_pairs(&self, current: &str) -> Vec<(String, String)> {
        match self.tool.as_str() {
            "Write" => vec![(current.to_string(), self.str_at("/tool_input/content"))],
            "MultiEdit" => self
                .input
                .pointer("/tool_input/edits")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|e| {
                    let s = |k: &str| e.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                    (s("old_string"), s("new_string"))
                })
                .collect(),
            _ => vec![(
                self.str_at("/tool_input/old_string"),
                self.str_at("/tool_input/new_string"),
            )],
        }
    }

    fn after_write(&self) -> Out {
        match self.vault_relative() {
            Some(rel) if rel.ends_with(".md") && !rel.starts_with(".obsidian/") => {
                let Some(report) = lint(&self.config, &self.today, std::slice::from_ref(&rel)) else {
                    return Out::Quiet;
                };
                if report.errors() == 0 {
                    return Out::Quiet;
                }
                Out::Context(format!(
                    "The lint found errors in {rel} after this write. Fix them now, in the same way as the rest of this change:\n\n{}",
                    report.error_rows().trim_end()
                ))
            }
            Some(_) => Out::Quiet,
            None => {
                self.note_change();
                Out::Quiet
            }
        }
    }

    /// Notes that Claude changed a project file outside the vault, for the stop reminder.
    fn note_change(&self) {
        if !self.has_vault() || self.by_agent() {
            return;
        }
        let target = self.str_at("/tool_input/file_path");
        if target.is_empty() {
            return;
        }
        let path = resolve(&self.cwd.join(&target));
        let roots = [
            self.config
                .root
                .canonicalize()
                .unwrap_or_else(|_| self.config.root.clone()),
            self.config.root.clone(),
        ];
        for root in &roots {
            if path.starts_with(root.join(".git")) {
                return;
            }
            if path.starts_with(root) {
                self.touch(&self.marker("changed"));
                return;
            }
        }
    }

    /// After a git merge, pull, rebase or cherry-pick: lints the notes it changed.
    fn lint_merged(&self) -> Out {
        let command = self.str_at("/tool_input/command");
        let re = regex::Regex::new(r"(^|[;&|(\s])git(\s+-C\s+\S+)?\s+(merge|pull|rebase|cherry-pick)(\s|$)").unwrap();
        if !re.is_match(&command) {
            return Out::Quiet;
        }
        let vault = self.config.vault_dir();
        if !vault.is_dir() {
            return Out::Quiet;
        }
        let (Some(before), Some(after)) = (
            git(&vault, &["rev-parse", "-q", "--verify", "ORIG_HEAD"]),
            git(&vault, &["rev-parse", "-q", "--verify", "HEAD"]),
        ) else {
            return Out::Quiet;
        };
        let (before, after) = (before.trim().to_string(), after.trim().to_string());
        if before == after {
            return Out::Quiet;
        }
        let changed: Vec<String> = git(
            &vault,
            &[
                "diff",
                "--relative",
                "--name-only",
                "--diff-filter=AMR",
                &before,
                &after,
                "--",
                ".",
            ],
        )
        .unwrap_or_default()
        .lines()
        .filter(|f| f.ends_with(".md") && !f.starts_with(".obsidian/"))
        .map(String::from)
        .collect();
        if changed.is_empty() {
            return Out::Quiet;
        }
        let Some(report) = lint(&self.config, &self.today, &changed) else {
            return Out::Quiet;
        };
        if report.errors() == 0 {
            return Out::Quiet;
        }
        Out::Context(format!(
            "That git command changed notes in the vault, and the lint found errors in them:\n\n{}\n\nTell the user. A decision number shared by two notes means two branches each took the next one: offer to have neoarchivist:agent renumber the newer decision and update the links to it, but ask first, since other branches may link to it. The agent can also fix broken links by piping them to the note's new name.",
            report.error_rows().trim_end()
        ))
    }

    // --- the stop reminder -----------------------------------------------------------------

    fn stop_reminder(&self) -> Out {
        if !self.config.user.stop_reminder || self.input.get("stop_hook_active") == Some(&Value::Bool(true)) {
            return Out::Quiet;
        }
        let changed = self.marker("changed");
        if !changed.exists() || !self.has_vault() || !self.marker("used").exists() {
            return Out::Quiet;
        }
        let _ = std::fs::remove_file(changed);
        Out::Block(squeeze(
            "Before finishing: this session used the vault and then changed project files, and hasn't sent neoarchivist:agent a record request since. Send one, in the background, if the work changed something outside the repo, made a choice whose reason will matter later, left work unfinished, finished or reopened a follow-up, or showed a note to be wrong; the neoarchivist:contract skill has the format. If none of that is true, routine code work that git explains needs no record: say in one line that there is nothing to record, and stop.",
        ))
    }
}
