//! `neoarchivist history`: checks that git changes keep the vault's history as written.
//!
//! - a memo can't be written for a future day, or created, renamed or deleted after its day;
//! - after its day, a memo changes only by ticking or unticking a checkbox, without
//!   touching its text, and by re-pointing a `[[link]]` while keeping its displayed text;
//! - a new decision is dated the day it is made, in its `date:` frontmatter;
//! - after its day, a decision changes only in the same two ways, and a rename that keeps
//!   its content passes.
//!
//! "Today" is the commit's author date, which a rebase keeps, so rebased work isn't taken
//! for a rewrite of the past. With `--staged` it is today.

use crate::config::Config;
use crate::util::{frontmatter_date, git, memo_date};
use regex::Regex;
use std::sync::LazyLock;

static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[[^\]|]+\|([^\]]+)\]\]").unwrap());
static BOX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([-*+]) \[[ xX]\]").unwrap());

/// Whether `old` and `new` differ only in checkbox state and link targets: `[[target|shown]]`
/// becomes `[[shown]]`, and every box becomes unticked, so only other changes remain.
pub fn same_but_boxes_and_links(old: &str, new: &str) -> bool {
    let norm = |s: &str| BOX.replace_all(&LINK.replace_all(s, "[[$1]]"), "$1 [ ]").into_owned();
    norm(old) == norm(new)
}

pub enum Scope {
    Staged,
    Range(String),
}

/// The problems, one line each.
pub fn history(config: &Config, today: &str, scope: &Scope) -> Vec<String> {
    let root = config.root.as_path();
    let Some(prefix) = vault_prefix(config) else {
        return Vec::new();
    };
    let empty_tree = git(root, &["hash-object", "-t", "tree", "/dev/null"])
        .unwrap_or_default()
        .trim()
        .to_string();
    let mut problems = Vec::new();
    match scope {
        Scope::Staged => {
            let base = if git(root, &["rev-parse", "-q", "--verify", "HEAD"]).is_some() {
                "HEAD".to_string()
            } else {
                empty_tree
            };
            check_diff(config, &prefix, &base, None, today, "", &mut problems);
        }
        Scope::Range(range) => {
            let commits = git(root, &["rev-list", "--reverse", "--no-merges", range]).unwrap_or_default();
            for commit in commits.lines() {
                let short = git(root, &["rev-parse", "--short", commit]).unwrap_or_default();
                let parent = format!("{commit}^");
                let parent = if git(root, &["rev-parse", "-q", "--verify", &parent]).is_some() {
                    parent
                } else {
                    empty_tree.clone()
                };
                let day = git(root, &["log", "-1", "--format=%ad", "--date=short", commit]).unwrap_or_default();
                let place = format!("{} ", short.trim());
                check_diff(
                    config,
                    &prefix,
                    &parent,
                    Some(commit),
                    day.trim(),
                    &place,
                    &mut problems,
                );
            }
        }
    }
    problems
}

/// The vault's path from the top of the git repository, with a trailing slash.
pub fn vault_prefix(config: &Config) -> Option<String> {
    let vault = config.vault_dir();
    if !vault.is_dir() {
        return None;
    }
    git(&vault, &["rev-parse", "--show-prefix"]).map(|p| p.trim().to_string())
}

fn check_diff(
    config: &Config,
    prefix: &str,
    old: &str,
    new: Option<&str>,
    today: &str,
    place: &str,
    problems: &mut Vec<String>,
) {
    let root = config.root.as_path();
    let top = git(root, &["rev-parse", "--show-toplevel"]).unwrap_or_default();
    let top = std::path::Path::new(top.trim());
    let mut args = vec!["diff", "--find-renames", "--name-status"];
    if new.is_none() {
        args.push("--cached");
    }
    args.push(old);
    if let Some(new) = new {
        args.push(new);
    }
    args.extend(["--", prefix]);
    let diff = git(top, &args).unwrap_or_default();
    let show = |rev: &str, path: &str| git(top, &["show", &format!("{rev}:{path}")]).unwrap_or_default();
    let new_rev = new.unwrap_or("");
    for line in diff.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        let (status, from, to) = match parts.as_slice() {
            [s, p] => (s.chars().next().unwrap_or(' '), *p, *p),
            [s, a, b] => (s.chars().next().unwrap_or(' '), *a, *b),
            _ => continue,
        };
        if !matches!(status, 'A' | 'D' | 'M' | 'R') {
            continue;
        }
        let old_text = if status == 'A' { String::new() } else { show(old, from) };
        let new_text = if status == 'D' {
            String::new()
        } else {
            show(new_rev, to)
        };
        let rel_from = from.strip_prefix(prefix).unwrap_or(from);
        let rel = to.strip_prefix(prefix).unwrap_or(to);
        let mut report = |path: &str, message: String| problems.push(format!("{place}{path}: {message}"));

        let mut status = status;
        if status == 'R' {
            if let Some(day) = memo_day(config, rel_from) {
                if day < today {
                    report(
                        rel_from,
                        format!("renames a past memo to {rel}. Memos are a record of what was known then."),
                    );
                    continue;
                }
            }
            status = 'M';
        }

        if let Some(day) = memo_day(config, rel) {
            if day > today {
                if status != 'D' {
                    report(
                        rel,
                        "writes a future day. A memo records what happened; plans belong in notes.".into(),
                    );
                }
            } else if day < today {
                match status {
                    'A' => report(
                        rel,
                        format!("creates a past day's memo on {today}. New history goes in that day's own memo."),
                    ),
                    'D' => report(
                        rel,
                        "deletes a past memo. Memos are a record of what was known then.".into(),
                    ),
                    _ => {
                        if !same_but_boxes_and_links(&old_text, &new_text) {
                            report(rel, "changes a past memo's prose. After its day, only checkbox state and link targets may change.".into());
                        }
                    }
                }
            }
            continue;
        }

        if is_decision(config, rel) {
            match status {
                'A' => {
                    let date = frontmatter_date(&new_text);
                    if date.as_deref() != Some(today) {
                        report(
                            rel,
                            format!(
                                "is a new decision dated {}, not {today}. A decision is dated the day it's made.",
                                date.as_deref().unwrap_or("nowhere")
                            ),
                        );
                    }
                }
                'M' => {
                    if let Some(date) = frontmatter_date(&old_text) {
                        if date.as_str() < today && !same_but_boxes_and_links(&old_text, &new_text) {
                            report(
                                rel,
                                format!(
                                    "changes a decision made on {date}. A changed mind is a new decision that supersedes this one."
                                ),
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// The day of a memo, from its path relative to the vault.
pub fn memo_day<'a>(config: &Config, rel: &'a str) -> Option<&'a str> {
    let name = rel.strip_prefix(config.memos())?.strip_prefix('/')?;
    if name.contains('/') {
        return None;
    }
    memo_date(name)
}

/// Whether a path relative to the vault is a decision note.
pub fn is_decision(config: &Config, rel: &str) -> bool {
    rel.strip_prefix(config.decisions())
        .and_then(|r| r.strip_prefix('/'))
        .is_some_and(|name| !name.contains('/') && name.ends_with(".md"))
}
