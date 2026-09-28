//! `neoarchivist todos`: the open follow-ups in the memos, as a Markdown table.
//!
//! Every unticked top-level `- [ ]` box in a memo's `## Follow-ups` section is one row,
//! oldest memo first: the day it was raised, its text (continuation lines joined), and its
//! nested items with their own state. The section runs to the next `#` or `##` heading.
//! Boxes anywhere else are ordinary checklists, not project work: they aren't listed, and
//! the summary line only counts the unticked ones. Frontmatter and fenced code are skipped.

use crate::config::Config;
use crate::util::{cell, memo_date, squeeze};
use regex::Regex;
use std::sync::LazyLock;

static TOP_BOX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[-*+][ \t]+\[([ xX])\][ \t]*").unwrap());
static NESTED_BOX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[ \t]+[-*+][ \t]+\[([ xX])\][ \t]*").unwrap());
static NESTED_ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[ \t]+([-*+]|[0-9]+[.)])[ \t]+").unwrap());
static FENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[ \t]*(```|~~~)").unwrap());

struct Row {
    date: String,
    text: String,
    subs: Vec<String>,
}

#[derive(PartialEq)]
enum State {
    None,
    Skip,
    Open,
    Done,
}

struct Item {
    state: State,
    text: String,
    subs: Vec<String>,
}

impl Item {
    fn new() -> Self {
        Item {
            state: State::None,
            text: String::new(),
            subs: Vec::new(),
        }
    }
}

#[derive(Default)]
struct Tally {
    rows: Vec<Row>,
    closed: usize,
    elsewhere: usize,
}

impl Tally {
    fn flush(&mut self, item: &mut Item, date: &str) {
        match item.state {
            State::Open => self.rows.push(Row {
                date: date.to_string(),
                text: squeeze(&item.text),
                subs: std::mem::take(&mut item.subs),
            }),
            State::Done => self.closed += 1,
            _ => {}
        }
        *item = Item::new();
    }
}

pub fn todos(config: &Config) -> String {
    let dir = config.vault_dir().join(config.memos());
    if !dir.is_dir() {
        return format!("No memos found at `{}`.\n", dir.display());
    }
    let mut files: Vec<(String, std::path::PathBuf)> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().is_file())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    memo_date(&name).map(|d| (d.to_string(), e.path()))
                })
                .collect()
        })
        .unwrap_or_default();
    if files.is_empty() {
        return format!("`{}/` has no memos yet.\n", config.memos());
    }
    files.sort();

    let mut tally = Tally::default();
    for (date, path) in &files {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        scan(&text, date, &mut tally);
    }
    render(&tally)
}

fn scan(text: &str, date: &str, tally: &mut Tally) {
    let mut item = Item::new();
    let mut in_followups = false;
    let mut fence = false;
    let mut frontmatter = false;
    for (i, raw) in text.lines().enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if i == 0 && line == "---" {
            frontmatter = true;
            continue;
        }
        if frontmatter {
            if line == "---" {
                frontmatter = false;
            }
            continue;
        }
        if FENCE.is_match(line) {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        let level = line.bytes().take_while(|b| *b == b'#').count();
        if level > 0 && line[level..].starts_with([' ', '\t']) {
            tally.flush(&mut item, date);
            if level <= 2 {
                in_followups = level == 2 && squeeze(&line[level..]) == "Follow-ups";
            }
            continue;
        }
        if let Some(c) = TOP_BOX.captures(line) {
            tally.flush(&mut item, date);
            let open = &c[1] == " ";
            if !in_followups {
                item.state = State::Skip;
                if open {
                    tally.elsewhere += 1;
                }
                continue;
            }
            item.state = if open { State::Open } else { State::Done };
            item.text = line[c.get(0).unwrap().end()..].to_string();
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        if !line.starts_with([' ', '\t']) {
            tally.flush(&mut item, date);
            continue;
        }
        if item.state != State::Open && item.state != State::Done {
            continue;
        }
        if let Some(c) = NESTED_BOX.captures(line) {
            let mark = if &c[1] == " " { "☐ " } else { "☑ " };
            item.subs
                .push(format!("{mark}{}", squeeze(&line[c.get(0).unwrap().end()..])));
        } else if let Some(c) = NESTED_ITEM.captures(line) {
            item.subs
                .push(format!("• {}", squeeze(&line[c.get(0).unwrap().end()..])));
        } else if let Some(last) = item.subs.last_mut() {
            *last = format!("{last} {}", squeeze(line));
        } else {
            item.text = format!("{} {}", item.text, squeeze(line));
        }
    }
    tally.flush(&mut item, date);
}

fn render(t: &Tally) -> String {
    let elsewhere = match t.elsewhere {
        0 => String::new(),
        1 => " 1 unticked box elsewhere in the memos is outside a Follow-ups section, so it is not listed.".into(),
        n => format!(
            " {n} unticked boxes elsewhere in the memos are outside a Follow-ups section, so they are not listed."
        ),
    };
    if t.rows.is_empty() {
        return format!("No open follow-ups. {} already ticked.{elsewhere}\n", t.closed);
    }
    let mut out = String::from("| # | Raised | Todo | Sub-items |\n|---|---|---|---|\n");
    for (i, row) in t.rows.iter().enumerate() {
        let subs = if row.subs.is_empty() {
            "—".to_string()
        } else {
            cell(&row.subs.join(" · "))
        };
        out += &format!("| {} | {} | {} | {} |\n", i + 1, row.date, cell(&row.text), subs);
    }
    let mut days: Vec<&str> = t.rows.iter().map(|r| r.date.as_str()).collect();
    days.dedup();
    let n = t.rows.len();
    out += &format!(
        "\n{n} open {}, raised on {} {}. {} already ticked.{elsewhere}\n",
        if n == 1 { "follow-up" } else { "follow-ups" },
        days.len(),
        if days.len() == 1 { "day" } else { "days" },
        t.closed
    );
    out
}
