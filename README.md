# neoarchivist

A Claude Code plugin for projects that keep an Obsidian vault, by default in `docs/`. One
agent, `neoarchivist:agent`, keeps it, and gives the project three kinds of durable memory:

- **Notes: what is true now.** Every note outside the two folders below, organised however
  the project likes, and rewritten in place when the facts change.
- **Memos: what meaningful things happened.** One curated memo per day,
  `Memos/YYYY-MM-DD.md`.
- **Decisions: why important choices were made.** One short note per choice,
  `Decisions/NNNNN-short-slug.md`, numbered across the vault and dated in its frontmatter.

Git records exactly what changed in the repo. Raw AI activity, meaning the commands run, the
files touched, the tests rerun and the approaches tried, is not project memory, so the vault
doesn't keep it. Recording is deliberately lossy: it keeps outcomes and drops execution, and
for routine work git already explains, it writes nothing at all.

Everything else (folders, templates, bases, tags, working rules) is up to each project, and
lives in the vault's `Conventions.md`.

## Install

With the `claude` CLI, git and jq on your PATH:

```bash
curl -fsSL https://raw.githubusercontent.com/ewilazarus/neoarchivist/main/bootstrap.sh | bash
```

It adds both marketplaces, installs neoarchivist for you in every project, bringing kepano's
`obsidian` plugin along, and puts `neoarchivist` in `~/.local/bin` for the git checks. Run
it again to update. Add `-s -- --project`, from a project's root, to declare the plugins in
that project's `.claude/settings.json` instead, so everyone who opens it is offered them.
To read the script first:

```bash
curl -fsSLO https://raw.githubusercontent.com/ewilazarus/neoarchivist/main/bootstrap.sh
less bootstrap.sh
bash bootstrap.sh --dry-run    # prints the commands without running them
bash bootstrap.sh
```

Then restart Claude Code and run `/neoarchivist:init` in the project.

The plugin runs a small binary, the `neoarchivist` CLI. The first time it's needed, the
plugin downloads the release matching its own version for your platform (macOS or Linux, on
arm64 or x86-64), checks it against its published sha256, and caches it in
`~/.cache/neoarchivist/`.

## Settings

Two files, both optional:

- **`.neoarchivist/config.toml`, at the project root,** defines the shared vault, so it's
  committed and everyone on the project agrees on it:

  ```toml
  vault = "docs"          # where the vault is, relative to the project root
  memos = "Memos"         # the memos folder, inside the vault
  decisions = "Decisions" # the decisions folder, inside the vault
  ```

  `/neoarchivist:init` writes it, asking first if the project already has a vault elsewhere
  or its folders go by other names. After that, it's edited by hand.

- **`$XDG_CONFIG_HOME/neoarchivist/config.toml`** (or `~/.config/neoarchivist/`) holds your
  own preferences:

  ```toml
  stop_reminder = false   # don't remind Claude to record work before it stops
  ```

  Setting `vault`, `memos` or `decisions` there is an error: they define the shared vault,
  and a personal override would have your Claude writing where nobody else looks. Obsidian
  presets live next to it, in `presets/`.

`neoarchivist config` shows the settings in effect, and where they come from.

## The agent

Reading a vault costs context: answering one question can mean reading five notes, a few
memos and a decision, and recording one piece of work means rereading the notes it touches.
So the main Claude never opens the vault. It sends `neoarchivist:agent` a request, and gets
back a short answer:

- **Ask**, in the foreground: `{"kind": "ask", "question": "…"}`. The agent reads what it
  needs and replies with the answer, the `[[links]]` it rests on, and anywhere the notes,
  the memos, the decisions and the system disagree.
- **Record**, in the background, once there's something worth keeping. The request carries
  what git can't show: facts that changed, each decision and why, what was tried and
  dropped, what's left open and how to tell it's done, follow-ups finished, and notes found
  wrong. The agent reads git for the rest, then writes, or explains why nothing needs
  writing.
- **Questions go through Claude.** A subagent can't talk to you, so when the agent needs an
  answer, its reply says `input-required` with its questions. Claude asks you, then resumes
  the same agent with your answers.

Both directions are JSON, checked by `neoarchivist contract`. A hook refuses a request that
doesn't fit, saying exactly what's missing ("`decided[0]` needs `why`"), and sends back a
reply that doesn't fit. The reply states follow the A2A task states: `completed`,
`input-required`, `failed`. The main Claude learns the format from the
`neoarchivist:contract` skill, the only neoarchivist skill it can see, and it loads only
when needed, so a session that never touches the vault carries just a few lines about it,
from the project's `CLAUDE.md`.

