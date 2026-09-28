#!/usr/bin/env bash
# Tests for `neoarchivist next-decision`: the number, across branches, and the slug.

set -eu
. "$(dirname "$0")/lib.sh"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t

# run <project> [title]: runs the script for the project, and prints its output and any error.
run() { local p=$1; shift; "$bin" --project "$p" next-decision "$@" 2>&1 | sed 's/^neoarchivist: //' || true; }

echo "next-decision.sh: numbering"

check "no vault" "No vault found at $tmp/none/docs." "$(run "$tmp/none")"

p="$tmp/plain"
mkdir -p "$p/docs"
check "no Decisions folder starts at 00001" "00001" "$(run "$p")"
mkdir -p "$p/docs/Decisions"
touch "$p/docs/Decisions/00001-a.md" "$p/docs/Decisions/00007-b.md" "$p/docs/Decisions/notes.md"
check "one more than the highest, gaps and all" "00008" "$(run "$p")"

p="$tmp/branches"
mkdir -p "$p/docs/Decisions"
git -C "$p" init -q -b main
touch "$p/docs/Decisions/00001-a.md"
git -C "$p" add -A && git -C "$p" commit -qm one
git -C "$p" checkout -qb feature
touch "$p/docs/Decisions/00002-b.md" "$p/docs/Decisions/00003-c.md"
git -C "$p" add -A && git -C "$p" commit -qm two
git -C "$p" checkout -q main
check "counts decisions on other branches" "00004" "$(run "$p")"

git -C "$p" update-ref refs/remotes/origin/elsewhere "$(git -C "$p" rev-parse feature)"
git -C "$p" branch -qD feature
check "and on remote-tracking branches" "00004" "$(run "$p")"

git -C "$p" update-ref -d refs/remotes/origin/elsewhere
git -C "$p" symbolic-ref refs/remotes/origin/HEAD refs/heads/main
check "a symbolic ref is skipped" "00002" "$(run "$p")"

echo "next-decision.sh: the path"

check "the title becomes a slug" "Decisions/00002-authorization-lives-in-middleware-v2.md" \
  "$(run "$p" "Authorization lives in Middleware — v2!")"
check "words without quotes" "Decisions/00002-use-postgres.md" "$(run "$p" Use Postgres)"
check "a title with nothing to slug" "The title needs some letters or digits for its slug." "$(run "$p" "¿¿")"

finish
