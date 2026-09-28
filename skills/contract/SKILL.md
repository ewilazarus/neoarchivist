---
name: contract
description: The message format for the neoarchivist:agent agent, the only way into this project's docs vault. Load it before sending the agent any request, to look something up in the vault or to record work there, and when a hook refuses a request to the agent.
---

# Talking to neoarchivist:agent

The `neoarchivist:agent` agent is the only way into this project's Obsidian vault. Send
it one JSON object as its whole task, and nothing else. A hook checks every request against
this contract and refuses one that doesn't fit, saying exactly what to fix.

## Look something up

Before work the vault documents, or to answer how or why something is the way it is. Run it
in the foreground and wait:

```json
{"kind": "ask", "question": "How are backups verified?", "why": "About to change the restore script"}
```

`why` is optional, but it helps the agent pick what matters.

## Record work

Once you changed something outside the repo, made a choice whose reason will matter later,
left work unfinished, finished or reopened a follow-up, or found a note wrong. Routine code
changes that git explains need no record. Run it in the background.

Git shows the agent *what* changed in the repository, so don't list files or commands.
Send what git can't show. Every list is optional, but at least one must have something in
it, and each decision needs its `why`, each open item its `done_when`:

```json
{"kind": "record",
 "changed":  ["Nightly backups now go to s3://acme-backups-2"],
 "decided":  [{"what": "Verify restores weekly", "why": "Daily took 40 minutes of I/O", "rejected": "Daily verification"}],
 "dropped":  [{"what": "restic over SFTP", "why": "Timed out on the NAS"}],
 "open":     [{"todo": "Restore one file from the new bucket", "done_when": "It matches its checksum"}],
 "finished": ["Move backups to the new bucket"],
 "reopened": ["Rotate the backup keys"],
 "wrong":    [{"note": "Backups", "issue": "Still names the old bucket"}],
 "commits":  "a1b2c3d..HEAD"}
```

## Replies

The agent answers with one JSON object. Its `status` follows the A2A task states:

- **`completed`**: show its `answer` as it is, when it has one. `sources` are the notes it
  rests on, and `disagreements` are places the vault contradicts itself: mention those.
  `written` lists what it wrote, and `reason` says why it wrote nothing.
- **`input-required`**: ask the user its `questions` word for word, with their `options`,
  and send the answers to **the same agent**, with SendMessage, in the same order:

  ```json
  {"kind": "answers", "answers": ["A project rule: record a decision"]}
  ```

- **`failed`**: tell the user its `reason`.
