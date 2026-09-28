---
name: lint
description: Check this project's docs vault as a whole, and list its problems as a table, including broken wikilinks and heading links, bare § references, malformed or future memos, and decisions with a shared number, no date, no Why or no memo linking them. Use when the user runs /neoarchivist:lint or asks to check, lint, validate or health-check the docs vault, or to find broken links in it.
disable-model-invocation: true
context: fork
agent: neoarchivist:agent
background: false
allowed-tools: Bash(${CLAUDE_PLUGIN_ROOT}/bin/neoarchivist *), Read, Grep, Glob
---

# Lint the docs vault

You're running this as neoarchivist:agent, for the user's `/neoarchivist:lint` command. Your
final message is for the user to read, in Markdown, not JSON. Where these steps say to ask
the user, end your reply with the numbered questions, and say they can answer in the
conversation for Claude to pass on; carry on when their answers arrive.

The hooks check each write as Claude makes it. Shell commands, edits in Obsidian and
merges between branches get past them, and a rename can break links in notes nobody
touched. This checks the whole vault as it stands. A script has already run the checks, so
the list doesn't depend on judgement. Here is its output:

!`${CLAUDE_PLUGIN_ROOT}/bin/neoarchivist lint`

## 1. Show the list

**Print the script's output above exactly as it is:** the table and the summary line under
it. Don't reorder, merge, reword or drop rows, and don't add problems of your own. If it
says there is no vault, or no problems, say so and stop.

An **error** is something that is wrong: a link that goes nowhere, a decision number used
twice, a memo dated in the future. A **warning** breaks a neoarchivist convention, and
the project may have a reason for it. `Conventions.md` wins where it says otherwise.

## 2. Say what can be fixed

After the table, sort the rows into two groups, citing their `#`:

- **Fixable now.** Anything in a note, in today's memo, or in a decision dated
  today. A broken link in a past memo or an older decision is fixable too, but only by
  re-pointing it with a pipe that keeps its displayed text, `[[New target|Old text]]`.
  For each, say what the fix would be, reading the file if you need to. Find where a
  renamed note went with `fd` or `rg`.
- **History, left as it is.** Any other problem in a past memo or a decision dated
  before today, such as a missing Summary or a missing `date:`. Those files are a record of
  what was known then, and the hooks refuse to change their prose. Say so, and don't
  propose a way around it.

Two errors need the user to choose:

- **A shared decision number** usually comes from two branches that each took the next
  one. The later decision needs a new number, from `neoarchivist next-decision`,
  and the memo links to it need updating. Ask before renaming it, because other
  people's branches may link to it.
- **A link to a note that doesn't exist yet** may be deliberate. Some vaults leave
  unresolved links as placeholders. Ask whether to create the note, remove the link or
  keep it.

## 3. Stop

Don't fix anything yet. End your reply by asking which fixes to make, with the
questions above among them. Make the ones the user approves under the record rules, once
their answers arrive. Fixing lint findings is housekeeping, not something that happened
in the project, so it isn't written into a memo unless the user asks.
