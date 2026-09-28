---
name: todos
description: List every open follow-up in this project's docs vault's memos as a table, with the day it was raised, its text and its sub-items, and recommend the easiest one to do next. Use when the user runs /neoarchivist:todos or asks what's open, what's left, what to pick up next, or for the TODO list or backlog.
disable-model-invocation: true
context: fork
agent: neoarchivist:agent
background: false
allowed-tools: Bash(${CLAUDE_PLUGIN_ROOT}/bin/neoarchivist *), Read, Grep, Glob
---

# Open follow-ups

You're running this as neoarchivist:agent, for the user's `/neoarchivist:todos` command. Your
final message is for the user to read, in Markdown, not JSON. Where these steps say to ask
the user, end your reply with the numbered questions, and say they can answer in the
conversation for Claude to pass on; carry on when their answers arrive.

There is no todo store. The TODO list is derived from the memos: every unticked top-level
`- [ ]` box under a `## Follow-ups` heading in a memo is one row, oldest day first,
in the order the boxes appear. Nested boxes are that row's sub-items, and boxes anywhere
else are ordinary checklists. A script has already collected them, so the list doesn't
depend on judgement. Here is its output:

!`${CLAUDE_PLUGIN_ROOT}/bin/neoarchivist todos`

## 1. Show the list

**Print the script's output above exactly as it is:** the table and the summary line under
it. Don't reorder, merge, reword, shorten or drop rows, and don't add any from memory. The
list is deterministic so that it can be trusted. If the output says there are no memos, or
nothing is open, say so and stop.

## 2. Recommend the next low-hanging fruit

After the table, recommend **one** item, by its `#`, as the best one to pick up next. You
may name a runner-up. Judge from the row itself, and if you need more, read the memo it
came from, where the Summary above the box says why it was left open, or the note it links.
Don't run anything that changes the system.

Prefer an item that is:

- **Unblocked.** Drop anything "waiting on" a person, an upstream change or another item.
- **Concrete.** It has a clear next action or a checkable outcome.
- **Small.** It needs few steps, or has only one or two unticked sub-items left.
- **Low-risk.** It doesn't take a running system down or need a sign-off.
- **Old.** Among close calls, pick the one that has been open longest.

Give the recommendation in two or three sentences: which item, why it's the easy one, and
its first concrete step. If nothing qualifies, because everything is blocked or large, say
which item is closest and what it is waiting on.

## 3. Stop

Don't start the work or tick any box. The user picks, and the main agent does the work.
Finishing a follow-up comes back to you later as a `record` request: tick the box in the
day that raised it, mention it in today's memo if finishing it was meaningful, and
rewrite any note whose facts changed.
