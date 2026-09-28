#!/usr/bin/env bash
# Tests for `neoarchivist lint`: what it reports, and how it prints.

set -eu
. "$(dirname "$0")/lib.sh"

# lint_at <docs> <args>: lints the vault at <docs>, a project's docs/.
lint_at() { local docs=$1; shift; "$bin" --project "$(dirname "$docs")" --today 2026-09-28 lint "$@"; }
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# new_vault <name>: makes an empty vault with memos and decisions, and prints its docs/ path.
new_vault() { mkdir -p "$tmp/$1/docs/Memos" "$tmp/$1/docs/Decisions" "$tmp/$1/docs/Reference"; printf '%s\n' "$tmp/$1/docs"; }

# note <docs> <path>: writes the note from stdin.
note() { cat >"$1/$2"; }

echo "lint.sh: empty vaults"

check "no vault" "No vault found at \`$tmp/none/docs\`." "$(lint_at "$tmp/none/docs")"

v=$(new_vault empty)
check "empty vault" "No problems found. Checked 0 notes: 0 memos and 0 decisions." "$(lint_at "$v")"

echo "lint.sh: a clean vault"

v=$(new_vault clean)
: >"$v/Reference/diagram.png"
note "$v" Reference/Spec.md <<'EOF'
# Spec

## 4.2: Assertion

A rule. ^rule
EOF
note "$v" Memos/2026-09-20.md <<'EOF'
---
description: "Set up the spec."
---

## Summary

