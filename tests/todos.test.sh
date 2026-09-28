#!/usr/bin/env bash
# Tests for `neoarchivist todos`: which boxes become rows, and how they print.

set -eu
. "$(dirname "$0")/lib.sh"

todos_at() { "$bin" --project "$(dirname "$1")" todos; }
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# new_vault <name>: makes an empty vault with a memos folder and prints its docs/ path.
new_vault() { mkdir -p "$tmp/$1/docs/Memos"; printf '%s\n' "$tmp/$1/docs"; }

# day <docs> <date>: writes the day file from stdin.
day() { cat >"$1/Memos/$2.md"; }

echo "todos: empty vaults"

check "no memos folder" \
  "No memos found at \`$tmp/none/docs/Memos\`." \
  "$(todos_at "$tmp/none/docs")"

v=$(new_vault empty)
check "a memos folder without memos" "\`Memos/\` has no memos yet." "$(todos_at "$v")"

echo "todos: v2 Follow-ups"

v=$(new_vault scope)
day "$v" 2026-09-10 <<'EOF'
---
description: "Did some work."
---

## Summary

- Did some work.

- [ ] This is NOT a global todo.

## Follow-ups

- [ ] This IS a global todo.
  - [ ] Nested subtask.

## Notes

- [ ] This is also NOT a global todo.
EOF
check "only top-level boxes under Follow-ups count" "\
| # | Raised | Todo | Sub-items |
|---|---|---|---|
| 1 | 2026-09-10 | This IS a global todo. | ☐ Nested subtask. |

1 open follow-up, raised on 1 day. 0 already ticked. 2 unticked boxes elsewhere in the memos are outside a Follow-ups section, so they are not listed." \
  "$(todos_at "$v")"

v=$(new_vault days)
mkdir -p "$v/Reference"
cat >"$v/Reference/Databases.md" <<'EOF'
## Supported databases

- [ ] Redis
- [ ] PostgreSQL
EOF
day "$v" 2026-09-01 <<'EOF'
---
description: "No follow-ups today."
---

## Summary

- Fixed the parser.
EOF
day "$v" 2026-09-02 <<'EOF'
## Summary

- Tidied the runbook.

## Follow-ups

## Notes

- Nothing open.
EOF
day "$v" 2026-09-27 <<'EOF'
## Summary

- Reworked authentication.

## Follow-ups

- [ ] Verify authentication behind the production reverse proxy.
  - Confirm forwarded headers are handled correctly.
  - [x] Verify locally.
  - [ ] Verify against production-equivalent proxy configuration.
- [ ] Add an integration test covering an expired session.
EOF
day "$v" 2026-09-12 <<'EOF'
## Summary

- Reworked GPU passthrough and confirmed the VM boots.

## Follow-ups

- [x] Confirm the VM boots.
  - [ ] A leftover box under a finished parent.
- [ ] Verify WOL after router reboot.
  - [x] Verify locally.
- Plain context bullet, not a task.
* [ ] Test suspend/resume with GPU passthrough,
  including the second monitor.
- [ ] Vérifier la sauvegarde — 日本語も確認。

```text
- [ ] Inside a code fence.
```

### Hardware

- [ ] Replace the thermal paste.

# Appendix

- [ ] After a level-one heading, so outside the section.
EOF
check "multiple days, nesting, bullets, fences, headings, non-ASCII" "\
| # | Raised | Todo | Sub-items |
|---|---|---|---|
| 1 | 2026-09-12 | Verify WOL after router reboot. | ☑ Verify locally. |
| 2 | 2026-09-12 | Test suspend/resume with GPU passthrough, including the second monitor. | — |
| 3 | 2026-09-12 | Vérifier la sauvegarde — 日本語も確認。 | — |
| 4 | 2026-09-12 | Replace the thermal paste. | — |
| 5 | 2026-09-27 | Verify authentication behind the production reverse proxy. | • Confirm forwarded headers are handled correctly. · ☑ Verify locally. · ☐ Verify against production-equivalent proxy configuration. |
| 6 | 2026-09-27 | Add an integration test covering an expired session. | — |

6 open follow-ups, raised on 2 days. 1 already ticked. 1 unticked box elsewhere in the memos is outside a Follow-ups section, so it is not listed." \
  "$(todos_at "$v")"

check "output is stable across runs" "$(todos_at "$v")" "$(todos_at "$v")"

v=$(new_vault done)
day "$v" 2026-09-05 <<'EOF'
## Follow-ups

- [x] Test the restore procedure.
- [X] Rotate the keys.
EOF
check "only ticked follow-ups" "No open follow-ups. 2 already ticked." "$(todos_at "$v")"

finish
