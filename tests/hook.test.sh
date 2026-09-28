#!/usr/bin/env bash
# Tests for `neoarchivist hook`: skill tracking, the reminder, and which writes it lets through.

set -eu
. "$(dirname "$0")/lib.sh"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

project="$tmp/project"
docs="$project/docs"
mkdir -p "$docs/Memos" "$docs/Decisions" "$docs/.obsidian"
export CLAUDE_PROJECT_DIR=$project CLAUDE_PLUGIN_DATA="$tmp/data" CLAUDE_PLUGIN_ROOT=$repo
# The user's own settings: none, so the defaults.
export XDG_CONFIG_HOME="$tmp/xdg"
launcher="$repo/bin/neoarchivist"

today=$(date +%F)
past=2020-01-01

cat >"$docs/Memos/$past.md" <<'EOF'
---
description: "Reworked backups."
---

## Summary

- Reworked backups around restic. See [[Backups]].

## Follow-ups

- [ ] Verify the restore path.
  - [ ] Restore one file.
  - [x] Write the runbook.
* [x] Rotate the keys.
EOF

cat >"$docs/Decisions/00001-use-postgres.md" <<'EOF'
---
description: "Store data in Postgres."
date: 2020-01-01
---

# Use Postgres

## Why

The team already runs it. See [[Databases]].
EOF

session=s1
# The agent a tool call comes from: "" for the main session, or an agent_id with its type.
agent="" agent_type=""
as_main() { agent="" agent_type=""; }
as_keeper() { agent=keeper1 agent_type=neoarchivist:agent; }
as_agent() { agent=$1 agent_type=$2; }

# run <json>: feeds one hook event to the guard and prints its decision: "deny", "remind",
# or "allow" when it prints nothing.
run() {
  local out
  out=$(printf '%s' "$1" | "$bin" hook)
  if [ -z "$out" ]; then
    echo allow
  elif [ "$(jq -r '.hookSpecificOutput.permissionDecision // empty' <<<"$out")" = deny ]; then
    echo deny
  elif [ -n "$(jq -r '.hookSpecificOutput.additionalContext // empty' <<<"$out")" ]; then
    echo remind
  else
    echo "unexpected: $out"
  fi
}

# event <json fields>: common fields for this session and agent, merged with the given ones.
event() {
  jq -nc --arg s "$session" --arg a "$agent" --arg cwd "$project" --argjson extra "$1" \
    --arg t "$agent_type" \
    '{session_id: $s, cwd: $cwd} + (if $a == "" then {} else {agent_id: $a, agent_type: $t} end) + $extra'
}

read_file() {
  run "$(event "$(jq -nc --arg p "$docs/$1" '{hook_event_name: "PreToolUse", tool_name: "Read", tool_input: {file_path: $p}}')")"
}

write_file() {
  run "$(event "$(jq -nc --arg p "$docs/$1" --arg c "$2" '{hook_event_name: "PreToolUse", tool_name: "Write", tool_input: {file_path: $p, content: $c}}')")"
}

edit_file() {
  run "$(event "$(jq -nc --arg p "$docs/$1" --arg o "$2" --arg n "$3" '{hook_event_name: "PreToolUse", tool_name: "Edit", tool_input: {file_path: $p, old_string: $o, new_string: $n}}')")"
}

echo "hook: only the agent touches the vault"

check "the main session can't read docs/" deny "$(read_file Conventions.md)"
check "or write it" deny "$(write_file Notes.md "x")"
check "and is sent to the keeper" yes \
  "$(printf '%s' "$(event "$(jq -nc --arg p "$docs/Notes.md" '{hook_event_name: "PreToolUse", tool_name: "Read", tool_input: {file_path: $p}}')")" |
     "$bin" hook | jq -r .hookSpecificOutput.permissionDecisionReason | grep -q 'neoarchivist:contract' && echo yes)"
check "docs/.obsidian/ is settings, and left alone" allow "$(read_file .obsidian/app.json)"
check "files outside docs/ are ignored" allow \
  "$(run "$(event "$(jq -nc --arg p "$project/src/main.sh" '{hook_event_name: "PreToolUse", tool_name: "Write", tool_input: {file_path: $p, content: "x"}}')")")"
as_agent sub1 general-purpose
check "another subagent can't read docs/" deny "$(read_file Conventions.md)"
as_agent sub2 vault-keeper
check "an agent that merely shares the name can't" deny "$(read_file Conventions.md)"
as_keeper
check "the keeper reads" allow "$(read_file Conventions.md)"
check "and writes" allow "$(write_file Notes.md "x")"