The agent's rules, for looking things up and for recording, are in `rules/`, compiled into
the binary, and handed to the agent as it starts, with the project's folder names filled
in. Its instructions and its side of the contract are in `agents/agent.md`.

## Commands

You can also go to the agent directly. These run inside it, so their work stays out of the
main conversation, and their result comes back to you in Markdown:

| Command | What it does |
|---|---|
| `/neoarchivist:init` | Run once per project, and again after updating. It settles where the vault is and what its folders are called, offers both plugins to the project through its settings, creates the memos and decisions folders, writes `Conventions.md` with you, adds a short section to the project's `CLAUDE.md`, can set up the git checks, and records the setup as the first memo. The file changes are shown first as a plan: only what's missing is added, a setting already set differently is kept, and a second run changes nothing. Its questions come to you in one round. |
| `/neoarchivist:ask <question>` | Look something up yourself: what is true from the notes, what happened from the memos, why from the decisions, with links and any disagreements. |
| `/neoarchivist:record` | Runs in the main conversation, since only it knows what happened: Claude turns the conversation into a record request and sends it to the agent in the background. |
| `/neoarchivist:todos` | Every open follow-up in the memos as a table: the day it was raised, its text and its sub-items, with a recommendation of the easiest to pick up next. |
| `/neoarchivist:lint` | Checks the whole vault, including what shell commands, Obsidian and merges wrote past the hooks: broken wikilinks, heading links and block links, bare `§` references, misnamed, future or malformed memos, and decisions with a shared number, no `date:`, no `## Why` or no memo linking them. It separates what can be fixed from history that stays as it is, and fixes nothing until you say so. |
| `/neoarchivist:graph` | Gives each kind of note its own colour in Obsidian's graph view, based on the layout in `Conventions.md`, keeping your other graph settings and any groups you chose unless you say otherwise, and offers to reload Obsidian. |
| `/neoarchivist:preset` | Saves a vault's Obsidian settings (app, appearance, core and community plugins and their settings, hotkeys, graph, CSS snippets, and the sidebar layout without any open notes) as a named preset, and applies one to another vault. It shows the changes first, lets you choose how to settle conflicts, installs missing community plugins fresh, and never copies notes or plugin code. |

The same work is available from the shell as `neoarchivist lint`, `todos`, `next-decision`,
`graph`, `preset`, `obsidian`, `init`, `history` and `check`; `neoarchivist --help` lists
them.

## Memos

A memo is a short summary of the day, not a log:

```markdown
---
description: "Reworked authentication and settled on middleware authorization."
---

## Summary

- Reworked authorization around application middleware and expanded anonymous and
  expired-session coverage. A router-level approach was abandoned because nested routes
  made it unnecessarily complex.

## Decisions

- [[Decisions/00012-authorization-lives-in-middleware|Authorization lives in middleware]]

## Follow-ups

- [ ] Verify authentication behind the production reverse proxy.
```

- **Today's memo converges.** Each bullet is a workstream or an outcome, not an AI session.
  A later record the same day refines the bullets already there instead of appending a new
  section, and keeps what you wrote in it yourself.
- **Past memos are history.** Once the day is over, a memo's prose doesn't change. Anything
  learned later goes in today's.
- **Checkboxes stay live.** A follow-up's words record why work was left open that day,
  and its box records whether it still is. So a past memo's boxes may be ticked, or
  unticked when work reopens, and nothing else about them changes.

A decision note holds one durable choice: what was decided, and why. The bar is high: where
a responsibility lives or which database to use qualifies, but a rename or a routine
refactor doesn't. A decision is rationale, not current state: when the project changes
direction, the notes are rewritten and a new decision supersedes the old one, which stays
as it was.

Unfinished work is an ordinary Markdown checkbox under `## Follow-ups`, in the memo of the
day it came up, right below the Summary that explains it. There is no `#todo` tag, no todo
folder and no task database: `/neoarchivist:todos` derives the list from those boxes. A
checkbox anywhere else is just a checklist. To get close to the same scope in Obsidian
Tasks, limit a query to the memos' Follow-ups:

````markdown
```tasks
not done
path includes Memos/
heading includes Follow-ups
```
````

## Hooks