- Wrote [[Spec#4.2: Assertion|Spec §4.2]], [[Spec#4.2 Assertion\|§4.2]] and [[Spec#^rule]], with ![[diagram.png]].
- Links in code don't count: `[[Also not a link]]`.

```text
[[Not a link]] §9
```

## Decisions

- [[Decisions/00001-use-the-spec|Use the spec]]

## Follow-ups

- [ ] Review [[Reference/Spec|the spec]].
EOF
note "$v" Decisions/00001-use-the-spec.md <<'EOF'
---
description: "Use the spec."
date: 2026-09-20
---

# Use the spec

## Why

See [[#Why|this section]] and [[Memos/2026-09-20]].
EOF
check "resolves paths, names, headings, blocks and embeds, and skips code" \
  "No problems found. Checked 3 notes: 1 memo and 1 decision." \
  "$(lint_at "$v")"

echo "lint.sh: a broken vault"

v=$(new_vault broken)
note "$v" Reference/Spec.md <<'EOF'
# Spec

## Intro
EOF
# A memo in some other format: no Summary, no description.
note "$v" Memos/2026-08-01.md <<'EOF'
## Set up the spec

Wrote [[Spec]].
EOF
note "$v" Memos/2026-09-21.md <<'EOF'
---
description: "Broke things."
---

## Summary

- See [[Nowhere]], [[Spec#Missing heading]], [[Spec#^nope|block]] and [[Spec]].
- Section §4.2 is bare, but `§4.2` and [[Spec#Intro|§1]] are not.

## Follow-ups

## Decisions

- [[Decisions/00002-second|Second]]
EOF
note "$v" Memos/2026-09-22.md <<'EOF'
## Follow-ups

- [ ] Something.
EOF
note "$v" Memos/2030-01-01.md <<'EOF'
## Summary

- Not yet.
EOF
note "$v" Memos/notes.md <<'EOF'
Scratch.
EOF
note "$v" Decisions/00002-first.md <<'EOF'
---
date: 2026-09-21
---

# First

## Why

Because.
EOF
note "$v" Decisions/00002-second.md <<'EOF'
# Second
EOF
# Expected output goes through a file: bash 3.2 misreads an apostrophe in a heredoc inside $().
cat >"$tmp/expected" <<'EOF'
| # | Severity | Where | Problem |
|---|---|---|---|
| 1 | error | `Decisions/00002-first.md` | Decision number 00002 is also used by `Decisions/00002-second.md`. Numbers are never shared. |
| 2 | warning | `Decisions/00002-first.md` | No memo links this decision. The memo of the day it was made lists it under `## Decisions`. |
| 3 | error | `Decisions/00002-second.md` | Decision number 00002 is also used by `Decisions/00002-first.md`. Numbers are never shared. |
| 4 | error | `Decisions/00002-second.md` | No `date: YYYY-MM-DD` in the frontmatter. |
| 5 | warning | `Decisions/00002-second.md` | No `## Why`. A decision says why it was made. |
| 6 | error | `Memos/2026-08-01.md` | No `## Summary`. Every memo has one. |
| 7 | warning | `Memos/2026-08-01.md` | No `description:` in the frontmatter. It is the memo's headline. |
| 8 | error | `Memos/2026-09-21.md:7` | Broken link `[[Nowhere]]`: no file named `Nowhere`. |
| 9 | error | `Memos/2026-09-21.md:7` | Broken link `[[Spec#Missing heading]]`: no heading `Missing heading` in `Reference/Spec.md`. |
| 10 | error | `Memos/2026-09-21.md:7` | Broken link `[[Spec#^nope]]`: no block `^nope` in `Reference/Spec.md`. |
| 11 | warning | `Memos/2026-09-21.md:8` | Bare section reference. Link the section itself, for example `[[Spec#4.2 Assertion\|Spec §4.2]]`. |
| 12 | warning | `Memos/2026-09-21.md:10` | `## Follow-ups` is empty. Leave a section out when it has nothing in it. |
| 13 | warning | `Memos/2026-09-21.md:12` | `## Decisions` comes after `## Follow-ups`. The order is Summary, Decisions, Follow-ups. |
| 14 | error | `Memos/2026-09-22.md` | No `## Summary`. Every memo has one. |
| 15 | warning | `Memos/2026-09-22.md` | No `description:` in the frontmatter. It is the memo's headline. |
| 16 | error | `Memos/2030-01-01.md` | Dated in the future. A memo records what happened. |
| 17 | warning | `Memos/2030-01-01.md` | No `description:` in the frontmatter. It is the memo's headline. |
| 18 | warning | `Memos/notes.md` | Not a memo. `Memos/` holds only `Memos/YYYY-MM-DD.md`. |

9 errors and 9 warnings in 7 files. Checked 8 notes: 5 memos and 2 decisions.
EOF
expected=$(cat "$tmp/expected")
check "reports every kind of problem, sorted by file and line" "$expected" "$(lint_at "$v")"

check "output is stable across runs" "$(lint_at "$v")" "$(lint_at "$v")"

echo "lint.sh: --only"

cat >"$tmp/expected" <<'EOF'
| # | Severity | Where | Problem |
|---|---|---|---|
| 1 | error | `Decisions/00002-second.md` | Decision number 00002 is also used by `Decisions/00002-first.md`. Numbers are never shared. |
| 2 | error | `Decisions/00002-second.md` | No `date: YYYY-MM-DD` in the frontmatter. |
| 3 | warning | `Decisions/00002-second.md` | No `## Why`. A decision says why it was made. |
| 4 | error | `Memos/2030-01-01.md` | Dated in the future. A memo records what happened. |
| 5 | warning | `Memos/2030-01-01.md` | No `description:` in the frontmatter. It is the memo's headline. |

3 errors and 2 warnings in 2 files. Reported on the 2 notes given with --only, not the rest of the vault.
EOF
expected=$(cat "$tmp/expected")
check "reports only the notes given, in any path form" "$expected" \
  "$(lint_at "$v" --only Decisions/00002-second.md --only "$v/Memos/2030-01-01.md")"

check "a clean note among broken ones" \
  "No problems found. Reported on the 1 note given with --only, not the rest of the vault." \
  "$(lint_at "$v" --only docs/Reference/Spec.md)"

finish
