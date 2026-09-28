//! neoarchivist: keeps a project's docs/ Obsidian vault behind one agent, and gives that
//! agent, the plugin's hooks and git the tools they use on it.

mod check;
mod config;
mod contract;
mod decision;
mod graph;
mod history;
mod hook;
mod init;
mod lint;
mod obsidian;
mod preset;
mod todos;
mod util;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "neoarchivist", version, about = "Keeps a project's docs/ Obsidian vault.")]
struct Cli {
    /// The project root. Defaults to $CLAUDE_PROJECT_DIR, the enclosing git repository, or
    /// the current directory.
    #[arg(long, global = true)]
    project: Option<PathBuf>,
    /// Check as if today were this date, YYYY-MM-DD. For tests.
    #[arg(long, global = true, hide = true)]
    today: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check the whole vault, and print its problems as a table.
    Lint {
        /// Report only on these notes, relative to the vault. The whole vault is still read.
        #[arg(long)]
        only: Vec<String>,
        /// Exit 1 when there are errors. Warnings never fail it.
        #[arg(long)]
        strict: bool,
    },
    /// List the open follow-ups in the memos.
    Todos,
    /// Check that git changes keep the vault's history as written.
    History(ScopeArgs),
    /// Lint the notes that changed and check the history rules: for git hooks and CI.
    Check(ScopeArgs),
    /// Print the path for a new decision from its title, or just the next number.
    NextDecision { title: Vec<String> },
    /// Check a message to or from neoarchivist:agent against their contract.
    Contract {
        #[command(subcommand)]
        action: ContractAction,
    },
    /// Show or set the colour groups of the vault's graph view.
    Graph {
        #[command(subcommand)]
        action: GraphAction,
    },
    /// Save a vault's Obsidian settings as a named preset, or apply one.
    Preset {
        #[command(subcommand)]
        action: PresetAction,
    },
    /// Check that the Obsidian CLI targets this vault, or reload Obsidian safely.
    Obsidian {
        #[command(subcommand)]
        action: ObsidianAction,
    },
    /// Set up a project: settings, folders, CLAUDE.md, .gitignore and the git checks.
    Init {
        #[command(subcommand)]
        action: InitAction,
    },
    /// Handle a Claude Code hook event, read as JSON on stdin.
    #[command(hide = true)]
    Hook,
    /// Print the settings in effect, and where they come from.
    Config,
}

#[derive(Subcommand)]
enum GraphAction {
    /// Print the colour groups, with their colours as hex.
    Show,
    /// Write colour groups, given in order as '<query>=#RRGGBB'.
    Set {
        /// Swap the groups already there for these.
        #[arg(long, conflicts_with = "add")]
        replace: bool,
        /// Keep the groups already there, and add these.
        #[arg(long)]
        add: bool,
        #[arg(required = true)]
        groups: Vec<String>,
    },
}