session=s4

echo "hook: today's memo"

check "creating today's journal" allow \
  "$(write_file "Memos/$today.md" $'---\ndescription: "Investigated auth."\n---\n\n## Summary\n\n- Investigated moving authorization into middleware.\n')"
check "consolidating today's summary" allow \
  "$(edit_file "Memos/$today.md" "- Investigated moving authorization into middleware." "- Implemented authorization in middleware after comparing it with a router-level approach.")"
check "adding a follow-up today" allow \
  "$(edit_file "Memos/$today.md" "## Summary" $'## Follow-ups\n\n- [ ] Verify auth behind the proxy.')"

echo "hook: past memos"

check "creating a future day is denied" deny "$(write_file "Memos/2999-01-01.md" "## Summary")"
printf '## Summary\n' >"$docs/Memos/2999-01-02.md"
check "editing an existing future day is denied" deny \
  "$(edit_file "Memos/2999-01-02.md" "## Summary" $'## Summary\n\n- Planned.')"
check "creating a future decision is denied" deny "$(write_file "Decisions/2999-01-01-01-plan.md" "# Plan")"
check "creating a backdated day is denied" deny "$(write_file "Memos/2020-01-02.md" "## Summary")"
check "rewriting past summary prose is denied" deny \
  "$(edit_file "Memos/$past.md" "- Reworked backups around restic." "- Reworked backups around restic, which was a mistake.")"
check "ticking a past top-level follow-up" allow \
  "$(edit_file "Memos/$past.md" "- [ ] Verify the restore path." "- [x] Verify the restore path.")"
check "unticking a past follow-up" allow \
  "$(edit_file "Memos/$past.md" "* [x] Rotate the keys." "* [ ] Rotate the keys.")"
check "ticking a past nested box" allow \
  "$(edit_file "Memos/$past.md" "  - [ ] Restore one file." "  - [x] Restore one file.")"
check "changing task text while ticking is denied" deny \
  "$(edit_file "Memos/$past.md" "- [ ] Verify the restore path." "- [x] Verify the restore path on the new server.")"
check "adding a follow-up to a past day is denied" deny \
  "$(edit_file "Memos/$past.md" "* [x] Rotate the keys." $'* [x] Rotate the keys.\n- [ ] Rotate them again.')"
check "adding a decision link to a past day is denied" deny \
  "$(edit_file "Memos/$past.md" "## Follow-ups" $'## Decisions\n\n- [[Decisions/00001-use-postgres|Use Postgres]]\n\n## Follow-ups')"
check "re-pointing a past link while keeping its text" allow \
  "$(edit_file "Memos/$past.md" "See [[Backups]]." "See [[Runbooks/Backups|Backups]].")"

current=$(cat "$docs/Memos/$past.md"; printf x) current=${current%x}
open_box='- [ ] Verify' ticked_box='- [x] Verify'
check "rewriting a whole past day with only a box changed" allow \
  "$(write_file "Memos/$past.md" "${current/"$open_box"/$ticked_box}")"
check "rewriting a whole past day with a new line is denied" deny \
  "$(write_file "Memos/$past.md" "${current}- [ ] Something new.
")"

echo "hook: decisions"

decision() { printf -- '---\ndescription: "%s"\ndate: %s\n---\n\n# %s\n' "$1" "$2" "$1"; }

check "creating the next decision, dated today" allow \
  "$(write_file "Decisions/00002-authorization-lives-in-middleware.md" "$(decision "Authorization lives in middleware" "$today")")"
check "creating a decision with a dated name is denied" deny \
  "$(write_file "Decisions/$today-01-authorization.md" "$(decision "Authorization" "$today")")"
check "creating a decision without a number is denied" deny \
  "$(write_file "Decisions/authorization-lives-in-middleware.md" "$(decision "x" "$today")")"
check "creating a decision without today's date is denied" deny \
  "$(write_file "Decisions/00002-use-sqlite.md" "$(decision "Use SQLite" 2020-01-02)")"
check "creating a decision without a date is denied" deny \
  "$(write_file "Decisions/00002-use-sqlite.md" "# Use SQLite")"
check "reusing a decision number is denied" deny \
  "$(write_file "Decisions/00001-use-sqlite.md" "$(decision "Use SQLite" "$today")")"
