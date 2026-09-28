//! `neoarchivist check`: for a pre-commit hook or CI. Lints the notes that changed, failing
//! on errors only, and checks the history rules for every change. Only changed notes are
//! linted, so older problems elsewhere don't block a commit; `neoarchivist lint` shows those.

use crate::config::Config;
use crate::history::{Scope, history, vault_prefix};
use crate::lint::lint;
use crate::util::git;

/// Prints what fails, and returns whether everything passed.
pub fn check(config: &Config, today: &str, scope: &Scope) -> bool {
    let Some(_) = vault_prefix(config) else { return true };
    let vault = config.vault_dir();
    let mut args = vec!["diff", "--relative", "--no-renames", "--name-only", "--diff-filter=AM"];
    let range_args;
    match scope {
        Scope::Staged => args.push("--cached"),
        Scope::Range(range) => {
            range_args = range.split_once("..").map(|(a, b)| (a.to_string(), b.to_string()));
            if let Some((a, b)) = &range_args {
                args.push(a);
                args.push(b);
            }
        }
    }
    args.extend(["--", "."]);
    let changed: Vec<String> = git(&vault, &args)
        .unwrap_or_default()
        .lines()
        .filter(|n| n.ends_with(".md") && !n.starts_with(".obsidian/") && vault.join(n).is_file())
        .map(String::from)
        .collect();

    let mut ok = true;
    if !changed.is_empty() {
        if let Some(report) = lint(config, today, &changed) {
            if report.errors() > 0 {
                println!("neoarchivist: the notes this changes have errors:\n");
                print!("{}", report.error_rows());
                println!();
                ok = false;
            }
        }
    }

    let problems = history(config, today, scope);
    if !problems.is_empty() {
        println!("neoarchivist: this changes the vault's history:\n");
        for p in &problems {
            println!("  {p}");
        }
        println!(
            "\nPast memos and older decisions change only by ticking a checkbox or re-pointing a link. New history goes in today's memo, and a changed mind is a new decision."
        );
        ok = false;
    }
    ok
}
