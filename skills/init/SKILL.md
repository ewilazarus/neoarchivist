---
name: init
description: Set up a project's Obsidian vault, kept by the neoarchivist:agent agent. It settles where the vault lives and what its memos and decisions folders are called, writes the project's Conventions.md with the user, adds a neoarchivist section to the project's CLAUDE.md, offers the plugins to everyone on the project through its settings, and can install git checks. Run once per project, and again after updating neoarchivist.
disable-model-invocation: true
context: fork
agent: neoarchivist:agent
background: false
allowed-tools: Bash(${CLAUDE_PLUGIN_ROOT}/bin/neoarchivist *)
---

# Set up the vault

You're running this as neoarchivist:agent, for the user's `/neoarchivist:init` command. Your
final message is for the user to read, in Markdown, not JSON. Where these steps say to ask
the user, end your reply with the numbered questions, and say they can answer in the
conversation for Claude to pass on; carry on when their answers arrive. **Ask everything
in one round:** the choices from step 2 and the questions from step 4 together, after the
plan's output, so the user answers once.

This sets up a project so you can keep its vault from then on. neoarchivist owns two
folders in it: the memos, one per day of what meaningful things happened and what was left
open, and the decisions, one note per important choice and its reason. Everything else is
defined by the `Conventions.md` you write here, together with the user.

**Never overwrite or move an existing file.** Where something already exists, adopt it,
report it, and move on. The one exception is the marked neoarchivist section in `CLAUDE.md`,
which this command owns and may replace. Do the steps in order.

neoarchivist makes every mechanical change: the settings file, the project's Claude
settings, the folders, the Daily notes setting, the `CLAUDE.md` section, `.gitignore` and
the git checks. It only adds what is missing, keeps any value already set differently and
says so, and changes nothing on a second run. Don't make those edits by hand.

```bash
${CLAUDE_PLUGIN_ROOT}/bin/neoarchivist init plan  [--vault DIR] [--memos DIR] [--decisions DIR] [--agents-md import|append] [--ignore-plugins] [--checks]
${CLAUDE_PLUGIN_ROOT}/bin/neoarchivist init apply [the same]
```

Run it from the project root.

## 1. Survey what exists

```bash
${CLAUDE_PLUGIN_ROOT}/bin/neoarchivist config
${CLAUDE_PLUGIN_ROOT}/bin/neoarchivist init plan
git rev-parse --is-inside-work-tree
```

Then Glob the project for an existing vault, meaning a folder holding a `.obsidian/` folder,
and its folders two levels deep, and read its `Conventions.md` if it has one. If the vault
already holds notes, the conventions describe *those notes as they are*. Don't propose
restructuring them here. That is a separate job, and only if the user asks.

## 2. Show the plan and settle its choices

Show the user the plan's output, and explain each part in plain words:

- **`.neoarchivist/config.toml`** holds where the vault is and what its memos and decisions
  folders are called. Everyone on the project shares it, so it's committed. The defaults
  are `docs`, `Memos` and `Decisions`.
- **`.claude/settings.json`** offers both plugins to everyone who opens the project.
  neoarchivist depends on kepano's `obsidian` plugin, and Claude Code only installs a
  dependency from a marketplace already added, so both marketplaces are declared. Plugins
  load at session start, so others get them in their next session. A `kept` line is a
  plugin someone set on purpose, such as one disabled for this project. Leave it that way
  unless the user says otherwise.
- **The memos and decisions folders.** Don't create a folder for todos or tasks. Open work
  is a plain checkbox under a `## Follow-ups` heading in the memo of the day it came up,
  and `/neoarchivist:todos` lists it from there.
- **`daily-notes.json`** appears only when the Daily notes core plugin is on. It points new
  daily notes at the memos folder. If the plan says it `kept` another folder, ask whether
  to change it, and if so edit that one key yourself.
- **The `CLAUDE.md` section.** Hooks only fire when Claude touches the vault, and skills
  only load when a request matches them. This section covers what neither catches: work
  that changes something the vault documents without ever opening it. It says when to ask
  you and when to have you record, and nothing else. **Don't add rules or conventions to
  it.** They live in your rules and in `Conventions.md`, and a copy here would go stale
  while loading into every session.
- **`.gitignore`** keeps each user's own Obsidian settings out of git: their open panes,
  the graph view's settings, their editor preferences such as the default view mode, and
  their theme and fonts. Obsidian rewrites these whenever someone clicks around, zooms the
  graph or changes a preference, so they'd churn on every commit, and they aren't the
  project's to set. Graph colours stay per-user as a result, and `/neoarchivist:graph` sets
  them for whoever runs it. Say what the lines hold rather than just naming "Obsidian
  files": a user who doesn't know what's in `.obsidian/` can't judge them.