decision "Authorization lives in middleware" "$today" >"$docs/Decisions/00002-authorization-lives-in-middleware.md"
check "filling in a lower number is denied" deny \
  "$(write_file "Decisions/00001-use-sqlite.md" "$(decision "Use SQLite" "$today")")"
check "refining a decision made today" allow \
  "$(edit_file "Decisions/00002-authorization-lives-in-middleware.md" "# Authorization lives in middleware" $'# Authorization lives in middleware\n\n## Why\n\nNested routes.')"
check "rewriting an old decision's rationale is denied" deny \
  "$(edit_file "Decisions/00001-use-postgres.md" "The team already runs it." "SQLite turned out simpler.")"
check "re-dating an old decision to today is denied" deny \
  "$(edit_file "Decisions/00001-use-postgres.md" "date: 2020-01-01" "date: $today")"
check "re-pointing a link in an old decision" allow \
  "$(edit_file "Decisions/00001-use-postgres.md" "[[Databases]]" "[[Reference/Databases|Databases]]")"
check "a superseding decision is a new file" allow \
  "$(write_file "Decisions/00003-use-sqlite.md" "$(decision "Use SQLite" "$today")
## Related

- Supersedes [[Decisions/00001-use-postgres|Use Postgres]]")"

echo "hook: section references"

check "a bare section reference is denied" deny \
  "$(write_file Notes.md "Specified in Spec §4.2.")"
