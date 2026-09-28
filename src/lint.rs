//! `neoarchivist lint`: checks the vault as a whole, and prints its problems as a table.
//!
//! - every `[[wikilink]]` and `![[embed]]` resolves to a file, and its `#heading` or
//!   `#^block` exists in that file;
//! - no bare section reference such as `§4.2` sits outside a link or code;
//! - a memo is named `YYYY-MM-DD.md` and isn't dated in the future, has a `description:`,
//!   a `## Summary`, and `## Summary`, `## Decisions` and `## Follow-ups` once each, in that
//!   order, none of them empty;
//! - a decision is named `NNNNN-short-slug.md`, its number isn't shared, it has a `date:`
//!   and a `## Why`, and some memo links it.
//!
//! Links resolve the way Obsidian resolves them, ignoring case: a vault-relative path, then
//! a path relative to the linking note, then any file whose path ends with the target.
//! Frontmatter is read, but fenced code and inline code are skipped.

use crate::config::Config;
use crate::util::{cell, memo_date, plural, squeeze};
use regex::Regex;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    fn as_str(&self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Problem {
    pub file: String,
    pub line: usize,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct Report {
    pub problems: Vec<Problem>,
    pub notes: usize,
    pub memos: usize,
    pub decisions: usize,
    /// The notes named with --only, if any.
    pub only: Option<Vec<String>>,
}

impl Report {
    pub fn errors(&self) -> usize {
        self.problems.iter().filter(|p| p.severity == Severity::Error).count()
    }

    pub fn warnings(&self) -> usize {
        self.problems.len() - self.errors()
    }

    /// The problems as a Markdown table, followed by a summary line.
    pub fn render(&self) -> String {
        let checked = match &self.only {
            Some(only) => format!(
                "Reported on {} given with --only, not the rest of the vault.",
                if only.len() == 1 {
                    "the 1 note".to_string()
                } else {
                    format!("the {} notes", only.len())
                }
            ),
            None => format!(
                "Checked {}: {} and {}.",
                plural(self.notes, "note"),
                plural(self.memos, "memo"),
                plural(self.decisions, "decision")
            ),
        };
        if self.problems.is_empty() {
            return format!("No problems found. {checked}\n");
        }
        let mut out = String::from("| # | Severity | Where | Problem |\n|---|---|---|---|\n");
        let mut files = HashSet::new();
        for (i, p) in self.problems.iter().enumerate() {
            let place = if p.line > 0 {
                format!("{}:{}", p.file, p.line)
            } else {
                p.file.clone()
            };
            out += &format!(
                "| {} | {} | `{}` | {} |\n",
                i + 1,
                p.severity.as_str(),
                place,
                cell(&p.message)
            );
            files.insert(&p.file);
        }
        out += &format!(
            "\n{} and {} in {}. {checked}\n",
            plural(self.errors(), "error"),
            plural(self.warnings(), "warning"),
            plural(files.len(), "file")
        );
        out
    }

    /// Only the error rows, with the table's header, for hooks and git checks.
    pub fn error_rows(&self) -> String {
        self.render()
            .lines()
            .filter(|l| l.starts_with("| # ") || l.starts_with("|---") || l.contains(" | error | "))
            .map(|l| format!("{l}\n"))
            .collect()
    }
}

/// Lints the vault. `only` limits the report to those notes, given relative to the vault
/// or starting with the vault's folder; the whole vault is still read, since links and
/// decision numbers depend on it.
pub fn lint(config: &Config, today: &str, only: &[String]) -> Option<Report> {
    let vault = config.vault_dir();
    if !vault.is_dir() {
        return None;
    }
    let only: Option<Vec<String>> = (!only.is_empty()).then(|| {
        let mut set: Vec<String> = only.iter().map(|p| relative_to_vault(config, p)).collect();
        set.sort();
        set.dedup();
        set
    });
    let mut linter = Linter::new(config, today);
    linter.index(&vault);
    linter.run(&vault);
    let mut report = linter.into_report();
    if let Some(only) = &only {
        report.problems.retain(|p| only.contains(&p.file));
    }
    report.only = only;
    Some(report)
}

/// The `--only` path relative to the vault.
pub fn relative_to_vault(config: &Config, path: &str) -> String {
    let vault_abs = config.vault_dir();
    let vault_abs = format!("{}/", vault_abs.display());
    let vault_rel = format!("{}/", config.project.vault.trim_end_matches('/'));
    let mut p = path;
    p = p.strip_prefix(vault_abs.as_str()).unwrap_or(p);
    p = p.strip_prefix("./").unwrap_or(p);
    p = p.strip_prefix(vault_rel.as_str()).unwrap_or(p);
    p.to_string()
}

struct Link {
    file: String,
    line: usize,
    shown: String,
    target: String,
    sub: String,
}

#[derive(Default)]
struct NoteFacts {
    has_description: bool,
    has_date: bool,
    has_why: bool,
    has_summary: bool,
}

struct Linter<'a> {
    config: &'a Config,
    today: &'a str,
    files: Vec<String>,
    lower: Vec<String>,
    headings: HashSet<(String, String)>,
    blocks: HashSet<(String, String)>,
    links: Vec<Link>,
    facts: HashMap<String, NoteFacts>,
    linked_from_memos: HashSet<String>,
    problems: Vec<Problem>,
    notes: usize,
    memos: usize,
    decisions: usize,
}

static FENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[ \t]*(```|~~~)").unwrap());
static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#+[ \t]").unwrap());
static BLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\^([A-Za-z0-9-]+)[ \t]*$").unwrap());
static INLINE_CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`[^`]*`").unwrap());
static MD_LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[[^\]]*\]\([^)]*\)").unwrap());
static BARE_SECTION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"§ ?[0-9]").unwrap());
static DATE_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^date:[ \t]*"?[0-9]{4}-[0-9]{2}-[0-9]{2}"?[ \t]*$"#).unwrap());
static WHY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^##[ \t]+Why[ \t]*$").unwrap());
static DECISION_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9]{5}-[a-z0-9][a-z0-9-]*\.md$").unwrap());