These choices change the plan. Ask them together:

- **Where the vault is, and what its folders are called,** when the project has no
  `.neoarchivist/config.toml` yet. If the survey found a vault somewhere other than
  `docs/`, or memos or decisions under other names, propose those. Pass the answers as
  `--vault`, `--memos` and `--decisions`. Once the file exists, it is edited by hand.
- **`--checks`, the vault's rules in git.** The hooks only guard Claude's writes. Edits in
  Obsidian, teammates' edits and shell commands skip them. This adds a pre-commit hook
  for this clone that runs `neoarchivist check --staged`, and on GitHub a workflow for
  every push and pull request. A commit then fails if a note it changes has lint errors,
  or if it rewrites a past memo or an older decision. `git commit --no-verify` skips the
  check once, for a deliberate migration. The hook needs `neoarchivist` on the PATH, which
  the install script puts there; without it, the hook skips with a warning. Each teammate
  gets the hook by running `/neoarchivist:init` in their own clone, and the workflow covers
  everyone. Recommend it for a vault shared by several people.
- **`CLAUDE.md: needs a choice`** means the project has `AGENTS.md` and no `CLAUDE.md`.
  Creating a `CLAUDE.md` makes Claude Code stop reading `AGENTS.md` on its own, so either
  create one that imports it (`--agents-md import`) or add the section to `AGENTS.md`
  (`--agents-md append`). Recommend `import`, because the section names Claude Code
  agents and skills that other agents reading `AGENTS.md` can't use.
- **`note: the vault has community plugins`**: their code and the list of enabled plugins
  are usually personal installs, so offer `--ignore-plugins`. Leave it off if the project
  shares its plugins, and say so in `Conventions.md`.

## 3. Apply

Run `init apply` with the flags chosen in step 2, and show its output.

**Obsidian reads `.obsidian/*.json` only at startup, and writes its in-memory settings
back when they change.** If Daily notes was changed and Obsidian is running, the change
won't show until it reloads, and it can be lost if the user changes a setting in the
meantime. Offer to reload it. With the user's OK, run
`${CLAUDE_PLUGIN_ROOT}/bin/neoarchivist obsidian reload`. It reloads only after checking
that the CLI targets this vault, and says what to do if it didn't. A vault Obsidian hasn't
opened yet has nothing to reload: step 6 tells the user to open it.

## 4. Write `Conventions.md` with the user

Start from `${CLAUDE_PLUGIN_ROOT}/assets/Conventions.md`, and fill it in from what the
survey found and what the user tells you. Create it at the vault's root with the Write
tool, which refuses to replace a file you haven't read. Ask about the things you can't
infer, together, not one by one:

- **Layout:** which folders exist or should exist, and what each one holds. The memos and
  decisions folders belong to neoarchivist, so don't define them again. Don't add another
  home for decisions or open work either, such as an ADR folder, a todo list or question
  notes. You only look in the decisions folder and in the memos' Follow-ups.
- **Notes:** how notes are named, which frontmatter they carry, and whether there are
  templates. If there are, record the folder they live in. If the user has no preference
  on frontmatter, offer a single `description:` line. The memos, the decisions and
  `Conventions.md` already carry one, so grepping `^description:` indexes the whole vault.
- **Tags and callouts:** any the project uses, and where they go. neoarchivist itself uses
  none, because its headings and folders carry the meaning.
- **Rules:** anything that binds work on this project. Examples are what needs
  confirming first, what must never be done, and how to reach the systems the vault
  documents.

Leave a section out if the user has no answer for it yet. An empty heading is not a
convention. Keep each rule an instruction, followed by its reason. **Don't restate the
memo, decision or follow-up rules here.** A copy is one more place that goes stale.

If `Conventions.md` already exists, don't rewrite it. Offer only the additions the survey
turned up.

## 5. Record the setup

Write today's memo under the record rules. Keep it short: a `## Summary` with a bullet or
two on what was set up, not one per answer the user gave. Write a decision note only for a
choice from step 4 whose reason will matter later, such as a layout rule the project
commits to, and link it under `## Decisions`. Put anything left for later, such as
conventions the user wants to settle once the project grows, under `## Follow-ups` as
plain checkboxes.

## 6. Hand over

Tell the user to open the vault's folder in Obsidian (Open folder as vault), and that from
now on they, and Claude, reach it through neoarchivist:agent: Claude sends it lookups and
records work, and they can run `/neoarchivist:ask` and `/neoarchivist:record` themselves.
If the layout has more than one folder, mention that `/neoarchivist:graph` colour-codes the
graph view by folder. If `neoarchivist preset list` shows saved presets, offer
`/neoarchivist:preset` to apply one, so the new vault gets the user's usual Obsidian
settings. Summarise what was created, what was already there, and what was skipped.
