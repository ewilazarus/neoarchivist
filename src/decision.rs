//! `neoarchivist next-decision`: the number, or the whole path, for a new decision.
//!
//! The number is one more than the highest in the decisions folder, on this checkout and on
//! every local and remote-tracking branch, so a decision made on one branch doesn't take a
//! number another branch already has. Numbers are never reused, even when a decision is
//! deleted. The slug is the title in lowercase ASCII, with anything else turned into hyphens.

use crate::config::Config;
use crate::util::git;
use anyhow::{Result, bail};

/// The next decision number, as five digits.
pub fn next_number(config: &Config) -> String {
    let dir = config.vault_dir().join(config.decisions());
    let mut highest: u32 = 0;
    let mut consider = |name: &str| {
        let name = name.rsplit('/').next().unwrap_or(name);
        if name.len() > 6 && name.ends_with(".md") && name.as_bytes()[5] == b'-' {
            if let Ok(n) = name[..5].parse::<u32>() {
                if name[..5].bytes().all(|b| b.is_ascii_digit()) {
                    highest = highest.max(n);
                }
            }
        }
    };
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            consider(&e.file_name().to_string_lossy());
        }
    }
    if let Some(prefix) = git(&config.vault_dir(), &["rev-parse", "--show-prefix"]) {
        let path = format!("{}{}/", prefix.trim(), config.decisions());
        let refs = git(
            &config.vault_dir(),
            &[
                "for-each-ref",
                "--format=%(refname) %(symref)",
                "refs/heads",
                "refs/remotes",
            ],
        )
        .unwrap_or_default();
        for line in refs.lines() {
            let mut parts = line.splitn(2, ' ');
            let refname = parts.next().unwrap_or("");
            if !parts.next().unwrap_or("").is_empty() {
                continue;
            }
            let files = git(
                &config.vault_dir(),
                &["ls-tree", "--name-only", "--full-tree", refname, "--", &path],
            )
            .unwrap_or_default();
            for f in files.lines() {
                consider(f);
            }
        }
    }
    format!("{:05}", highest + 1)
}

/// The slug of a title: lowercase ASCII letters and digits, joined by hyphens.
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in title.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if gap && !out.is_empty() {
                out.push('-');
            }
            out.push(c);
            gap = false;
        } else {
            gap = true;
        }
    }
    out
}

/// The path for a new decision, relative to the vault.
pub fn next_path(config: &Config, title: &str) -> Result<String> {
    let slug = slug(title);
    if slug.is_empty() {
        bail!("The title needs some letters or digits for its slug.");
    }
    Ok(format!("{}/{}-{slug}.md", config.decisions(), next_number(config)))
}