#[derive(Subcommand)]
enum PresetAction {
    /// List the saved presets.
    List,
    /// Save the vault's settings as a preset.
    Save { name: String },
    /// Show what applying a preset would change, and change nothing.
    Plan { name: String },
    /// Apply a preset.
    Apply {
        name: String,
        /// Whose value wins where the vault already sets something differently.
        #[arg(long, value_enum, default_value = "vault")]
        prefer: PreferArg,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum PreferArg {
    Vault,
    Preset,
}

#[derive(Subcommand)]
enum ObsidianAction {
    /// Exit 0 if `obsidian` commands reach this vault.
    Check,
    /// Reload Obsidian, so it rereads the vault's settings, if it targets this vault.
    Reload,
}

#[derive(Subcommand)]
enum InitAction {
    /// Show the changes, and make none.
    Plan(InitArgs),
    /// Make the changes.
    Apply(InitArgs),
}

#[derive(clap::Args)]
struct InitArgs {
    /// Where the vault is, relative to the project root, for a project without settings yet.
    #[arg(long)]
    vault: Option<String>,
    /// What the memos folder is called, inside the vault.
    #[arg(long)]
    memos: Option<String>,
    /// What the decisions folder is called, inside the vault.
    #[arg(long)]
    decisions: Option<String>,
    /// With only AGENTS.md: create CLAUDE.md importing it, or append the section to it.
    #[arg(long, value_enum)]
    agents_md: Option<AgentsMdArg>,
    /// Also gitignore community plugins.
    #[arg(long)]
    ignore_plugins: bool,
    /// Install the pre-commit hook and, on GitHub, the CI workflow.
    #[arg(long)]
    checks: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum AgentsMdArg {
    Import,
    Append,
}

#[derive(clap::Args)]
#[group(required = true, multiple = false)]
struct ScopeArgs {
    /// The changes staged for the next commit.
    #[arg(long)]
    staged: bool,
    /// Each commit in a range, such as main..HEAD.
    #[arg(long, value_name = "A..B")]
    range: Option<String>,
}

impl ScopeArgs {
    fn scope(&self) -> history::Scope {
        match &self.range {
            Some(r) => history::Scope::Range(r.clone()),
            None => history::Scope::Staged,
        }
    }
}

#[derive(Subcommand)]
enum ContractAction {
    /// Read a message on stdin, print `ok` or its problems, and exit 1 on problems.
    Check { side: SideArg },
}

#[derive(Clone, Copy, ValueEnum)]
enum SideArg {
    Request,
    Reply,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("neoarchivist: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    if let Command::Hook = cli.command {
        // A hook fails open: any error goes to stderr with exit 1, which Claude Code shows
        // as a non-blocking hook error and lets the action through.
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input)?;
        return Ok(match hook::run(&input, cli.today.as_deref()) {
            Ok(out) => {
                if let Some(out) = out {
                    println!("{out}");
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("neoarchivist hook failed, allowing the action: {e:#}");
                ExitCode::FAILURE
            }
        });
    }
    let config = config::Config::load(cli.project.as_deref())?;
    let today = cli.today.clone().unwrap_or_else(util::today);
    let ok = |b: bool| if b { ExitCode::SUCCESS } else { ExitCode::FAILURE };
    Ok(match cli.command {
        Command::Lint { only, strict } => match lint::lint(&config, &today, &only) {
            None => {
                println!("No vault found at `{}`.", config.vault_dir().display());
                ExitCode::SUCCESS
            }
            Some(report) => {
                print!("{}", report.render());
                ok(!strict || report.errors() == 0)
            }
        },
        Command::Todos => {
            print!("{}", todos::todos(&config));
            ExitCode::SUCCESS
        }
        Command::History(scope) => {
            let problems = history::history(&config, &today, &scope.scope());
            for p in &problems {
                println!("{p}");
            }
            ok(problems.is_empty())
        }
        Command::Check(scope) => ok(check::check(&config, &today, &scope.scope())),
        Command::NextDecision { title } => {
            if !config.vault_dir().is_dir() {
                anyhow::bail!("No vault found at {}.", config.vault_dir().display());
            }
            if title.is_empty() {
                println!("{}", decision::next_number(&config));
            } else {
                println!("{}", decision::next_path(&config, &title.join(" "))?);
            }
            ExitCode::SUCCESS
        }
        Command::Contract {
            action: ContractAction::Check { side },
        } => {
            let mut message = String::new();
            std::io::stdin().read_to_string(&mut message)?;
            let side = match side {
                SideArg::Request => contract::Side::Request,
                SideArg::Reply => contract::Side::Reply,
            };
            let problems = contract::check(side, &message);
            if problems.is_empty() {
                println!("ok");
            }
            for p in &problems {
                println!("{p}");
            }
            ok(problems.is_empty())
        }
        Command::Graph { action } => {
            match action {
                GraphAction::Show => print!("{}", graph::show(&config)?),
                GraphAction::Set { replace, add, groups } => {
                    let how = if replace {
                        graph::How::Replace
                    } else if add {
                        graph::How::Add
                    } else {
                        graph::How::Refuse
                    };
                    print!("{}", graph::set(&config, how, &groups)?)
                }
            }
            ExitCode::SUCCESS
        }
        Command::Preset { action } => {
            let out = match action {
                PresetAction::List => preset::list(&config),
                PresetAction::Save { name } => preset::save(&config, &name)?,
                PresetAction::Plan { name } => preset::plan_or_apply(&config, &name, false, preset::Prefer::Vault)?,
                PresetAction::Apply { name, prefer } => {
                    let prefer = match prefer {
                        PreferArg::Vault => preset::Prefer::Vault,
                        PreferArg::Preset => preset::Prefer::Preset,
                    };
                    preset::plan_or_apply(&config, &name, true, prefer)?
                }
            };
            print!("{out}");
            ExitCode::SUCCESS
        }
        Command::Obsidian { action } => ok(match action {
            ObsidianAction::Check => obsidian::check(&config),
            ObsidianAction::Reload => obsidian::reload(&config),
        }),
        Command::Init { action } => {
            let (apply, args) = match action {
                InitAction::Plan(a) => (false, a),
                InitAction::Apply(a) => (true, a),
            };
            let mut config = config;
            config.set_project(args.vault, args.memos, args.decisions)?;
            let opts = init::Options {
                apply,
                agents_md: args.agents_md.map(|a| match a {
                    AgentsMdArg::Import => init::AgentsMd::Import,
                    AgentsMdArg::Append => init::AgentsMd::Append,
                }),
                ignore_plugins: args.ignore_plugins,
                checks: args.checks,
            };
            print!("{}", init::run(&config, &opts)?);
            ExitCode::SUCCESS
        }
        Command::Hook => unreachable!("handled before the settings load"),
        Command::Config => {
            println!("project:   {}", config.root.display());
            println!(
                "           {} {}",
                config.project_file().display(),
                if config.project_file().is_file() {
                    ""
                } else {
                    "(not there: defaults)"
                }
            );
            println!("vault:     {}", config.project.vault);
            println!("memos:     {}", config.project.memos);
            println!("decisions: {}", config.project.decisions);
            let user_file = config.user_dir.join("config.toml");
            println!(
                "user:      {} {}",
                user_file.display(),
                if user_file.is_file() {
                    ""
                } else {
                    "(not there: defaults)"
                }
            );
            println!("stop_reminder: {}", config.user.stop_reminder);
            println!("presets:   {}", config.presets_dir().display());
            ExitCode::SUCCESS
        }
    })
}
