#!/usr/bin/env bash
# Tests for `neoarchivist history` and `check`, on real git repositories.

set -eu
. "$(dirname "$0")/lib.sh"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t

p="$tmp/project"
mkdir -p "$p/docs/Memos" "$p/docs/Decisions"
g() { git -C "$p" "$@"; }
g init -q -b main

# commit <day> <message>: commits everything, authored on that day.
commit() { g add -A && GIT_AUTHOR_DATE="$1T12:00:00+00:00" g commit -qm "$2"; }

# history <args> / check <args>: runs the script in the project, printing its output and
# exit status.
history() { local out s=0; out=$(cd "$p" && "$bin" --today 2026-09-28 history "$@" 2>&1) || s=$?; printf '%s [%d]' "$out" "$s"; }
check_sh() { local out s=0; out=$(cd "$p" && "$bin" --today 2026-09-28 check "$@" 2>&1) || s=$?; printf '%s [%d]' "$out" "$s"; }

# reset: drops every staged and unstaged change.
reset() { g reset -q --hard; g clean -qfd; }

cat >"$p/docs/Memos/2026-09-20.md" <<'EOF'
## Summary

- Set up backups. See [[Backups]].

## Follow-ups

- [ ] Verify the restore path.
EOF
printf -- '---\ndate: 2026-09-20\n---\n\n# Use restic\n\n## Why\n\nIt encrypts. See [[Backups]].\n' >"$p/docs/Decisions/00001-use-restic.md"
printf '# Backups\n' >"$p/docs/Backups.md"
commit 2026-09-20 base

echo "history.sh --staged"

check "no changes" " [0]" "$(history --staged)"

sed -i.bak 's/- \[ \] Verify/- [x] Verify/' "$p/docs/Memos/2026-09-20.md" && rm "$p/docs/Memos/2026-09-20.md.bak"
sed -i.bak 's/\[\[Backups\]\]/[[Runbooks\/Backups|Backups]]/' "$p/docs/Decisions/00001-use-restic.md" && rm "$p/docs/Decisions/00001-use-restic.md.bak"
printf '## Summary\n\n- Verified the restore.\n' >"$p/docs/Memos/2026-09-28.md"
printf -- '---\ndate: 2026-09-28\n---\n\n# Keep snapshots\n\n## Why\n\nCheap.\n' >"$p/docs/Decisions/00002-keep-snapshots.md"
g add -A
check "ticking, re-pointing, today's day and today's decision" " [0]" "$(history --staged)"
reset

sed -i.bak 's/Set up backups/Set up backups badly/' "$p/docs/Memos/2026-09-20.md" && rm "$p/docs/Memos/2026-09-20.md.bak"
sed -i.bak 's/It encrypts/It is fast/' "$p/docs/Decisions/00001-use-restic.md" && rm "$p/docs/Decisions/00001-use-restic.md.bak"
printf '## Summary\n\n- Late.\n' >"$p/docs/Memos/2026-09-21.md"
printf '## Summary\n\n- Plans.\n' >"$p/docs/Memos/2026-10-01.md"
printf -- '---\ndate: 2026-09-01\n---\n\n# Backdated\n' >"$p/docs/Decisions/00003-backdated.md"
printf '# Undated\n' >"$p/docs/Decisions/00004-undated.md"
g add -A
check "every kind of rewrite is reported" "\
Decisions/00001-use-restic.md: changes a decision made on 2026-09-20. A changed mind is a new decision that supersedes this one.
Decisions/00003-backdated.md: is a new decision dated 2026-09-01, not 2026-09-28. A decision is dated the day it's made.
Decisions/00004-undated.md: is a new decision dated nowhere, not 2026-09-28. A decision is dated the day it's made.
Memos/2026-09-20.md: changes a past memo's prose. After its day, only checkbox state and link targets may change.
Memos/2026-09-21.md: creates a past day's memo on 2026-09-28. New history goes in that day's own memo.
Memos/2026-10-01.md: writes a future day. A memo records what happened; plans belong in notes. [1]" \
  "$(history --staged)"
reset

g rm -q docs/Memos/2026-09-20.md
check "deleting a past day is reported" \
  "Memos/2026-09-20.md: deletes a past memo. Memos are a record of what was known then. [1]" \
  "$(history --staged)"
reset

g mv docs/Decisions/00001-use-restic.md docs/Decisions/00001-back-up-with-restic.md
check "renaming a decision, content kept, passes" " [0]" "$(history --staged)"
reset

g mv docs/Memos/2026-09-20.md docs/Memos/2026-09-19.md
check "renaming a past day is reported" \
  "Memos/2026-09-20.md: renames a past memo to Memos/2026-09-19.md. Memos are a record of what was known then. [1]" \
  "$(history --staged)"
reset

printf 'unstaged prose\n' >>"$p/docs/Memos/2026-09-20.md"
check "unstaged changes aren't this commit's" " [0]" "$(history --staged)"
reset

echo "history.sh --range"

base=$(g rev-parse HEAD)
printf '## Summary\n\n- Started the day.\n' >"$p/docs/Memos/2026-09-21.md"
commit 2026-09-21 "day one"
printf '\n- Finished the day.\n' >>"$p/docs/Memos/2026-09-21.md"
commit 2026-09-21 "same day, later"
printf '\n- Remembered later.\n' >>"$p/docs/Memos/2026-09-21.md"
commit 2026-09-22 "next day"
late=$(g rev-parse --short HEAD)
check "each commit is judged as of the day it was authored" \
  "$late Memos/2026-09-21.md: changes a past memo's prose. After its day, only checkbox state and link targets may change. [1]" \
  "$(history --range "$base..HEAD")"
g reset -q --hard HEAD~1
check "so a range written on its days passes, whenever it's checked" " [0]" "$(history --range "$base..HEAD")"

echo "check.sh"

printf 'See [[Nowhere]].\n' >"$p/docs/Old problem.md"
commit 2026-09-21 "an old broken link"
printf 'See [[Backups]] and [[Missing]].\n' >"$p/docs/Linking.md"
g add -A
check "lint errors in the notes staged, not older ones" "\
neoarchivist: the notes this changes have errors:

| # | Severity | Where | Problem |
|---|---|---|---|
| 1 | error | \`Linking.md:1\` | Broken link \`[[Missing]]\`: no file named \`Missing\`. | [1]" \
  "$(check_sh --staged)"
reset

sed -i.bak 's/Set up backups/Set up nothing/' "$p/docs/Memos/2026-09-20.md" && rm "$p/docs/Memos/2026-09-20.md.bak"
g add -A
check "history problems, with what to do instead" "\
neoarchivist: this changes the vault's history:

  Memos/2026-09-20.md: changes a past memo's prose. After its day, only checkbox state and link targets may change.

Past memos and older decisions change only by ticking a checkbox or re-pointing a link. New history goes in today's memo, and a changed mind is a new decision. [1]" \
  "$(check_sh --staged)"
reset

printf '## Summary\n\n- Fine.\n' >"$p/docs/Memos/2026-09-28.md"
g add -A
check "a clean commit passes silently" " [0]" "$(check_sh --staged)"
reset

before=$(g rev-parse HEAD)
printf 'See [[Gone]].\n' >"$p/docs/Broken.md"
commit 2026-09-28 "broken"
check "a range lints the notes it changed" yes \
  "$(check_sh --range "$before..HEAD" | grep -q 'Broken.md:1' && echo yes)"

echo "check.sh: a project without a vault"

mkdir -p "$tmp/plain" && git -C "$tmp/plain" init -q
check "nothing to check" " [0]" "$(p="$tmp/plain" check_sh --staged)"

finish
