# Recall from the docs vault

`{{vault}}/` is an Obsidian vault. Every path below is relative to it. These are your rules for
an `ask`: **only read**. Writing is for a `record` request, under the record rules.

kepano's `obsidian-markdown`, `obsidian-bases` and `obsidian-cli` skills own the syntax and
the tooling. Load them through the Skill tool when you need them, instead of guessing.

## First, read the project's conventions

Read `Conventions.md` at the vault root if it exists. neoarchivist fixes only `{{memos}}/` and
`{{decisions}}/`. Everything else is up to the project, and `Conventions.md` is where it
defines how the vault is laid out, what its notes carry and which rules bind work here. It
takes precedence over your defaults. Where it says nothing, look at how the vault is actually
organised before assuming anything.

## Which part of the vault answers what

- **Notes**, meaning every file outside `{{memos}}/` and `{{decisions}}/`, however the project
  organises them, hold **what is true now**.
- **`{{memos}}/YYYY-MM-DD.md`** holds **what meaningful things happened** each day, and the
  work left open.
- **`{{decisions}}/NNNNN-slug.md`** holds **why an important choice was made**, one
  choice per note.
- **Git** holds exactly what changed in the repo. What the agent ran or tried is kept
  nowhere, on purpose.

| Question | Where to look |
|---|---|
| What is true now? How is X configured? How does X work? | Notes |
| What happened today, or on a given day? | That day's memo |
| What happened with X over time? | The memos, plus the decisions about X |
| Why did we choose X? What have we decided about X? | `{{decisions}}/` first, then the memos for the chronology |
| What remains open? | The memos' Follow-ups, via `/neoarchivist:todos` |
| Why does this follow-up exist? | The Summary above it, in the day that raised it |

**Current-state questions go to the notes.** The memos and decisions are history, and
never the authority on the present: a decision says what was chosen then, not what is
true now. If you have to read history to find out whether something is *currently* true,
you have found a gap. A note should hold that fact, and filling the gap is a job for
`record`.

**A disagreement is a finding.** If a note says X and a newer memo or decision says
Y, don't silently pick one. Report that the note may be stale, with both sources. The same
goes for the note and the system it describes: read the note before acting on the thing,
and when they disagree, don't assume the system is broken. The disagreement is the
finding, and the note gets rewritten.

## Reading the memos

A memo has a `## Summary` of the day's outcomes, and optionally `## Decisions`, which
links that day's decision notes, and `## Follow-ups`, which holds the open work as plain
checkboxes. Its `description:` is the day's headline, so a sweep of descriptions is a quick
index of the project's history. Today's file is still converging, and later calls to
`record` may refine it. Past days don't change, except for their boxes.

**The top-level unticked boxes under `## Follow-ups` across `{{memos}}/` are the project's
TODO list.** A box is the one part of a past day that *is* current, because it is ticked
when the work is done. Nested boxes are subtasks. A box anywhere else, in a memo or in
a note, is an ordinary checklist and not project work. For the whole list as a table, with
a suggestion of what to do next, use `/neoarchivist:todos`.

## Reading decisions

Each note in `{{decisions}}/` holds one choice, with a `## Why`. It is written on the day of
the choice and not changed afterwards. When the project changes direction, a newer
decision supersedes the old one and links it from `## Related`, so read the newest decision
on a subject, and follow its links back for the history.

**Don't undo a recorded decision unless asked.** If something is disabled, pinned or
deliberately missing and a decision explains why, someone made that call on purpose.

## Querying

**Search with the Grep and Glob tools.** They need no permission prompt, and cover what
the vault needs:

- Grep `^description:` in `{{vault}}/{{memos}}/`: one headline per day, a quick index of the
  project's history. In `{{vault}}/{{decisions}}/`: one line per decision.
- Grep a subject in `{{vault}}/{{decisions}}/`, with `-i`, for the decisions about it; in
  `{{vault}}/` for everything.
- Grep `^## Follow-ups` in `{{vault}}/{{memos}}/`, with lines after it, for the follow-ups and
  their day. `"{{bin}}" todos` lists the open ones exactly.
- Glob `**/Connection pool.md` in `{{vault}}/` to find where a `[[link]]` lives.

**The Obsidian CLI adds search with context and backlinks, but check which vault it's
pointed at first.** Projects' vaults tend to share a folder name, so the CLI talks to
whichever vault was focused last. Before its first use in a task, run
`"{{bin}}" obsidian check`. It compares the vault the CLI reports with this project's
`{{vault}}/`, symlinks resolved. If it fails, keep to Grep and Glob.

```bash
obsidian search:context query="path:{{decisions}} connection pool"  # decisions about a subject
obsidian search:context query="connection pool"                    # full text, with matches
obsidian read file="2026-09-25"                                    # one day's memo
obsidian backlinks file="Connection pool"                          # what refers to a note
```

Note names often contain spaces, so quote them.

## Gaps are findings

When answering shows that a note is missing, wrong or out of date, say so in the reply's
`disagreements`. Don't fix it during an `ask`: the main agent decides whether to send a
`record` request.
