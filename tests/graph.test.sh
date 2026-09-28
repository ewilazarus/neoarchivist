#!/usr/bin/env bash
# Tests for `neoarchivist graph`: showing, converting, and what it keeps.

set -eu
. "$(dirname "$0")/lib.sh"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# run <args>: runs `graph` on the project at $project, and prints its output and any error.
project=""
run() { "$bin" --project "$project" graph "$@" 2>&1 | sed 's/^neoarchivist: //' || true; }

v="$tmp/docs"
project=$tmp
mkdir -p "$v"

echo "graph.sh: a vault without groups"

check "no vault" "No vault at $tmp/none/docs." "$(project=$tmp/none run show)"
check "no graph.json yet" "No colour groups." "$(run show)"
check "a colour must be six hex digits" "'#4C8BF' isn't a colour: use six hex digits, like #4C8BF5." \
  "$(run set 'path:Memos=#4C8BF')"
check "a group needs a colour" "A group is '<query>=#RRGGBB', not 'path:Memos'." \
  "$(run set 'path:Memos')"

check "set converts hex, keeps the order, and creates the file" "\
Wrote $v/.obsidian/graph.json:
| # | Colour | Query |
|---|---|---|
| 1 | #F5A623 | file:Home OR file:Conventions |
| 2 | #8A8F98 | path:Memos |
| 3 | #4C8BF5 | path:Concepts |" \
  "$(run set 'file:Home OR file:Conventions=#F5A623' 'path:Memos=8a8f98' 'path:Concepts=#4C8BF5')"
check "rgb is stored as an integer" '[16098851,9080728,5016565] false' \
  "$(jq -c '[.colorGroups[].color.rgb]' "$v/.obsidian/graph.json") $(jq '.["collapse-color-groups"]' "$v/.obsidian/graph.json")"

echo "graph.sh: a vault with groups"

jq '. + {scale: 1.4, "showTags": true, search: "auth"}' "$v/.obsidian/graph.json" >"$tmp/g" && mv "$tmp/g" "$v/.obsidian/graph.json"
check "set refuses to replace chosen groups by default" \
  "graph.json already has 3 colour groups, which someone chose. Pass --replace to swap them for these, or --add to keep them and add these." \
  "$(run set 'path:Decisions=#D68A46' 2>&1 | tail -n 1)"

run set --add 'path:Decisions=#D68A46' 'path:Memos=#777777' >/dev/null
check "--add appends new queries and recolours existing ones" "\
| # | Colour | Query |
|---|---|---|
| 1 | #F5A623 | file:Home OR file:Conventions |
| 2 | #777777 | path:Memos |
| 3 | #4C8BF5 | path:Concepts |
| 4 | #D68A46 | path:Decisions |" "$(run show)"
check "other settings are kept" '1.4 true "auth"' \
  "$(jq -r '"\(.scale) \(.showTags) \(.search | tojson)"' "$v/.obsidian/graph.json")"

run set --replace 'path:Notes=#123456' >/dev/null
check "--replace swaps every group" "\
| # | Colour | Query |
|---|---|---|
| 1 | #123456 | path:Notes |" "$(run show)"

echo '{ broken' >"$v/.obsidian/graph.json"
check "invalid JSON is refused, and left alone" "$v/.obsidian/graph.json isn't valid JSON.|{ broken" \
  "$(run set 'path:Notes=#123456')|$(cat "$v/.obsidian/graph.json")"

finish