The plugin's hooks hold the design in place, so none of it depends on Claude remembering:

- **Only the agent touches the vault.** Reads and writes from the main Claude, or any other
  agent, are refused and pointed at the agent. Obsidian's own settings in the vault's
  `.obsidian/` are exempt, because they aren't notes.
- **The agent starts with its rules,** and each request and reply is checked against the
  contract, as above.
- **The agent's own commands run without prompts:** neoarchivist itself, read-only git and
  Obsidian CLI commands, and `date`, each on its own. A plugin agent can't carry permission
  rules, so the hook approves exactly those. Anything combined with `&&`, a pipe, a
  substitution or a redirection still asks you.
- **Editing a past memo** is refused unless the only changes tick or untick checkboxes, at
  any depth and without touching their text, or re-point a broken `[[link]]` while keeping
  its displayed text. Creating a memo for a past day is refused too, and a future day's
  can't be written at all: a memo records what happened, and plans belong in the project's
  notes. "Today" is the machine's local date.
- **Decisions** are created as `NNNNN-slug.md`, taking the next number, with `date:` set to
  today. "Next" counts the decisions on every local and remote-tracking branch too, so two
  branches don't both take the same number. A decision may be refined on the day it was
  made; after that, the same rule as a past memo applies, because a changed mind is a new
  decision.
- **Each note the agent writes is linted** straight after the write. Any errors go back to
  the agent to fix while the note is still in hand. Warnings are left to
  `/neoarchivist:lint`, since a note may be only part-way written.
- **Merges are linted too.** After Claude runs `git merge`, `pull`, `rebase` or
  `cherry-pick`, the notes it changed are linted the same way. That is where a decision
  number taken on two branches, or a link to a note renamed on the other side, first shows
  up.
- **Section references are links.** A write that adds a bare `§4.2` outside a link or code
  is refused, with a hint to write `[[Spec#4.2 Assertion|Spec §4.2]]` instead.
- **A reminder to record, before finishing.** If a session used the agent, then changed
  project files outside the vault, and hasn't sent it a record request since, Claude is
  asked once, as it stops, whether any of it is worth recording. Often the answer is no,
  and it says so in a line. `stop_reminder = false` in your own settings turns it off.

The past-memo check enforces the *shape* of a change, not its meaning: it lets any
checkbox's state change without proving the box sits under `## Follow-ups`. If the binary
is missing or anything goes wrong, the hooks let the action through and report a hook
error, so they never lock you out of your own vault. They see Claude's tools, not the
shell, and not your own edits in Obsidian; the git checks cover those.

## Checking every commit

To hold the rules for everyone's edits, not only Claude's, accept `/neoarchivist:init`'s
`--checks`:

- **A pre-commit hook** runs `neoarchivist check --staged`. The commit fails if a note it
  changes has lint errors, or if it rewrites a past memo or an older decision beyond
  ticking a checkbox or re-pointing a link. Each clone installs its own hook, so each
  teammate runs `/neoarchivist:init` once. `git commit --no-verify` skips it, for a
  deliberate migration. Without `neoarchivist` on the PATH, the hook skips with a warning.
- **On GitHub, a workflow** installs this version's release and runs
  `neoarchivist check --range` on every push and pull request. It judges each commit as of
  the day it was authored, so rebased work isn't mistaken for a rewrite. Elsewhere, run the
  same command in your CI.

Only notes a commit changes are linted, so older problems don't block anyone.

## Development

The toolchain comes from [devbox](https://www.jetify.com/devbox):

```bash
devbox run build   # cargo build --release
devbox run test    # the unit tests, then tests/run.sh
devbox run lint    # cargo fmt --check, and clippy
```

`tests/run.sh` builds the debug binary and runs the bash suite, which specifies every
command's output. On macOS, `/bin/bash tests/run.sh` also checks bash 3.2, the shell the
git hooks and the launcher meet on a stock Mac. CI runs it on macOS and Ubuntu. To try the
plugin from a checkout, build it and run `claude --plugin-dir .`: the launcher uses the
local build instead of downloading a release.

A release is a version tag, `vX.Y.Z`, matching `Cargo.toml` and both manifests in
`.claude-plugin/`; `tests/versions.test.sh` checks they agree. The release workflow builds
and publishes the binaries the launcher downloads.

## License

MIT.

It builds on [kepano/obsidian-skills](https://github.com/kepano/obsidian-skills) for the
Markdown, Bases and CLI know-how.