check "a heading link to a section" allow \
  "$(write_file Notes.md "Specified in [[Spec#4.2 Assertion|Spec §4.2]].")"
check "an escaped heading link in a table" allow \
  "$(write_file Notes.md "| field | [[#2.6 Repository IDs\\|§2.6]] |")"
check "a Markdown link to an outside section" allow \
  "$(write_file Notes.md "See [RFC 8949 §4.2.1](https://www.rfc-editor.org/rfc/rfc8949#section-4.2.1).")"
check "a reference in code" allow \
  "$(write_file Notes.md $'Use `§4` literally.\n\n```\nsee §53\n```\n')"
printf 'Legacy text, see §7.\n' >"$docs/Legacy.md"
check "editing near an existing bare reference" allow \
  "$(edit_file Legacy.md "Legacy text, see §7." "Legacy notes, see §7.")"
check "adding one to a file that has some is denied" deny \
  "$(edit_file Legacy.md "Legacy text, see §7." "Legacy text, see §7 and §8.")"
check "a bare reference in today's journal is denied" deny \
  "$(edit_file "Memos/$today.md" "## Summary" $'## Summary\n\n- Settled §7.')"

echo "hook: linting a written note"

# written <path> <content>: puts the note on disk, then sends the PostToolUse of its Write.
written() {
  mkdir -p "$(dirname "$docs/$1")"
  printf '%s' "$2" >"$docs/$1"
  printf '%s' "$(event "$(jq -nc --arg p "$docs/$1" --arg c "$2" '{hook_event_name: "PostToolUse", tool_name: "Write", tool_input: {file_path: $p, content: $c}}')")" |
    "$bin" hook
}

check "a clean note is quiet" "" "$(written Linted.md "See [[Legacy]].")"
check "a warning alone is quiet" "" \
  "$(written "Memos/$today.md" $'---\ndescription: "x"\n---\n\n## Summary\n\n- Done.\n\n## Follow-ups\n')"
check "a note outside the vault is quiet" "" \
  "$(printf '%s' "$(event "$(jq -nc --arg p "$project/README.md" '{hook_event_name: "PostToolUse", tool_name: "Write", tool_input: {file_path: $p}}')")" | "$bin" hook)"
check "an error is handed back as context" "\
PostToolUse
The lint found errors in Linted.md after this write. Fix them now, in the same way as the rest of this change:

| # | Severity | Where | Problem |
|---|---|---|---|
| 1 | error | \`Linted.md:1\` | Broken link \`[[Legacy#Missing]]\`: no heading \`Missing\` in \`Legacy.md\`. |
| 2 | error | \`Linted.md:1\` | Broken link \`[[Nowhere]]\`: no file named \`Nowhere\`. |" \
  "$(written Linted.md "See [[Nowhere]] and [[Legacy#Missing]]." | jq -r '.hookSpecificOutput | .hookEventName, .additionalContext')"
check "another note's errors aren't reported" "" "$(written Other.md "Fine.")"

echo "hook: the contract"

# request <tool> <message> [subagent type]: sends the PreToolUse of an Agent or SendMessage
# call, and prints the decision.
request() {
  local input
  if [ "$1" = Agent ]; then
    input=$(jq -nc --arg p "$2" --arg t "${3:-neoarchivist:agent}" '{hook_event_name: "PreToolUse", tool_name: "Agent", tool_input: {subagent_type: $t, prompt: $p}}')
  else
    input=$(jq -nc --arg m "$2" '{hook_event_name: "PreToolUse", tool_name: "SendMessage", tool_input: {to: "keeper1", message: $m}}')
  fi
  run "$(event "$input")"
}

# subagent <event> <type> <last message> [active]: sends a SubagentStart or SubagentStop, and
# prints the hook's JSON output.
subagent() {
  jq -nc --arg s "$session" --arg e "$1" --arg t "$2" --arg m "${3:-}" --argjson active "${4:-false}" \
    '{session_id: $s, hook_event_name: $e, agent_id: "k9", agent_type: $t, last_assistant_message: $m, stop_hook_active: $active}' |
    "$bin" hook
}

session=contract1
as_main
check "a request that fits goes through" allow "$(request Agent '{"kind": "ask", "question": "Where do backups go?"}')"
check "one in a json fence too" allow "$(request Agent '```json
{"kind": "record", "open": [{"todo": "Verify restore", "done_when": "A file restores"}]}
```')"
check "prose is refused" deny "$(request Agent 'Please record that we chose restic.')"
printf '%s\n' '{"kind": "record", "decided": [{"what": "Use restic"}]}' >"$tmp/no-why.json"
check "a decision without its why is refused, saying so" yes \
  "$(event "$(jq -nc --rawfile p "$tmp/no-why.json" '{hook_event_name: "PreToolUse", tool_name: "Agent", tool_input: {subagent_type: "neoarchivist:agent", prompt: $p}}')" |
     "$bin" hook | jq -r .hookSpecificOutput.permissionDecisionReason | grep -qF '"decided"[0] needs "why"' && echo yes)"
check "other agents' tasks aren't checked" allow "$(request Agent 'Find the flaky test.' general-purpose)"
check "an answers message to the keeper is checked" deny "$(request SendMessage '{"kind": "answers", "answers": []}')"
check "and goes through when it fits" allow "$(request SendMessage '{"kind": "answers", "answers": ["Yes"]}')"
check "a plain message to another agent isn't" allow "$(request SendMessage 'Carry on with the refactor.')"

rules=$(subagent SubagentStart neoarchivist:agent | jq -r .hookSpecificOutput.additionalContext)
check "the keeper starts with its rules" yes \
  "$(printf '%s' "$rules" | grep -q '^# The recall rules' && printf '%s' "$rules" | grep -q '^# The record rules' && echo yes)"
check "with the plugin's launcher filled in" "\"$launcher\" next-decision" \
  "$(printf '%s' "$rules" | grep -o "\"$launcher\" next-decision" | head -n 1)"
check "and no placeholder left" 0 "$(printf '%s' "$rules" | grep -c 'CLAUDE_PLUGIN_ROOT' || true)"
check "other agents start without them" "" "$(subagent SubagentStart general-purpose)"

check "a reply that fits lets the keeper stop" "" \
  "$(subagent SubagentStop neoarchivist:agent '{"status": "completed", "reason": "Nothing to record."}')"
check "one that doesn't sends it back, saying why" "block yes" \
  "$(subagent SubagentStop neoarchivist:agent 'I wrote the journal.' | jq -r '"\(.decision) \(.reason | test("one JSON object") | if . then "yes" else "no" end)"')"
check "but only once" "" "$(subagent SubagentStop neoarchivist:agent 'I wrote the journal.' true)"
check "other agents' replies aren't checked" "" "$(subagent SubagentStop general-purpose 'Done.')"

# subagent_as <agent_id> <event> <type> [last message]: the same, for a given agent_id.
subagent_as() {
  jq -nc --arg s "$session" --arg id "$1" --arg e "$2" --arg t "$3" --arg m "${4:-}" \
    '{session_id: $s, hook_event_name: $e, agent_id: $id, agent_type: $t, last_assistant_message: $m, stop_hook_active: false}' |
    "$bin" hook
}

session=contract2
check "a keeper started by a slash command is told to answer in Markdown" yes \
  "$(subagent_as cmd1 SubagentStart neoarchivist:agent | jq -r .hookSpecificOutput.additionalContext | head -n 1 | grep -q "slash command" && echo yes)"
check "and its Markdown reply isn't held to the contract" "" \
  "$(subagent_as cmd1 SubagentStop neoarchivist:agent '| # | Raised | Todo |')"
request Agent '{"kind": "ask", "question": "One?"}' >/dev/null
request Agent '{"kind": "ask", "question": "Two?"}' >/dev/null
subagent_as par1 SubagentStart neoarchivist:agent >/dev/null
subagent_as par2 SubagentStart neoarchivist:agent >/dev/null
check "two requests sent in parallel start two contract keepers" "block block" \
  "$(subagent_as par1 SubagentStop neoarchivist:agent 'prose' | jq -r .decision) $(subagent_as par2 SubagentStop neoarchivist:agent 'prose' | jq -r .decision)"
subagent_as cmd2 SubagentStart neoarchivist:agent >/dev/null
check "and a command started after them is still a command" "" \
  "$(subagent_as cmd2 SubagentStop neoarchivist:agent 'Markdown for the user.')"

echo "hook: the agent's commands"

# shell_as <command>: sends the PreToolUse of a Bash call, and prints "allow" when the hook
# approves it, or "ask" when it leaves it to the usual permission check.
shell_as() {
  local out
  out=$(event "$(jq -nc --arg c "$1" '{hook_event_name: "PreToolUse", tool_name: "Bash", tool_input: {command: $c}}')" | "$bin" hook)
  [ "$(jq -r '.hookSpecificOutput.permissionDecision // empty' <<<"$out" 2>/dev/null)" = allow ] && echo allow || echo ask
}

as_keeper
check "the plugin's launcher is approved" allow "$(shell_as "$launcher todos")"
check "quoted, with arguments" allow "$(shell_as "\"$launcher\" next-decision \"Use restic\"")"
check "neoarchivist on the PATH, too" allow "$(shell_as "neoarchivist lint --only Memos/2026-09-28.md")"
check "the Obsidian CLI's reads" allow "$(shell_as "obsidian search:context query=backups")"
check "but not its other commands" ask "$(shell_as "obsidian plugin:install id=x")"
check "nor the hook itself" ask "$(shell_as "$launcher hook")"
check "read-only git" allow "$(shell_as "git log --stat -5")"
check "git -C, too" allow "$(shell_as "git -C $project diff")"
check "date" allow "$(shell_as "date +%F")"
check "a launcher outside the plugin isn't" ask "$(shell_as "$repo/../other/bin/neoarchivist todos")"
check "a chained command isn't" ask "$(shell_as "$launcher todos; rm -rf docs")"
check "a substitution isn't" ask "$(shell_as "git log \$(rm -rf docs)")"
check "a redirection isn't" ask "$(shell_as "git diff > docs/x.md")"
check "git that writes isn't" ask "$(shell_as "git commit -am x")"
check "git diff --output isn't" ask "$(shell_as "git diff --output=docs/x.md")"
as_main
check "nor anything from the main session" ask "$(shell_as "$launcher todos")"
as_agent sub3 general-purpose
check "or another agent" ask "$(shell_as "git status")"

echo "hook: the stop reminder"

# stop [active]: sends a Stop event, and prints "remind" when Claude is asked to go on.
stop() {
  local out
  out=$(jq -nc --arg s "$session" --arg cwd "$project" --argjson active "${1:-false}" \
    '{session_id: $s, cwd: $cwd, hook_event_name: "Stop", stop_hook_active: $active}' | "$bin" hook)
  if [ -z "$out" ]; then echo quiet
  elif [ "$(jq -r '.decision' <<<"$out")" = block ]; then echo remind
  else echo "unexpected: $out"; fi
}

# changed <path>: sends the PostToolUse of an Edit to a file.
changed() {
  event "$(jq -nc --arg p "$1" '{hook_event_name: "PostToolUse", tool_name: "Edit", tool_input: {file_path: $p, old_string: "a", new_string: "b"}}')" |
    "$bin" hook >/dev/null
}

session=stop1
as_main
check "a session that changed nothing stops quietly" quiet "$(stop)"
changed "$project/src/app.sh"
check "changes without using the keeper stop quietly" quiet "$(stop)"
request Agent '{"kind": "ask", "question": "How is it deployed?"}' >/dev/null
check "after using the keeper, changes get a reminder" remind "$(stop)"
check "only once" quiet "$(stop)"
changed "$project/src/app.sh"
check "never while a stop hook is already active" quiet "$(stop true)"
check "and new changes get a new reminder" remind "$(stop)"

changed "$project/src/app.sh"
request Agent '{"kind": "record", "changed": ["Deployed to the new host"]}' >/dev/null
check "a record request clears it" quiet "$(stop)"

as_keeper
changed "$project/CLAUDE.md"
as_main
check "the keeper's own writes outside docs/ don't count" quiet "$(stop)"

changed "$docs/Notes.md"
changed "$tmp/elsewhere/file.txt"
changed "$project/.git/config"
check "notes, files outside the project and .git don't count" quiet "$(stop)"

as_agent sub9 general-purpose
changed "$project/src/app.sh"
as_main
check "another subagent's changes count for the session" remind "$(stop)"

changed "$project/src/app.sh"
mkdir -p "$XDG_CONFIG_HOME/neoarchivist" && echo 'stop_reminder = false' >"$XDG_CONFIG_HOME/neoarchivist/config.toml"
check "stop_reminder = false in the user's config turns it off" quiet "$(stop)"
rm "$XDG_CONFIG_HOME/neoarchivist/config.toml"

session=s4

echo "hook: branches and merges"

as_keeper

export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
repo_project="$tmp/merging"
mkdir -p "$repo_project/docs/Memos" "$repo_project/docs/Decisions"
g() { git -C "$repo_project" "$@"; }
g init -q -b main
printf '# Old name\n' >"$repo_project/docs/Old name.md"
decision "Use Postgres" 2020-01-01 >"$repo_project/docs/Decisions/00001-use-postgres.md"
g add -A && g commit -qm base
g checkout -qb feature
decision "Use Redis" 2020-01-02 >"$repo_project/docs/Decisions/00002-use-redis.md"
printf 'See [[Old name]].\n' >"$repo_project/docs/Linking.md"
g add -A && g commit -qm feature
g checkout -q main
g mv "docs/Old name.md" "docs/New name.md"
g commit -qm rename

# shell <command>: sends the PostToolUse of a Bash command run in the git project.
shell() {
  jq -nc --arg s "$session" --arg cwd "$repo_project" --arg c "$1" \
    '{session_id: $s, cwd: $cwd, hook_event_name: "PostToolUse", tool_name: "Bash", tool_input: {command: $c}}' |
    CLAUDE_PROJECT_DIR=$repo_project "$bin" hook
}

check "a decision number taken on another branch is denied" deny "$(
  export CLAUDE_PROJECT_DIR=$repo_project docs=$repo_project/docs
  write_file Decisions/00002-use-sqlite.md "$(decision "Use SQLite" "$today")")"
check "the next number after every branch's" allow "$(
  export CLAUDE_PROJECT_DIR=$repo_project docs=$repo_project/docs
  write_file Decisions/00003-use-sqlite.md "$(decision "Use SQLite" "$today")")"

decision "Use SQLite" 2020-01-03 >"$repo_project/docs/Decisions/00002-use-sqlite.md"
g add -A && g commit -qm sqlite

check "other shell commands are quiet" "" "$(shell "git status")"
g merge -q --no-edit feature
merged=$(shell "cd $repo_project && git merge feature" | jq -r '.hookSpecificOutput.additionalContext')
check "a merge reports the shared number" yes \
  "$(printf '%s' "$merged" | grep -q 'Decision number 00002 is also used by `Decisions/00002-use-sqlite.md`' && echo yes)"
check "and the link to a note renamed on the other side" yes \
  "$(printf '%s' "$merged" | grep -q 'Broken link `\[\[Old name\]\]`' && echo yes)"
check "but not notes the merge didn't change" no \
  "$(printf '%s' "$merged" | grep -q '00002-use-sqlite.md` | Decision' && echo yes || echo no)"
check "git -C and pull count as well" yes \
  "$(shell "git -C $repo_project pull --no-rebase" | grep -q additionalContext && echo yes)"
g reset -q --hard HEAD~1 && g update-ref ORIG_HEAD HEAD
check "nothing to compare is quiet" "" "$(shell "git merge feature")"

echo "hook: failing open"

out=$(printf 'not json' | "$bin" hook 2>/dev/null) && status=0 || status=$?
check "bad input lets the action through with a non-blocking error" "1:" "$status:$out"

finish
