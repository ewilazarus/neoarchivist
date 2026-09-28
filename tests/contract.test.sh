#!/usr/bin/env bash
# Tests for `neoarchivist contract`, and for the promises the plugin's files make about it.

set -eu
. "$(dirname "$0")/lib.sh"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# verdict <request|reply> <message>: "ok", or the problems, one per line.
verdict() { printf '%s' "$2" | "$bin" contract check "$1" 2>&1 || true; }

echo "contract: requests"

check "an ask" ok "$(verdict request '{"kind": "ask", "question": "Where do backups go?", "why": "Moving them"}')"
check "a record, in a json fence" ok "$(verdict request '
```json
{"kind": "record", "changed": ["Backups go to the new bucket"], "commits": "a1..b2"}
```
')"
check "answers" ok "$(verdict request '{"kind": "answers", "answers": ["Yes"]}')"
check "prose" "The message isn't one JSON object: send only the object, alone or in a \`\`\`json fence." \
  "$(verdict request 'Please record that we chose restic.')"
check "two objects" "The message isn't one JSON object: send only the object, alone or in a \`\`\`json fence." \
  "$(verdict request '{"kind": "ask", "question": "a"} {"kind": "ask", "question": "b"}')"
check "an unknown kind" '"kind" must be "ask", "record" or "answers".' "$(verdict request '{"kind": "note"}')"
check "an ask without a question" 'An ask needs "question", as text.' "$(verdict request '{"kind": "ask", "question": "  "}')"
check "a misspelt field, a missing why and a missing done_when" '
A record has an unknown field "dicided".
"decided"[0] needs "why", as text.
"open"[0] needs "done_when", as text.' \
  "$(printf '\n'; verdict request '{"kind": "record", "dicided": [], "decided": [{"what": "x"}], "open": [{"todo": "y"}]}')"
check "an empty record" \
  "A record needs at least one non-empty list: changed, decided, dropped, open, finished, reopened or wrong." \
  "$(verdict request '{"kind": "record", "changed": [], "commits": "a..b"}')"
check "a list of the wrong type" '"changed" must be a list of text.' \
  "$(verdict request '{"kind": "record", "changed": "Backups moved", "finished": ["x"]}')"
check "no answers" '"answers" must be a non-empty list of text.' "$(verdict request '{"kind": "answers", "answers": []}')"

echo "contract: replies"

check "an answer" ok "$(verdict reply '{"status": "completed", "answer": "Weekly.", "sources": ["[[Backups]]"]}')"
check "nothing to record" ok "$(verdict reply '{"status": "completed", "reason": "Routine refactor."}')"
check "questions" ok "$(verdict reply '{"status": "input-required", "questions": [{"q": "Decision?", "options": ["yes", "no"]}]}')"
check "a completed reply with nothing in it" \
  "A completed reply needs an answer, what was written, or a reason there was nothing to write." \
  "$(verdict reply '{"status": "completed", "written": []}')"
check "input-required without questions" "An input-required reply needs its questions." \
  "$(verdict reply '{"status": "input-required", "questions": []}')"
check "failed without a reason" 'A failed reply needs "reason", as text.' "$(verdict reply '{"status": "failed"}')"
check "an unknown status and field" '
The reply has an unknown field "extra".
"status" must be "completed", "input-required" or "failed".' \
  "$(printf '\n'; verdict reply '{"status": "done", "extra": 1}')"
check "a written entry without its change" '"written"[0] needs "change", as text.' \
  "$(verdict reply '{"status": "completed", "written": [{"path": "Journal/2026-09-28.md"}]}')"

echo "contract: the plugin's files keep to it"

# Every ```json block in the files that teach the contract must pass as a request or reply.
for file in agents/agent.md skills/contract/SKILL.md skills/record/SKILL.md assets/CLAUDE-section.md; do
  awk -v out="$tmp/block" '
    /^[ \t]*```json[ \t]*$/ { n++; inside = 1; next }
    inside && /^[ \t]*```[ \t]*$/ { inside = 0; next }
    inside { sub(/^  /, ""); print > (out "." n) }' "$repo/$file"
  bad=""
  for block in "$tmp"/block.*; do
    [ -f "$block" ] || continue
    if [ "$(verdict request "$(cat "$block")")" != ok ] && [ "$(verdict reply "$(cat "$block")")" != ok ]; then
      bad="$bad $(basename "$block")"
    fi
  done
  rm -f "$tmp"/block.*
  check "every example in $file fits" "" "$bad"
done

# Forked commands run in the agent, and the main agent sees no skill but the contract.
forked=$(grep -l '^context: fork' "$repo"/skills/*/SKILL.md | wc -l | tr -d ' ')
in_agent=$(grep -l '^agent: neoarchivist:agent' "$repo"/skills/*/SKILL.md | wc -l | tr -d ' ')
check "every forked command runs in the agent" "$forked" "$in_agent"
check "the agent exists" "name: agent" "$(grep -m1 '^name:' "$repo/agents/agent.md")"
visible=$(for f in "$repo"/skills/*/SKILL.md; do grep -q '^disable-model-invocation: true' "$f" || basename "$(dirname "$f")"; done)
check "the main agent sees only the contract skill" contract "$visible"
check "no file still names the old project" "" \
  "$(grep -rIl -i 'docs-vault\|vault-keeper\|keeper-contract' "$repo/agents" "$repo/skills" "$repo/rules" "$repo/assets" "$repo/hooks" "$repo/bin" "$repo/src" "$repo/.claude-plugin" || true)"

finish