/// The heading text as a link spells it: Obsidian drops the characters a link can't hold.
pub fn heading_key(heading: &str) -> String {
    let h: String = heading
        .to_lowercase()
        .chars()
        .filter(|c| *c != '`')
        .map(|c| if ":#^%[]|\\".contains(c) { ' ' } else { c })
        .collect();
    squeeze(&h)
}

/// The wikilinks in `s`, as (inner text) pairs, and `s` with them removed.
pub fn take_links(s: &str) -> (Vec<String>, String) {
    let mut links = Vec::new();
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("[[") {
        let after = &rest[i + 2..];
        let Some(j) = after.find("]]") else { break };
        links.push(after[..j].to_string());
        out.push_str(&rest[..i]);
        rest = &after[j + 2..];
    }
    out.push_str(rest);
    (links, out)
}

/// How many bare section references `text` holds, outside code and links.
pub fn bare_sections(text: &str) -> usize {
    static FENCED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)```.*?```").unwrap());
    static INLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`[^`\n]*`").unwrap());
    static WIKI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[[^\]]*\]\]").unwrap());
    let t = FENCED.replace_all(text, "");
    let t = INLINE.replace_all(&t, "");
    let t = WIKI.replace_all(&t, "");
    let t = MD_LINK.replace_all(&t, "");
    BARE_SECTION.find_iter(&t).count()
}

fn dirname(p: &str) -> &str {
    p.rfind('/').map(|i| &p[..i]).unwrap_or("")
}

fn basename(p: &str) -> &str {
    p.rfind('/').map(|i| &p[i + 1..]).unwrap_or(p)
}

impl<'a> Linter<'a> {
    fn new(config: &'a Config, today: &'a str) -> Self {
        Linter {
            config,
            today,
            files: Vec::new(),
            lower: Vec::new(),
            headings: HashSet::new(),
            blocks: HashSet::new(),
            links: Vec::new(),
            facts: HashMap::new(),
            linked_from_memos: HashSet::new(),
            problems: Vec::new(),
            notes: 0,
            memos: 0,
            decisions: 0,
        }
    }

    fn index(&mut self, vault: &Path) {
        let mut files = Vec::new();
        walk(vault, vault, &mut files);
        files.sort();
        self.lower = files.iter().map(|f| f.to_lowercase()).collect();
        self.files = files;
    }

    fn kind(&self, file: &str) -> Kind {
        let dir = dirname(file);
        if dir == self.config.memos() {
            Kind::Memo
        } else if dir == self.config.decisions() {
            Kind::Decision
        } else {
            Kind::Note
        }
    }

    fn report(&mut self, file: &str, line: usize, severity: Severity, message: impl Into<String>) {
        self.problems.push(Problem {
            file: file.to_string(),
            line,
            severity,
            message: message.into(),
        });
    }

    fn run(&mut self, vault: &Path) {
        let files = self.files.clone();
        for file in files.iter().filter(|f| f.ends_with(".md")) {
            self.notes += 1;
            let text = std::fs::read_to_string(vault.join(file)).unwrap_or_default();
            self.scan(file, &text);
        }
        self.check_links();
        for file in files.iter().filter(|f| f.ends_with(".md")) {
            match self.kind(file) {
                Kind::Memo => self.check_memo(file),
                Kind::Decision => self.check_decision(file),
                Kind::Note => {}
            }
        }
    }

    fn add_link(&mut self, file: &str, line: usize, inner: &str) {
        let mut inner = inner.split('|').next().unwrap_or("").to_string();
        if inner.ends_with('\\') {
            inner.pop();
        }
        let shown = inner.clone();
        let (target, sub) = match inner.find('#') {
            Some(i) => (inner[..i].to_string(), inner[i + 1..].to_string()),
            None => (inner.clone(), String::new()),
        };
        self.links.push(Link {
            file: file.to_string(),
            line,
            shown,
            target: squeeze(&target),
            sub,
        });
    }

    fn scan(&mut self, file: &str, text: &str) {
        let kind = self.kind(file);
        let mut facts = NoteFacts::default();
        let mut sections = Sections::default();
        let mut in_frontmatter = false;
        let mut fence = false;
        for (i, raw) in text.lines().enumerate() {
            let n = i + 1;
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            if n == 1 && line == "---" {
                in_frontmatter = true;
                continue;
            }
            if in_frontmatter {
                if line == "---" {
                    in_frontmatter = false;
                    continue;
                }
                if line.starts_with("description:") {
                    facts.has_description = true;
                }
                if DATE_LINE.is_match(line) {
                    facts.has_date = true;
                }
                for inner in take_links(line).0 {
                    self.add_link(file, n, &inner);
                }
                continue;
            }
            if FENCE.is_match(line) {
                fence = !fence;
                sections.content();
                continue;
            }
            if fence {
                continue;
            }
            if HEADING.is_match(line) {
                let h = line.trim_start_matches('#').trim_start_matches([' ', '\t']);
                let h = h
                    .trim_end_matches([' ', '\t'])
                    .trim_end_matches('#')
                    .trim_end_matches([' ', '\t']);
                self.headings.insert((file.to_string(), heading_key(h)));
            }
            if let Some(c) = BLOCK.captures(line) {
                self.blocks.insert((file.to_string(), c[1].to_string()));
            }
            let s = INLINE_CODE.replace_all(line, "");
            let (links, s) = take_links(&s);
            for inner in links {
                self.add_link(file, n, &inner);
            }
            let s = MD_LINK.replace_all(&s, "");
            if BARE_SECTION.is_match(&s) {
                self.report(
                    file,
                    n,
                    Severity::Warning,
                    "Bare section reference. Link the section itself, for example `[[Spec#4.2 Assertion|Spec §4.2]]`.",
                );
            }
            if kind == Kind::Decision && WHY.is_match(line) {
                facts.has_why = true;
            }
            if kind != Kind::Memo {
                continue;
            }
            let level = line.bytes().take_while(|b| *b == b'#').count();
            if (level == 1 || level == 2) && line[level..].starts_with([' ', '\t']) {
                self.close_section(file, &mut sections);
                let name = squeeze(&line[level..]);
                if level == 2 && ["Summary", "Decisions", "Follow-ups"].contains(&name.as_str()) {
                    self.open_section(file, n, &name, &mut sections);
                }
            } else if !line.trim().is_empty() {
                sections.content();
            }
        }
        if kind == Kind::Memo {
            self.close_section(file, &mut sections);
            facts.has_summary = sections.seen.contains_key("Summary");
        }
        self.facts.insert(file.to_string(), facts);
    }

    fn close_section(&mut self, file: &str, s: &mut Sections) {
        if let Some((name, line)) = s.current.take() {
            if !s.has_content {
                self.report(
                    file,
                    line,
                    Severity::Warning,
                    format!("`## {name}` is empty. Leave a section out when it has nothing in it."),
                );
            }
        }
    }

    fn open_section(&mut self, file: &str, n: usize, name: &str, s: &mut Sections) {
        let rank = match name {
            "Summary" => 1,
            "Decisions" => 2,
            _ => 3,
        };
        let seen = s.seen.entry(name.to_string()).or_insert(0);
        *seen += 1;
        if *seen > 1 {
            self.report(
                file,
                n,
                Severity::Warning,
                format!("`## {name}` appears more than once. A memo has one of each section."),
            );
        } else if rank < s.last_rank {
            self.report(
                file,
                n,
                Severity::Warning,
                format!(
                    "`## {name}` comes after `## {}`. The order is Summary, Decisions, Follow-ups.",
                    s.last_name
                ),
            );
        }
        if rank > s.last_rank {
            s.last_rank = rank;
            s.last_name = name.to_string();
        }
        s.current = Some((name.to_string(), n));
        s.has_content = false;
    }

    fn find_file(&self, from: &str, target: &str) -> Option<String> {
        if target.is_empty() {
            return Some(from.to_string());
        }
        let t = target.to_lowercase();
        let t_md = format!("{t}.md");
        let exact = |l: &String, a: &str, b: &str| l == a || l == b;
        if let Some(k) = self.lower.iter().position(|l| exact(l, &t, &t_md)) {
            return Some(self.files[k].clone());
        }
        let dir = dirname(from).to_lowercase();
        if !dir.is_empty() {
            let rel = format!("{dir}/{t}");
            let rel_md = format!("{rel}.md");
            if let Some(k) = self.lower.iter().position(|l| exact(l, &rel, &rel_md)) {
                return Some(self.files[k].clone());
            }
        }
        let tail = format!("/{t}");
        let tail_md = format!("/{t}.md");
        self.lower
            .iter()
            .position(|l| l.ends_with(&tail) || l.ends_with(&tail_md))
            .map(|k| self.files[k].clone())
    }

    fn check_links(&mut self) {
        let links = std::mem::take(&mut self.links);
        for link in &links {
            let Some(target) = self.find_file(&link.file, &link.target) else {
                self.report(
                    &link.file,
                    link.line,
                    Severity::Error,
                    format!("Broken link `[[{}]]`: no file named `{}`.", link.shown, link.target),
                );
                continue;
            };
            if self.kind(&link.file) == Kind::Memo && self.kind(&target) == Kind::Decision {
                self.linked_from_memos.insert(target.clone());
            }
            if link.sub.is_empty() {
                continue;
            }
            let last = link.sub.rsplit('#').next().unwrap_or("");
            if let Some(block) = last.strip_prefix('^') {
                if !self.blocks.contains(&(target.clone(), block.to_string())) {
                    self.report(
                        &link.file,
                        link.line,
                        Severity::Error,
                        format!("Broken link `[[{}]]`: no block `{last}` in `{target}`.", link.shown),
                    );
                }
            } else if !self.headings.contains(&(target.clone(), heading_key(last))) {
                self.report(
                    &link.file,
                    link.line,
                    Severity::Error,
                    format!("Broken link `[[{}]]`: no heading `{last}` in `{target}`.", link.shown),
                );
            }
        }
    }

    fn check_memo(&mut self, file: &str) {
        self.memos += 1;
        let memos = self.config.memos().to_string();
        let Some(date) = memo_date(basename(file)) else {
            self.report(
                file,
                0,
                Severity::Warning,
                format!("Not a memo. `{memos}/` holds only `{memos}/YYYY-MM-DD.md`."),
            );
            return;
        };
        if date > self.today {
            self.report(
                file,
                0,
                Severity::Error,
                "Dated in the future. A memo records what happened.",
            );
        }
        let facts = self.facts.get(file);
        let (has_summary, has_description) = (
            facts.is_some_and(|f| f.has_summary),
            facts.is_some_and(|f| f.has_description),
        );
        if !has_summary {
            self.report(file, 0, Severity::Error, "No `## Summary`. Every memo has one.");
        }
        if !has_description {
            self.report(
                file,
                0,
                Severity::Warning,
                "No `description:` in the frontmatter. It is the memo's headline.",
            );
        }
    }

    fn check_decision(&mut self, file: &str) {
        self.decisions += 1;
        let name = basename(file);
        if !DECISION_NAME.is_match(name) {
            self.report(file, 0, Severity::Warning, "Not named `NNNNN-short-slug.md`.");
        } else {
            let number = &name[..5];
            let prefix = format!("{}/{number}-", self.config.decisions());
            let others: Vec<String> = self
                .files
                .iter()
                .filter(|f| {
                    f.starts_with(&prefix) && f.ends_with(".md") && !f[prefix.len()..].contains('/') && *f != file
                })
                .cloned()
                .collect();
            for other in others {
                self.report(
                    file,
                    0,
                    Severity::Error,
                    format!("Decision number {number} is also used by `{other}`. Numbers are never shared."),
                );
            }
        }
        let facts = self.facts.get(file);
        let (has_date, has_why) = (facts.is_some_and(|f| f.has_date), facts.is_some_and(|f| f.has_why));
        if !has_date {
            self.report(file, 0, Severity::Error, "No `date: YYYY-MM-DD` in the frontmatter.");
        }
        if !has_why {
            self.report(
                file,
                0,
                Severity::Warning,
                "No `## Why`. A decision says why it was made.",
            );
        }
        if !self.linked_from_memos.contains(file) {
            self.report(
                file,
                0,
                Severity::Warning,
                "No memo links this decision. The memo of the day it was made lists it under `## Decisions`.",
            );
        }
    }

    fn into_report(mut self) -> Report {
        self.problems.sort_by(|a, b| {
            (a.file.as_str(), a.line, a.severity.as_str(), a.message.as_str()).cmp(&(
                b.file.as_str(),
                b.line,
                b.severity.as_str(),
                b.message.as_str(),
            ))
        });
        Report {
            problems: self.problems,
            notes: self.notes,
            memos: self.memos,
            decisions: self.decisions,
            only: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Memo,
    Decision,
    Note,
}

#[derive(Default)]
struct Sections {
    current: Option<(String, usize)>,
    has_content: bool,
    seen: BTreeMap<String, usize>,
    last_rank: usize,
    last_name: String,
}

impl Sections {
    fn content(&mut self) {
        if self.current.is_some() {
            self.has_content = true;
        }
    }
}

/// Every file under `dir`, relative to `root`, leaving out Obsidian's settings and trash.
fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if dir == root && [".obsidian", ".trash", ".git"].contains(&name.as_ref()) {
            continue;
        }
        if name == ".DS_Store" {
            continue;
        }
        if path.is_dir() {
            walk(root, &path, out);
        } else if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}
