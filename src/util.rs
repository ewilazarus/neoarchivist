//! Small helpers shared by the commands.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Today's date, `YYYY-MM-DD`, in local time.
pub fn today() -> String {
    jiff::Zoned::now().date().to_string()
}

/// `1 error`, `2 errors`.
pub fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("{n} {word}")
    } else {
        format!("{n} {word}s")
    }
}

/// Trims, and collapses runs of spaces and tabs into one space.
pub fn squeeze(s: &str) -> String {
    s.split([' ', '\t'])
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Escapes `|` for a Markdown table cell.
pub fn cell(s: &str) -> String {
    s.replace('|', "\\|")
}

/// Whether `name` is `YYYY-MM-DD.md`, and the date if so.
pub fn memo_date(name: &str) -> Option<&str> {
    let date = name.strip_suffix(".md")?;
    let b = date.as_bytes();
    let ok = b.len() == 10
        && b.iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        });
    ok.then_some(date)
}

/// The `date:` in a note's frontmatter, if it has one.
pub fn frontmatter_date(text: &str) -> Option<String> {
    let mut lines = text.lines();
    if lines.next()?.trim_end_matches('\r') != "---" {
        return None;
    }
    let re = regex::Regex::new(r#"^date:[ \t]*"?(\d{4}-\d{2}-\d{2})"?[ \t]*$"#).unwrap();
    for line in lines {
        let line = line.trim_end_matches('\r');
        if line == "---" {
            break;
        }
        if let Some(c) = re.captures(line) {
            return Some(c[1].to_string());
        }
    }
    None
}

/// Runs git, and returns its stdout when it succeeds.
pub fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Resolves symlinks in the deepest existing directory, since the file may not exist yet.
pub fn resolve(path: &Path) -> PathBuf {
    if let Ok(p) = path.canonicalize() {
        return p;
    }
    match (path.parent(), path.file_name()) {
        (Some(dir), Some(name)) => match dir.canonicalize() {
            Ok(dir) => dir.join(name),
            Err(_) => path.to_path_buf(),
        },
        _ => path.to_path_buf(),
    }
}

/// Writes a file through a temporary file, creating its directory.
pub fn write_file(path: &Path, text: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("can't create {}", dir.display()))?;
    }
    let tmp = path.with_extension("neoarchivist-tmp");
    std::fs::write(&tmp, text).with_context(|| format!("can't write {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("can't write {}", path.display()))
}
